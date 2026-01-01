# GitHub Copilot Instructions — Tunez (Rust)

You are working in the `Tunez` repository: a cross-platform Rust CLI + TUI music player.
Primary goals: fast keyboard-first TUI (ratatui), robust playback, and a modular architecture with built-in Providers and Scrobblers.

## 1) Working Agreements (Read First)
- Follow the project requirements docs as the source of truth (Tunez PRD + TUI mockups).
- Prefer small, reviewable commits and incremental changes over big rewrites.
- If something is ambiguous, propose 1–2 reasonable options and choose the simplest.
- Do not introduce speculative features not in scope (no LLM/agent frameworks).

## 2) Architectural Principles
- Keep Tunez modular internally (workspace crates/modules), but it ships as a single app: `tunez`.
- Providers and Scrobblers are built-in Rust components (no external plugin host in Phase 1).
- **Provider stream contract (Phase 1): Providers return a stream URL only.**
- Providers declare capabilities (e.g., playlists, lyrics, offline download support).
- UI must degrade gracefully when a capability is absent.

### Provider
- Use a `Provider` trait in a shared crate (`tunez-core` or equivalent).
- All Providers must implement:
  - search tracks
  - get track metadata
  - get stream URL
  - list/search playlists if the provider supports playlists (capability-gated)
- If a capability is unsupported, return a typed `NotSupported` error (do not panic).

### Scrobbler
- Use a `Scrobbler` trait in a shared crate.
- Tunez provides scrobblers with playback telemetry:
  - track identity + metadata
  - track duration (seconds, when known)
  - played duration / position (seconds)
  - state transitions (started/resumed/paused/stopped/ended)
- The scrobbler decides its own reporting rules/intervals (Tunez does not hard-code thresholds).
- Scrobbling failures must never interrupt playback (log + UI indicator only).

## 3) Rust Style and Quality Bar
- Write idiomatic, safe Rust. Avoid `unsafe` unless absolutely necessary.
- Prefer strong types and explicit error handling (`thiserror`-style) over strings.
- Avoid unnecessary clones; design ownership clearly.
- Use `tracing` for logging; do not use `println!` for operational logs.

### Error handling
- No panics for normal runtime failures (network errors, decode errors, unsupported formats).
- For invalid/unreadable tracks: log it, show a user-visible message, and skip to next track.

## 4) Async / Concurrency
- Keep the TUI responsive: never block the UI loop on network/audio.
- Prefer message/event-driven design:
  - UI tick events
  - player events (track changed, progress, errors)
  - provider events (search results, paging)
- Use `tokio` tasks for network/background work; coordinate via channels.
- Avoid `Arc<Mutex<...>>` unless needed; prefer single-owner state with message passing.

## 5) TUI Guidance (ratatui)
- Follow the TUI mockups for layout and navigation patterns.
- Provide keyboard-first navigation with discoverable help (`?` overlay).
- Keep rendering efficient; degrade visualizer resolution/FPS gracefully on slow terminals.
- Avoid flashy animations that cause excessive CPU usage; prefer adaptive FPS.

## 6) Security and Configuration
- Never store plaintext passwords in config files.
- Store tokens/refresh tokens in OS keyring when possible.
- Avoid logging secrets (tokens, auth headers, passwords).
- Treat offline download/DRM concerns as provider/user responsibility:
  - provider exposes `supports_offline_download` capability
  - download location and cache eviction policy are user configuration options

## 7) Testing Expectations
- Add/maintain tests for logic-heavy code:
  - queue operations and invariants
  - playback state machine
  - provider contract behavior (search + stream URL)
  - scrobbler telemetry handling
- Prefer deterministic tests (mock HTTP; no real network/audio required).
- Keep unit tests fast; integration tests for CLI parsing and wiring.

## 8) Build / CI Discipline
Before proposing a change as “done”, ensure:
- `cargo fmt`
- `cargo clippy -D warnings`
- `cargo test`

## 9) When Editing Multiple Files
- Explain the change set at a high level.
- Keep refactors scoped: “make it compile” first, then improve incrementally.
