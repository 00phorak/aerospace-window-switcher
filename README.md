# aerospace-window-switcher

A simple, <i>blazingly fast</i>™ window switcher for the Aerospace window manager.

## Description

This application provides a quick way to switch between windows in Aerospace. It displays a list of windows and allows you to search and select a window to focus.

## Usage
Put the following keybind into your aerospace config toml with your desired keybind:
```toml
alt-space = 'exec-and-forget <path-to-binary>'
```
Then press alt-space to bring up the aerospace window switcher and start typing. It wil fuzzy find your desired app and then you can confirm your selection with Enter to switch to the window/workspace.
```
Esc - exit window switcher
Enter - confirm your choice
C-j or C-n - next selection
C-k or C-p - previous selection
```

## Dependencies

- Rust (stable toolchain)
- eframe (egui framework)
- fuzzy-matcher

## Building

To build the project, run:

```bash
cargo build --release
```

## Running

To run the application, execute:

```bash
cargo run --release
```

Or, if you've built it, run the binary directly:

```bash
./target/release/aerospace-window-switcher
```
