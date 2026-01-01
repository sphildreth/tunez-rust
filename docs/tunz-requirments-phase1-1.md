# Tunez — Phase 1 Fix Plan (Consolidated)

**Purpose**: This document consolidates findings from:
- `docs/tunez-requirements-review-grok.md`
- `docs/tunez-requirements-review-qwen.md`
- `docs/tunez-requirements-review-glm47.md`

It is a single actionable “fix list” for a coding agent to execute to bring **Phase 1** to **Definition of Done** per `docs/tunez-requirements.md`.

**Scope rules**
- Implement only what Phase 1 requires; no speculative Phase 2/3 features.
- Preserve existing public APIs unless a PRD requirement forces a change.
- Quality gates are mandatory: `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, and `cargo build --all-features`.

**Key reconciled disagreements across reviews (resolved by code inspection)**
- **OS keyring integration**: Not present in the codebase (no `keyring` usage found; no credential store module). Treat §6.3/§6.5 as **FAIL** until implemented.
- **Log redaction**: Not present (no redaction/sanitization layer found). Treat as **FAIL** until implemented.
- **Player error recovery**: `Player::set_error()` sets error state but does not auto-skip. Treat §4.9 as **PARTIAL** until it logs + shows a user message + skips.
- **Queue persistence**: Not present (queue is in-memory only). Treat §4.6 persistence as **PARTIAL** until implemented.
- **Real audio backend**: `src/tunez-audio/src/real.rs` uses `BufReader<File>` inside `MediaSourceStream`, which is incompatible with Symphonia’s `MediaSource` trait. Treat Phase 1 build gate as **FAIL** until fixed.

---

## 0) Definition of Done (Phase 1)

A coding agent may mark Phase 1 complete only when all of the following are true:
- `cargo fmt --check` passes
- `cargo clippy -- -D warnings` passes
- `cargo test` passes
- `cargo build --all-features` passes
- Required Phase 1 “MUST” items are implemented for:
  - §4.5 Playback (real local playback + controls)
  - §4.6 Queue operations + persistence (persistence is SHOULD, but reviews treat it as required for Phase 1 completion; implement it)
  - §4.9 “invalid track → log + user message + skip”
  - §4.10 scrobbling disabled-by-default + persistence + never interrupts playback (and actually wired)
  - §6.3/§6.5 secure secret handling (keyring) + avoid leaking secrets in logs

---

## 1) Immediate unblockers (must do first)

### 1.1 Fix `cargo clippy -- -D warnings`

**Goal**: clippy is green with `-D warnings`.

**Likely items (from reviews)**
- Dead code warnings in provider crates:
  - `src/providers/filesystem-provider/src/scan.rs` (`scan_library` never used)
  - `src/providers/melodee-provider/src/mapping.rs` (`MelodeePaging` never constructed)
- Useless casts:
  - `src/providers/filesystem-provider/src/tags.rs` (unnecessary `as u32`)

**Acceptance criteria**
- `cargo clippy -- -D warnings` passes.

**Notes**
- Prefer removing dead code over adding `#[allow(dead_code)]` unless it’s clearly part of Phase 1 and about to be used.

---

### 1.2 Fix `cargo build --all-features` (real audio backend compile)

**Goal**: real audio backend compiles (and ideally works for local files).

**Primary file**
- `src/tunez-audio/src/real.rs`

**Known failure mode (from review + code inspection)**
- Symphonia’s `MediaSourceStream::new()` expects a boxed `dyn MediaSource`, but current code passes `BufReader<File>` which does not implement `MediaSource`.

**Suggested fix approach**
- Use `File` directly (which implements `MediaSource`), or use a Symphonia-supported wrapper type.
  - Example shape: `MediaSourceStream::new(Box::new(file), Default::default())`
- Ensure sample format match arms return consistent `Result` types (if/where relevant).

**Acceptance criteria**
- `cargo build --all-features` passes.

---

## 2) Security & privacy MUSTs (Phase 1)

### 2.1 OS keyring storage for tokens/refresh tokens (PRD §6.3/§6.5)

**Goal**: No secrets in config files; tokens persisted securely in OS keyring.

**Current state**
- `src/tunez-core/src/config.rs` contains provider config, but no keyring integration.
- No `keyring` crate usage found.

**Work items**
1. Add a small, provider-agnostic credential store in core (e.g., `tunez-core/src/secrets.rs`):
   - Backed by the `keyring` crate.
   - Namespacing: service name like `tunez` and account keys that include provider + profile (+ username if applicable).
2. Ensure provider auth flows (at least Melodee, if it uses auth) read/write tokens via this store.
3. Ensure config contains only non-secret values (base URLs, usernames), not tokens.

**Acceptance criteria**
- Token/refresh token are never written to `config.toml`.
- On restart, authenticated providers can recover tokens from keyring.
- Failures to access keyring are handled gracefully (no panic; actionable error).

---

### 2.2 Log redaction / “do not log secrets” hardening (PRD §6.5)

**Goal**: prevent accidental logging of tokens or credential-bearing URLs.

**Current state**
- `src/tunez-core/src/logging.rs` sets up tracing, but no redaction layer exists.

**Work items**
1. Define a minimal redaction utility in core (string-based is acceptable for Phase 1):
   - Redact common patterns: `Authorization: Bearer ...`, `token=...`, `access_token`, `refresh_token`, and URLs like `https://user:pass@host/...`.
2. Ensure provider HTTP logging (if any) avoids logging full URLs with query tokens.

**Acceptance criteria**
- Code has a single obvious place to apply redaction.
- Tests exist for redaction of representative strings.

---

## 3) Playback MUSTs (PRD §4.5)

### 3.1 Ensure actual local playback works (not only Null engine)

