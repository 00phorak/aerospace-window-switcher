use eframe::egui;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Mutex, Arc};

const SEARCH_BOX_HEIGHT: f32 = 32.0;
const ITEM_HEIGHT: f32 = 28.0;
const MAX_LIST_HEIGHT: f32 = 400.0;
const PADDING_TOP: f32 = 8.0;

struct WindowInfo {
    id: String,
    name: String,
    info: String,
}

struct AerospaceWindowSwitcher {
    windows: Vec<WindowInfo>,
    search_query: String,
    filtered_windows: Vec<usize>,
    selected_index: Option<usize>,
    is_loading: bool,
    load_start_time: std::time::Instant,
    window_to_focus: Option<String>,
    windows_shared: Arc<Mutex<Option<Vec<WindowInfo>>>>,
}

impl Default for AerospaceWindowSwitcher {
    fn default() -> Self {
        let windows_shared = Arc::new(Mutex::new(None));
        let windows_shared_clone = windows_shared.clone();

        std::thread::spawn(move || {
            let fetched = Self::fetch_windows();
            let mut guard = windows_shared_clone.lock().unwrap();
            *guard = Some(fetched);
        });

        Self {
            windows: Vec::new(),
            search_query: String::new(),
            filtered_windows: Vec::new(),
            selected_index: None,
            is_loading: true,
            load_start_time: std::time::Instant::now(),
            window_to_focus: None,
            windows_shared,
        }
    }
}

impl AerospaceWindowSwitcher {
    fn fetch_windows() -> Vec<WindowInfo> {
        let output = match Command::new("aerospace")
            .args(["list-windows", "--all"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                eprintln!("Failed to execute aerospace command: {}", e);
                return Vec::new();
            }
        };

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            eprintln!("Aerospace command failed: {}", error);
            return Vec::new();
        }

