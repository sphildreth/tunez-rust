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

## Getting started (current scaffold)

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo run -p tunez-cli
```

## Workspace layout (in progress)

- `src/tunez-core/` — shared utilities (config loading, logging bootstrap, app paths)
- `src/tunez-cli/` — binary entrypoint (`tunez`)
- Planned: `src/tunez-ui/`, `src/tunez-player/`, `src/tunez-audio/`, `src/tunez-viz/`, `src/providers/` (see PRD)

## Configuration

- Default config path: `${CONFIG_DIR}/tunez/config.toml` (resolved via `directories`).
- Schema versioned by `config_version`; unknown/missing file uses safe defaults.

Example snippet (logging fields added in Phase 1A):

```toml
config_version = 1

[logging]
level = "info"        # trace|debug|info|warn|error
max_log_files = 7     # retention; oldest files pruned
stdout = true         # also emit logs to stdout
```

## Repository layout

Current:

- `docs/` — PRD and ASCII TUI mockups
- `docs-site/` — reserved for future documentation site
- `scripts/` — reserved for build/dev scripts
- `src/` — Rust workspace crates

Planned Rust workspace layout (see PRD; crates will live under `src/`):

- `src/tunez-core/` — domain types, Provider traits, errors
- `src/tunez-ui/` — ratatui UI, themes, keybindings
- `src/tunez-player/` — queue + playback state machine
- `src/tunez-audio/` — stream reader, decoder, output, buffering
- `src/tunez-viz/` — spectrum/waveform computation
- `src/tunez-cli/` — CLI parsing and command dispatch
- `src/providers/` — built-in Provider crates

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
