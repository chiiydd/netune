# netune

A terminal Netease Cloud Music player built in Rust.

<p align="center">
  <em><!-- screenshot placeholder --></em>
</p>

## Features

- **QR code login** + browser cookie import
- **Search** songs, playlists, daily recommendations
- **Playback** with queue (Sequential / LoopAll / LoopOne / Shuffle)
- **Lyrics** display with translation support
- **Disk audio cache** with LRU eviction
- **Queue state** persistence (save / load)
- **CJK-aware** TUI layout

## Architecture

netune uses a 4-layer workspace architecture inspired by [wisp](https://github.com/chiiydd/wisp). Dependencies are strictly one-way: **low → high**.

```
L3  netune-tui       ← ratatui pages, events, theme, chrome
 │
L2  netune-player    ← playback control, queue, rodio audio
 │
L1  netune-api       ← Netease Cloud Music HTTP client, crypto
 │
L0  netune-core      ← data models, error types, config, trait definitions
```

## Tech Stack

| Layer | Crate | Role |
|-------|-------|------|
| TUI | ratatui 0.29 + crossterm 0.28 | Terminal UI framework |
| Audio | rodio 0.22 | Audio playback |
| Async | tokio | Async runtime |
| HTTP | reqwest (`.no_proxy()`) | HTTP client, bypass proxy |
| Crypto | AES-CBC / ECB | Netease API encryption |
| Error | thiserror + color-eyre | Error handling |
| Logging | tracing | Structured logging |

## Build & Run

```bash
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo run --release
cargo bench --all
```

## Testing

- **105** unit tests — `cargo test --all-targets`
- **4** criterion benchmarks — `cargo bench --all`

## Project Structure

```
netune/
├── Cargo.toml              # workspace root
├── crates/
│   ├── netune-core/        # L0: models, errors, config, traits
│   ├── netune-api/         # L1: Netease HTTP client, crypto
│   ├── netune-player/      # L2: playback, queue, rodio
│   └── netune-tui/         # L3: ratatui UI, events, theme
└── README.md
```

## License

MIT