        let reader = BufReader::new(output.stdout.as_slice());
        reader
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(3, '|').collect();
                if parts.len() >= 3 {
                    Some(WindowInfo {
                        id: parts[0].trim().to_string(),
                        name: parts[1].trim().to_string(),
                        info: parts[2].trim().to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    fn filter_windows(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_windows = (0..self.windows.len()).collect();
            self.selected_index = Some(0);
            return;
        }

        let matcher = SkimMatcherV2::default();
        let mut scored_indices: Vec<(usize, i64)> = self.windows
            .iter()
            .enumerate()
            .filter_map(|(idx, window)| {
                let name_score = matcher.fuzzy_match(&window.name, &self.search_query);
                let info_score = matcher.fuzzy_match(&window.info, &self.search_query);
                match (name_score, info_score) {
                    (Some(s1), Some(s2)) => Some((idx, s1.max(s2))),
                    (Some(s), None) | (None, Some(s)) => Some((idx, s)),
                    (None, None) => None,
                }
            })
            .collect();

        scored_indices.sort_by(|a, b| b.1.cmp(&a.1));
        self.filtered_windows = scored_indices.into_iter().map(|(idx, _)| idx).collect();
        self.selected_index = Some(0);
    }

    fn is_loading_timed_out(&self) -> bool {
        self.load_start_time.elapsed() > std::time::Duration::from_secs(2)
    }

    fn focus_selected_window(&mut self) -> bool {
        if let Some(selected) = self.selected_index {
            if let Some(&idx) = self.filtered_windows.get(selected) {
                self.window_to_focus = Some(self.windows[idx].id.clone());
                return true;
            }
        }
        false
    }
}

impl eframe::App for AerospaceWindowSwitcher {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.is_loading {
            let should_update = {
                let mut guard = self.windows_shared.lock().unwrap();
                if let Some(fetched) = guard.take() {
                    self.windows = fetched;
                    true
                } else if self.is_loading_timed_out() {
                    true
                } else {
                    false
                }
            };
            if should_update {
                self.is_loading = false;
                self.filter_windows();
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Enter))
            && self.selected_index.is_some()
            && self.focus_selected_window()
        {
            if let Some(window_id) = self.window_to_focus.take() {
                let _ = Command::new("sh")
                    .args(["-c", &format!("sleep 0.05 && aerospace focus --window-id {}", window_id)])
                    .spawn();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if !self.filtered_windows.is_empty() {
            if ctx.input(|i| {
                i.key_pressed(egui::Key::ArrowDown)
                    || (i.modifiers.ctrl
                        && (i.key_pressed(egui::Key::N) || i.key_pressed(egui::Key::J)))
            }) {
                self.selected_index =
                    Some((self.selected_index.unwrap_or(0) + 1) % self.filtered_windows.len());
            } else if ctx.input(|i| {
                i.key_pressed(egui::Key::ArrowUp)
                    || (i.modifiers.ctrl
                        && (i.key_pressed(egui::Key::P) || i.key_pressed(egui::Key::K)))
            }) {
                self.selected_index = Some(if let Some(index) = self.selected_index {
                    if index == 0 {
                        self.filtered_windows.len() - 1
                    } else {
                        index - 1
                    }
                } else {
                    0
                });
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(PADDING_TOP);

            let search_response = ui.add_sized(
                [ui.available_width(), SEARCH_BOX_HEIGHT],
                egui::TextEdit::singleline(&mut self.search_query)
                    .frame(true)
                    .margin(egui::vec2(8.0, 8.0))
                    .font(egui::TextStyle::Monospace),
            );

            if search_response.changed() {
                self.filter_windows();
            }

            if !ui.memory(|m| m.has_focus(search_response.id)) {
                ui.memory_mut(|m| m.request_focus(search_response.id));
            }

            ui.add_space(8.0);

            if self.is_loading {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new("Loading windows...")
                            .color(egui::Color32::from_rgb(180, 180, 180)),
                    );
                });
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .max_height(MAX_LIST_HEIGHT)
                    .show(ui, |ui| {
                        let mut selected = self.selected_index.unwrap_or(0);
                        let mut window_to_focus = None;

                        for (idx, &win_idx) in self.filtered_windows.iter().enumerate() {
                            let window = &self.windows[win_idx];
                            let is_selected = selected == idx;

                            let text = format!("{} | {}", window.name, window.info);
                            let button =
                                egui::Button::new(egui::RichText::new(text).monospace())
                                    .fill(if is_selected {
                                        egui::Color32::from_rgba_premultiplied(
                                            70, 130, 180, 200,
                                        )
                                    } else {
                                        ui.style().visuals.widgets.inactive.bg_fill
                                    })
                                    .min_size(egui::vec2(ui.available_width(), ITEM_HEIGHT));

                            if ui.add(button).clicked() {
                                selected = idx;
                                window_to_focus = Some(win_idx);
                            }
                        }

                        self.selected_index = Some(selected);

                        if let Some(idx) = window_to_focus {
                            self.selected_index =
                                self.filtered_windows.iter().position(|&i| i == idx);
                            if self.focus_selected_window() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    });
            }
        });
    }
}

fn main() {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 400.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_active(false)
            .with_visible(true),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Aerospace Window Switcher",
        native_options,
        Box::new(|cc| {
            let mut style = (*cc.egui_ctx.style()).clone();
            style.visuals.window_shadow.blur = 8;
            style.visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
            style.visuals.widgets.hovered.bg_fill =
                egui::Color32::from_rgba_premultiplied(60, 60, 60, 180);
            style.visuals.widgets.active.bg_fill =
                egui::Color32::from_rgba_premultiplied(80, 80, 80, 180);
            style.visuals.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
            style.visuals.override_text_color = Some(egui::Color32::from_rgb(220, 220, 220));
            style.visuals.panel_fill = egui::Color32::TRANSPARENT;
            style.visuals.window_fill = egui::Color32::TRANSPARENT;
            cc.egui_ctx.set_style(style);
            Ok(Box::new(AerospaceWindowSwitcher::default()))
        }),
    );
}
