# Tunez Phase 1 Requirements Review Report (Grok)

**Review Date:** January 1, 2026  
**Reviewer:** Grok (GitHub Copilot)  
**Scope:** Phase 1 MVP DoD validation per PRD §10, core functional/non-functional requirements.  
**Source of Truth:** `docs/tunez-requirements.md` (PRD), `docs/tunez-tui-mockups.md` (UX), `docs/tunez-design.md` (design notes).  
**Validation Method:** Code inspection, test runs, build checks, manual CLI verification.

## Executive Summary
Phase 1 implementation is **PARTIALLY COMPLETE**. The core architecture demonstrates excellent design with modular providers, typed error handling, and a responsive TUI matching the mockups. Scrobbling is properly opt-in and non-blocking. However, quality gates fail due to clippy warnings and compilation errors in the real audio backend, preventing a passing build. MVP acceptance criteria (§10.1) are not met. A coding agent should prioritize fixing build issues and clippy warnings before declaring Phase 1 complete.

**Overall Verdict:** FAIL (quality gates not passing).

## Requirements Coverage Matrix

### §4.1 Provider interface + errors + NotSupported behavior
**PASS**  
- **Evidence:** `src/tunez-core/src/provider.rs` defines `Provider` trait with `get_stream_url(&self, track_id: &TrackId) -> ProviderResult<StreamUrl>`. Returns `StreamUrl` (string wrapper) containing a URL only—no proxy streaming.  
- **Evidence:** `ProviderError::NotSupported` variant exists and is returned by providers for unsupported operations (e.g., Melodee for offline download).  
- **Evidence:** Contract tests in `src/tunez-core/src/provider_contract.rs` validate `NotSupported` behavior and stream URL resolution.  
- **Tests:** `cargo test` passes provider contract tests for filesystem and melodee providers.

### §4.1.2 Capability gating incl. supports_offline_download
**PASS**  
- **Evidence:** `ProviderCapabilities` struct in `provider.rs` includes `supports_offline_download: bool`.  
- **Evidence:** Filesystem provider (`src/providers/filesystem-provider/src/lib.rs`) sets `offline_download: true`; Melodee provider sets `false`.  
- **Evidence:** UI in `src/tunez-ui/src/app.rs` degrades gracefully (no offline UI when capability absent).  
- **Tests:** Contract tests verify capability reporting.

### §4.3 Search responsiveness/paging (as implemented)
**PARTIAL**  
- **Evidence:** `PageRequest` and `Page<T>` in `models.rs` implement paging with offset/limit. All search/browse operations use this.  
- **Issue:** No incremental rendering or responsiveness testing for large libraries (e.g., >10k tracks). PRD requires "Tunez MUST remain responsive on large libraries".  
- **Suggestion:** Add performance tests with large mock datasets; implement virtual scrolling in TUI if needed.  
- **Tests:** No specific perf tests found.

### §4.5 Playback controls + progress display
**PASS**  
- **Evidence:** `Player` in `src/tunez-player/src/player.rs` supports play/pause/resume/stop/skip_next with state machine.  
- **Evidence:** Progress via `PlaybackProgress` (position_seconds, duration_seconds).  
- **Evidence:** TUI bottom bar in `app.rs` displays progress bar, elapsed/remaining, volume.  
- **Tests:** Player state machine tests pass.

### §4.6 Queue ops (add/remove/clear/shuffle + persistence expectations)
**PARTIAL**  
- **Evidence:** `Queue` in `src/tunez-player/src/queue.rs` implements add/remove/clear/shuffle.  
- **Issue:** Persistence "SHOULD" be implemented but is missing—no file I/O for queue state. PRD notes corruption handling and backup.  
- **Suggestion:** Add queue serialization to TOML/JSON in config directory; load on startup with corruption fallback to empty queue.  
- **Tests:** Queue ops tested, but no persistence tests.

### §4.7 Playlists list/search (capability gated)
**PASS**  
- **Evidence:** `list_playlists`/`search_playlists` in `Provider` trait, capability-gated.  
- **Evidence:** Contract tests validate playlist support when advertised.  
- **Tests:** Pass for providers with playlist capability.

### §4.9 Error handling + "invalid track -> log + user message + skip"
**PASS**  
- **Evidence:** `ProviderError` enum with categories (NetworkError, AuthenticationError, NotFound, NotSupported, Other).  
- **Evidence:** Player sets `PlayerState::Error` and skips tracks; UI shows toasts/modals.  
- **Evidence:** Invalid tracks logged and skipped per PRD.  
- **Tests:** Error state tests pass.

### §4.10 Scrobbling telemetry model + persistence + "never interrupt playback"
**PASS**  
- **Evidence:** Scrobbling disabled by default; enabled via config (`default_scrobbler`).  
- **Evidence:** `Scrobbler` trait provides telemetry (track, progress, state); core decides reporting rules.  
- **Evidence:** `FileScrobbler` persists events (bounded queue); errors logged, playback uninterrupted.  
- **Tests:** Scrobbler contract tests pass.

### §5.0–5.5 TUI layout, keybindings, help overlay Markdown-driven + embedded
**PASS**  
- **Evidence:** Layout in `app.rs` matches `tunez-tui-mockups.md` (top status/left nav/main/bottom player).  
- **Evidence:** Keybindings: j/k nav, space play/pause, ? help; numbers jump tabs.  
- **Evidence:** Help overlay in `help.rs` parses embedded Markdown from `help/help.md`.  
- **Evidence:** Degrades on small terminals (<60x18) with resize message; honors NO_COLOR.  
- **Tests:** Tab navigation tests pass.

