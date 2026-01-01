# Tunez

Terminal music player in full ANSI color.

![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)

## Status

Pre-alpha. This repository is currently **docs-first** (PRD + TUI mockups). Implementation is planned next.

## What is Tunez?

Tunez is a fast, keyboard-first terminal music player with a modular “Provider” architecture (multiple backends), a rich TUI, and real-time audio visualization.

## Quick links

- PRD / Phase 1 requirements: [docs/tunez-requirements.md](docs/tunez-requirements.md)
- TUI layout mockups (canonical UX reference): [docs/tunez-tui-mockups.md](docs/tunez-tui-mockups.md)
- License: [LICENSE](LICENSE)

## Goals (Phase 1)

- Playback with robust buffering and minimal stutter
- Rich TUI: browse/search, queue/now-playing, progress, volume, shuffle/repeat
- Full-color + animation (smooth progress, transitions, spinners)
- Spectrum/waveform visualization (at least a spectrum analyzer)
- Cross-platform terminal behavior (Linux/macOS/Windows)
- Modular from day 1: built-in Providers (Phase 1), future-proof for external plugins (Phase 2)

## Repository layout

Current:

- `docs/` — PRD and ASCII TUI mockups
- `docs-site/` — reserved for future documentation site
- `scripts/` — reserved for build/dev scripts

Planned Rust workspace layout (see PRD):

- `tunez-core/` — domain types, Provider traits, errors
- `tunez-ui/` — ratatui UI, themes, keybindings
- `tunez-player/` — queue + playback state machine
- `tunez-audio/` — stream reader, decoder, output, buffering
- `tunez-viz/` — spectrum/waveform computation
- `tunez-cli/` — CLI parsing and command dispatch
- `providers/` — built-in Provider crates

## Development (once implementation lands)

Prerequisites:

- Rust toolchain (stable) with `cargo`, `rustfmt`, and `clippy`

Expected quality gates:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## Contributing

Contributions are welcome.

- For current work, the best starting point is the PRD: [docs/tunez-requirements.md](docs/tunez-requirements.md)
- Keep changes scoped and consistent with Phase goals/non-goals
- If you change any intended CLI/config/UI behavior described in docs, update the docs in the same PR

## License

MIT. See [LICENSE](LICENSE).
