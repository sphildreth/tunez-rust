# Tunez Requirements Review - Phase 1

**Date:** 2026-01-02  
**Version:** Phase 1 (Built-in Providers)  
**Reviewer:** Antigravity (Assistant)

## 1. Executive Summary

Phase 1 is **COMPLETE**. The codebase has been reviewed against requirements, and identified issues with scrobbling initialization and potential thread-safety/runtime panics have been resolved. The architecture is robust, and all MVP acceptance criteria are met.

**Verdict:** **PASS**
*   **PASSED**: Architecture, Provider Contract, TUI Layout/Responsiveness, Security (Keyring/Redaction), CLI Wiring, Scrobbling, Async Runtime integration.
*   **RESOLVED**: Scrobbling initialization wiring, TUI thread unboundedness (moved to `tokio::task::spawn_blocking`), and CLI runtime entry point (`#[tokio::main]`).
*   **NOTE**: "MVP Acceptance Criteria" are 100% met.

---

## 2. Requirements Coverage Matrix

| Section | Requirement | Status | Evidence/Notes |
| :--- | :--- | :--- | :--- |
| **§4.1** | **Provider Interface** | **PASS** | `Provider` trait (tunez-core) enforces strict contract. `StreamUrl` wrapper ensures URL-only return. Typed `ProviderError` allows consistent UI handling. |
| **§4.1.2** | **Capability Gating** | **PASS** | `ProviderCapabilities` struct fully implemented. `FilesystemProvider` correctly reports capabilities (offline_download=true, playlists=gated). |
| **§4.2** | **Connection/Auth** | **PASS** | `secrets.rs` implements OS keyring storage. `redact.rs` ensures logs are clean. Configuration separates secrets from config files. |
| **§4.3** | **Search Responsiveness** | **PASS** | `tunez-ui/src/app.rs` now uses `tokio::task::spawn_blocking` to offload search, ensuring responsiveness and bounded resource usage. |
| **§4.5** | **Playback Controls** | **PASS** | `Player` state machine handles Play/Pause/Stop/Error. `tunez-cli` wiring connects `AudioEngine`. |
| **§4.6** | **Queue Management** | **PASS** | `Queue` struct supports add/remove/shuffle. `QueuePersistence` loads/saves state on startup/shutdown. |
| **§4.7** | **Playlists** | **PASS** | `FilesystemProvider` implements `list_playlists`, `get_playlist`. UI has `Playlists` tab wired to provider calls. |
| **§4.9** | **Error Handling** | **PASS** | `handle_track_error` logs, notifies UI callback (toast), and skips to next track automatically. No panic paths found. |
| **§4.10** | **Scrobbling Telemetry** | **PASS** | Fixed wiring bug in `App::new`. CLI now initializes Tokio runtime, preventing panics in `ScrobblerManager`'s background tasks. |
| **§5.0** | **TUI Layout** | **PASS** | strict 80x24 check implemented. Tabs match mockups (Now Playing, Search, etc.). Help overlay present. |
| **§5.2.1** | **Help Overlay** | **PASS** | `help.rs` embeds `help.md` binary. Markdown parsing implemented. |
| **§6.3** | **Security/Privacy** | **PASS** | Secrets in keyring. Logs redacted. Scrobbler defaults to disabled unless configured. |
| **§9** | **CLI Selectors** | **PASS** | `PlaySelector` precedence enforced (ID > Playlist > Track). `tunez play --id` wins. |
| **§10.2** | **Quality Gates** | **PASS** | `cargo fmt`, `clippy`, `test` all pass, including async tests for TUI app logic. |

---

## 3. Resolved Issues

### 1. Scrobbling Wiring Bug (RESOLVED)
*   **Issue**: `scrobbler_manager.set_enabled(scrobbler_manager.is_active())` was returning false because the manager defaulted to disabled.
*   **Fix**: Updated `App::new` to use `scrobbler_manager.set_enabled(ctx.scrobbler.is_some())`.

### 2. Runtime Panic Risk (RESOLVED)
*   **Issue**: `ScrobblerManager` uses `tokio::spawn`, but `tunez-cli`'s main function was synchronous, which would cause a runtime panic.
*   **Fix**: Added `tokio` dependency to `tunez-cli` and annotated `main` with `#[tokio::main]`.

### 3. TUI Thread Spawning (RESOLVED)
*   **Issue**: Unbounded `std::thread::spawn` usage in `App` methods.
*   **Fix**: Switched to `tokio::task::spawn_blocking` to use a managed blocking thread pool, reducing resource exhaustion risks. Added `tokio` to `tunez-ui` dependencies and updated tests.

---

## 4. MVP Acceptance Verdict

*   Provide selection: **YES**
*   Built-in Providers (FS + Melodee): **YES**
*   Search & Play: **YES**
*   Scrobbling works: **YES** (Wiring fixed, runtime support added)
*   Playlists: **YES**
*   Queue/Controls: **YES**
*   Visualizer: **YES**
*   Cross-platform: **YES**

**Conclusion**: The project is **PASS**. All concerns have been addressed.

---

## 5. Next Steps

*   Proceed to Phase 2 planning/implementation.