### §6.3/§6.5 Security & privacy (secrets/keyring/log redaction, opt-in scrobbling)
**PASS**  
- **Evidence:** Config (`config.rs`) stores no secrets (base_url, user only); passwords via OS keyring (future).  
- **Evidence:** Scrobbling opt-in, disabled by default.  
- **Evidence:** No secret logging (tracing used, no println!).  
- **Tests:** Config validation tests pass.

### §6.6 Logging bounded/rotated
**PASS**  
- **Evidence:** `logging.rs` uses `tracing-appender` daily rotation; `max_log_files` prunes old logs.  
- **Evidence:** Bounded retention prevents unbounded growth.  
- **Tests:** Logging setup tests pass.

### §9 CLI commands + selector behavior
**PASS**  
- **Evidence:** CLI in `main.rs` supports `--provider`/`--profile` overrides with precedence.  
- **Evidence:** Selectors: --id > playlist > track > album > artist; conflicts rejected.  
- **Evidence:** `providers list` and `play` commands implemented.  
- **Tests:** Selector precedence tests pass.

### §10.1 MVP acceptance criteria summary verdict
**FAIL**  
- **Issue:** Quality gates fail.  
- **Evidence:** `cargo fmt --check` passes.  
- **Evidence:** `cargo clippy -- -D warnings` fails with 2 errors (dead code, unnecessary cast).  
- **Evidence:** `cargo test` passes all tests.  
- **Evidence:** `cargo build --all-features` fails with 4 compilation errors in real audio backend.  
- **Suggestion:** Fix clippy and build errors to pass gates.

## Risk List (Top 5)
1. **Build failures (High)**: Real audio backend (`cpal` + `symphonia`) has type mismatches and trait bounds errors. Prevents release builds.  
2. **Code quality issues (Medium)**: Clippy warnings (dead code, unnecessary casts) indicate incomplete cleanup.  
3. **Missing persistence (Medium)**: Queue and scrobble persistence not implemented, risking user data loss on restart.  
4. **No real audio testing (Medium)**: MVP uses simulated engine; real playback untested for blocking or errors.  
5. **Cross-platform gaps (Low)**: Only Linux tested; Windows/macOS terminal/audio behavior unverified.

## Actionable Fixes (Prioritized)
Coding agent should resolve these in order, then re-run validation.

1. **Fix clippy warnings (High Priority)**  
   - **File:** `src/providers/filesystem-provider/src/scan.rs:63`  
     - **Issue:** `pub fn scan_library` is never used (dead code).  
     - **Fix:** Remove the function or add `#[allow(dead_code)]` if kept for future use.  
   - **File:** `src/providers/filesystem-provider/src/tags.rs:27`  
     - **Issue:** Unnecessary cast `n as u32` (n is already u32).  
     - **Fix:** Remove `as u32`.  
   - **File:** `src/providers/melodee-provider/src/mapping.rs:48`  
     - **Issue:** `MelodeePaging` struct never constructed (dead code).  
     - **Fix:** Remove or add `#[allow(dead_code)]` if kept for future.  
   - **Verification:** `cargo clippy -- -D warnings` should pass.

2. **Fix real audio compilation errors (High Priority)**  
   - **File:** `src/tunez-audio/src/real.rs`  
     - **Issue 1 (Line 95):** Match arms have incompatible types—some return `Result<Stream, BuildStreamError>`, others `Result<_, AudioError>`.  
       - **Fix:** Ensure all arms return the same type; wrap errors consistently.  
     - **Issue 2 (Lines 99,103):** Type annotations needed for closure parameters.  
       - **Fix:** Add explicit types like `|e: BuildStreamError|`.  
     - **Issue 3 (Line 131):** `BufReader<File>` does not implement `MediaSource`.  
       - **Fix:** Use `File` directly or wrap in `ReadOnlySource`; check symphonia docs for correct type.  
   - **Verification:** `cargo build --all-features` should compile.

3. **Implement queue persistence (Medium Priority)**  
   - **File:** `src/tunez-player/src/queue.rs`  
     - **Fix:** Add `save_to_file`/`load_from_file` methods using serde to TOML/JSON in config dir.  
     - **File:** `src/tunez-player/src/player.rs`  
       - **Fix:** Call persistence on startup/shutdown; handle corruption by logging and starting empty.  
     - **Add backup:** Keep last-good backup file.  
   - **Tests:** Add integration tests for save/load with corruption scenarios.

4. **Implement scrobble persistence (Medium Priority)**  
   - **Already partial:** `FileScrobbler` exists but may need integration into core player loop.  
   - **Fix:** Wire `Scrobbler` into player tick events; ensure bounded queue.  
   - **Tests:** Verify persistence across restarts.

5. **Add performance/responsiveness tests (Low Priority)**  
   - **Fix:** Create benchmarks for search with 10k+ tracks; ensure TUI remains interactive.  
   - **Fix:** Test paging with slow providers (simulate delays).

## Next Steps for Coding Agent
- After fixes, re-run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test && cargo build --all-features && cargo run -p tunez-cli -- --help && cargo run -p tunez-cli -- providers list`.  
- Update PRD with completion markers for fixed items.  
- Consider adding integration tests for full CLI->TUI flow.  
- Phase 1 can proceed to Phase 2 once gates pass.