**Goal**: “Play audio on the local audio device.”

**Current state**
- `tunez-audio` has `NullAudioEngine` and a cpal+symphonia `CpalAudioEngine`.
- `tunez-player` can call `play_with_audio()` but does not manage resume/pause semantics beyond stop.

**Work items**
1. After audio compiles, validate that playing a local file produces audible output.
2. Confirm that play/pause/resume/stop behave as expected (do not block UI loop).

**Acceptance criteria**
- There is a documented “happy path” run command (local file) that plays.

---

### 3.2 Playback controls gaps (seek/volume)

**Note**: Reviews disagree on whether this is already complete. `Player` currently has no seek/volume API. If PRD Phase 1 requires these controls in the current scope, implement minimal support.

**Work items**
- Add volume control and optional seek support end-to-end:
  - `tunez-player` APIs
  - `tunez-audio` handle operations (or emulate if backend can’t seek)
  - `tunez-ui` bindings (only if required by PRD for Phase 1)

**Acceptance criteria**
- Keyboard actions for volume and seek work (or are clearly disabled with a message if backend doesn’t support seek for a given source type).

---

## 4) Queue persistence SHOULD (PRD §4.6) — implement to close Phase 1

### 4.1 Persist queue across restarts with corruption handling

**Current state**
- `src/tunez-player/src/queue.rs` is in-memory only.

**Work items**
1. Add `Queue` persistence to disk in an OS-appropriate data/config directory:
   - Serialize enough state to restore queue order and current selection.
   - Use a stable format (TOML/JSON).
2. Corruption handling (explicit in PRD):
   - If unreadable/corrupt, start empty, show a non-fatal warning, keep corrupt file.
3. Keep a last-known-good backup file (best-effort).

**Acceptance criteria**
- A unit/integration test verifies save/load roundtrip.
- A test verifies corrupt file handling does not crash and results in empty queue.

---

## 5) Error handling MUST (PRD §4.9)

### 5.1 Invalid track workflow: log + user-visible message + skip

**Current state**
- `Player::set_error()` sets `PlayerState::Error` and stops audio.
- No automatic skip-to-next behavior exists.

**Work items**
1. Implement a policy: on decode/unsupported track errors, log error + show a user-visible message + advance to next track.
2. Ensure this is non-blocking and does not panic.
3. Add tests for “error triggers skip” behavior.

**Acceptance criteria**
- Playback does not stall indefinitely on an invalid track.
- UI surfaces a clear error message.

---

## 6) Scrobbling MUST (PRD §4.10)

### 6.1 Wire scrobbler into playback loop/tick

**Current state**
- `tunez-core` defines `Scrobbler` trait and `FileScrobbler` persistence.
- No evidence that the player calls `Scrobbler::submit()` on tick/state changes.

**Work items**
1. Add a 1-second tick (or reuse an existing UI tick) that emits scrobble telemetry while playing.
2. Emit events on state transitions: started/resumed/paused/stopped/ended.
3. Ensure failures never interrupt playback (log + UI indicator only).

**Acceptance criteria**
- With scrobbling enabled, events are produced during playback.
- With scrobbling disabled, no events are produced.

---

### 6.2 Ensure persistence queue is bounded and retried

**Current state**
- `FileScrobbler` exists, but verify it’s truly bounded (count/age) and retried as required by §4.10.3.

**Work items**
- Ensure `FileScrobbler` queue is bounded and old items are pruned.
- On startup, attempt to replay persisted events if configured.

**Acceptance criteria**
- Persistence cannot grow without bound.
- Restart does not lose queued events.

---

## 7) TUI completeness (PRD §5.x)

**Note**: One review claims the TUI matches the mockups and is “PASS”, another claims most views are placeholders and not wired. Treat this as “verify and close gaps” work.

**Work items (only what PRD Phase 1 requires)**
- Confirm Search tab actually performs provider search and renders results.
- Confirm Queue tab renders queue and supports required operations.
- Confirm playback controls are wired to the UI (play/pause/skip, and volume/seek if implemented).
- Confirm error toasts/banners exist for provider/audio errors.

**Acceptance criteria**
- A user can navigate, search, queue items, and control playback entirely from the keyboard.

---

## 8) Performance/responsiveness (PRD §4.3)

**Work items**
- Add at least one deterministic test/benchmark that simulates large result sets (10k tracks) and ensures paging/rendering doesn’t hang.

**Acceptance criteria**
- No UI-blocking operations on large libraries; paging is used.

---

## 9) Verification commands (run in this order)

1. `cargo fmt --check`
2. `cargo clippy -- -D warnings`
3. `cargo test`
4. `cargo build --all-features`

Optional smoke checks:
- `cargo run -p tunez-cli -- --help`
- `cargo run -p tunez-cli -- providers list`

---

## 10) Suggested execution order (for a coding agent)

1. Fix clippy warnings (fast)
2. Fix audio backend compile and get `--all-features` green
3. Implement keyring + log redaction (security MUST)
4. Wire scrobbling tick/events (MUST) + validate persistence/bounds
5. Queue persistence with corruption handling + tests
6. Error skip workflow + UI surfacing
7. Verify TUI wiring for search/queue/playback controls
8. Add minimal performance/responsiveness test(s)

---

## Appendix: File hotspots (from reviews)

- Audio backend: `src/tunez-audio/src/real.rs`
- Config: `src/tunez-core/src/config.rs`
- Logging: `src/tunez-core/src/logging.rs`
- Player: `src/tunez-player/src/player.rs`
- Queue: `src/tunez-player/src/queue.rs`
- Providers:
  - Filesystem provider: `src/providers/filesystem-provider/src/*`
  - Melodee provider: `src/providers/melodee-provider/src/*`
