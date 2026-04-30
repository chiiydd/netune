# CLAUDE.md

Guidance for AI agents working in this repository.

## What this project is

`netune` is a terminal Netease Cloud Music player built in Rust. It has a 4-layer
workspace architecture inspired by [wisp](https://github.com/chiiydd/wisp).

## Crate layout

```
L3  netune-tui       ← ratatui pages, events, theme, chrome
L2  netune-player    ← playback control, queue, rodio audio
L1  netune-api       ← Netease Cloud Music HTTP client, crypto
L0  netune-core      ← data models, error types, config, trait definitions
```

**Dependencies are strictly one-way**, low → high. `netune-tui` must not import
`netune-api` or `netune-core` directly — everything comes through `netune-player`
or via re-exports.

## TUI architecture (follow wisp pattern)

- `app.rs` — state machine + event loop + page_stack
- `chrome.rs` — title bar + statusline (pages don't draw their own chrome)
- `theme.rs` — color constants (Theme::ACCENT etc.)
- `pages/*.rs` — each page implements:
  - `title()` — page title
  - `mode()` — mode badge (label + color)
  - `context()` — statusline context spans
  - `hints()` — keybinding hints
  - `render()` — page body rendering
  - `handle_event()` — keyboard events, returns PageAction
  - `tick()` — periodic refresh (for playback progress etc.)
- `widgets/*.rs` — reusable components (progress bar, lyric view)

## Development groups

| Group | Agent | Modules |
|-------|-------|---------|
| A 组 | Codex | netune-core + netune-api + netune-player |
| B 组 | Claude Code | netune-tui (all pages and widgets) |

## Conventions

- **Async**: tokio-only
- **Errors**: `thiserror` for library, `color_eyre::Result` at binary boundary
- **Paths**: `camino::Utf8PathBuf` over `std::path::PathBuf`
- **Logging**: `tracing`
- **MSRV**: 1.85
- **API response models**: use `#[serde(rename_all = "camelCase")]` for JSON fields

## Build & test

```sh
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo run --release
```
