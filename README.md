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
- Accessibility/monochrome friendly: honors `NO_COLOR`, avoids color-only meaning, shows text labels for status.

## Getting started (current scaffold)

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo run -p tunez-cli
```

### CLI helpers (Phase 1 wiring)

- List configured providers: `cargo run -p tunez-cli -- providers list`
- Build a play request (selectors only for now):  
  `cargo run -p tunez-cli -- play --provider <id> --track "<name>" --album "<album>" --artist "<artist>" -p`

## Running the executable (`tunez`)

The runnable binary name is `tunez`.

Run via Cargo (recommended during development):

```bash
# Equivalent; the package is src/tunez-cli and it builds the `tunez` binary
cargo run -p tunez-cli

# Explicitly name the binary
cargo run -p tunez-cli --bin tunez
```

Run the built binary directly:

```bash
cargo build -p tunez-cli
./target/debug/tunez

cargo build -p tunez-cli --release
./target/release/tunez
```

Install `tunez` onto your PATH:

```bash
cargo install --path src/tunez-cli
tunez
```

## Workspace layout

- `src/tunez-core/` — shared domain (config, paths, provider/scrobbler traits, contract tests)
- `src/tunez-cli/` — binary entrypoint (`tunez`)
- `src/tunez-ui/` — ratatui UI shell
- `src/tunez-player/` — queue + playback state machine
- `src/tunez-audio/` — audio engine wiring
- `src/tunez-plugin/` — external plugin host (Phase 2)
- `src/providers/` — built-in providers (filesystem, melodee)

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

### External Plugin Providers (Phase 2)

Tunez supports external plugins via an exec-based protocol. Plugins communicate with Tunez using JSON messages over stdin/stdout.

Example plugin provider configuration:

```toml
[providers.my-plugin]
kind = "plugin"

[providers.my-plugin.profiles.default]
plugin_executable = "/usr/local/bin/my-music-plugin"
plugin_args = ["--config", "/etc/my-plugin.toml"]
```

For plugin development documentation, see `src/tunez-plugin/src/lib.rs`.

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
