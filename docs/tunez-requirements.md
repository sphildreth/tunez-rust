# Tunez TUI Player — Requirements (PRD)

## 1. Overview

**Product name:** `tunez` (Melodee After Dark)
**Product tagline:** “Terminal music player in full ANSI color.”
**Type:** Cross‑platform CLI + terminal UI (TUI) music player  
**Platforms:** Linux, macOS, Windows (native terminal)  
**Back-end:** Plugin style with sources for local music and Melodee streaming music server (HTTP API defined by the provided OpenAPI spec)

### 1.1 Problem statement
You want a *fast, keyboard-first, colorful* terminal player that can browse/search your library and play audio from your streaming music server—while also being “fun”: smooth transitions, animated UI widgets, and a real-time spectrum/waveform visualization.

### 1.2 Why a TUI instead of a GUI?
- Runs anywhere: SSH, tmux, headless boxes, minimal desktops
- “Always there” experience like `ncmpcpp`, but modern/animated
- Fun engineering challenge (audio pipeline + rendering loop + async networking)

---

## 2. Goals and Non-Goals

### 2.1 Goals (v1)
1. **Play audio from the server** (streaming over HTTP) with robust buffering and minimal stutter.
2. **A rich TUI**: browsing, searching, queue/now-playing, progress, volume, shuffle/repeat.
3. **Color + animation**: smooth progress bar, transitions between views, loading spinners.
4. **Spectrum / waveform visualization** synchronized with playback (at least a spectrum analyzer).
5. **Cross-platform**: consistent behavior on Linux/macOS/Windows terminals.
6. **Server-first workflow**: use the Melodee API to search, fetch metadata, playlists, etc.

### 2.2 Non-goals (v1)
- Local library management (file tagging, local scanning)
- Multi-room audio / casting
- Editing metadata on the server (unless already supported and easy)
- A full “Spotify client” feature set (social, collaborative playlists, etc.)

---

## 3. Users and Use Cases

### 3.1 Primary user persona
- Power user who lives in terminals (tmux, SSH)
- Wants “no-mouse” control and a slick now-playing experience
- Already has a music server and wants a terminal-native player

### 3.2 Core user stories
- As a user, I can **connect** to my Melodee server and stay logged in.
- As a user, I can **search** songs and start playback immediately.
- As a user, I can **browse** albums/artists/playlists and queue items.
- As a user, I can **see what’s playing** and control playback with hotkeys.
- As a user, I can **enjoy a spectrum/waveform animation** while music plays.
- As a user, I can **resume** where I left off (queue + position, if supported) or at least restore my last view/state.

---

## 4. Functional Requirements

### 4.1 Connection & Authentication
**Must**
- Configure:
  - Server base URL (e.g., `https://music.example.com`)
  - Credentials (email/password) OR another supported auth flow
- **Security Critical**: Do NOT store passwords in the config file. Use the OS keyring (Keychain, Credential Manager, Secret Service) to store the Refresh Token.
- Authenticate and store tokens securely (platform keychain where possible).
- Refresh tokens automatically (if refresh tokens exist) and retry requests on auth expiry.

**Should**
- Support multiple server profiles (e.g., prod/home/lab).
- Support “device ID” / “client name” for server-side auditing.

**Nice-to-have**
- OAuth login flows (Google) using an out-of-band/browser flow.

---

### 4.2 Library Search and Discovery
**Must**
- Search songs by text query.
- Show paged results and allow keyboard navigation.

**Should**
- Advanced search filters (artist/album/year/genre) if supported by API.
- Search albums/artists/playlists too.

**Nice-to-have**
- Recommendations / charts views.

---

### 4.3 Browsing Views
**Must**
- Views (tabs) for:
  - **Now Playing**
  - **Queue**
  - **Search**
  - **Playlists**
- Detail panels for current selection (duration, artist, album, etc.).

**Should**
- Views for:
  - Albums
  - Artists
  - Liked/Top-rated/Recently played (if supported)

---

### 4.4 Playback
**Must**
- Stream audio via HTTP and play on the local audio device.
- Playback controls:
  - Play/Pause
  - Next/Previous
  - Seek ±5s/±30s
  - Volume up/down + mute
- Progress display:
  - elapsed / remaining
  - buffered indicator (optional, but recommended)

**Should**
- Gapless-ish playback (best-effort) by pre-buffering the next track.
- Multiple output device selection (where feasible).

**Nice-to-have**
- ReplayGain / loudness normalization (if metadata is available).
- Equalizer presets (if client-side EQ is implemented).
- OS-level media controls (MPRIS on Linux, SMTC on Windows) so media keys work when the terminal is unfocused.

---

### 4.5 Queue Management
**Must**
- A local playback queue inside the client:
  - Add track(s) to end / play next
  - Remove item(s)
  - Clear queue
  - Shuffle queue
- Queue persists in local app state between runs (optional for v1, but helpful).

**Should**
- If the server supports queue endpoints: sync queue state (server ↔ client).
- Support “radio mode” (auto-append recommendations based on current song).

---

### 4.6 Playlists
**Must**
- List user playlists.
- Open a playlist, list its songs, and play/queue them.

**Should**
- Create / rename / delete playlists (if supported).
- Add/remove songs.
- Reorder playlist.

---

### 4.7 Lyrics
**Should**
- Fetch and display lyrics for the current song when available.
- Provide scrolling and “follow along” display (nice-to-have).

---

### 4.8 Scrobbling / Play Events
**Should**
- Notify server when:
  - playback starts
  - playback ends
  - position checkpoints (e.g., every 30s or 50% progress)
- Handle offline/temporary server disconnect by batching play events.

---

### 4.9 Errors, Offline, and Resilience
**Must**
- Graceful error states in the UI (banner/toast).
- Network timeouts and retries with exponential backoff.
- If stream fails mid-track, allow retry or skip.

**Should**
- Cache metadata (songs/albums/playlists) to speed up navigation.
- “Offline mode” for browsing cached metadata (not necessarily playing).

---

## 5. TUI/UX Requirements

### 5.1 Visual layout
**Baseline layout**
- Top bar: connection status, server profile, search box (contextual)
- Left sidebar: tabs / navigation
- Main pane: lists (songs/albums/playlists) or lyrics/details
- Bottom player bar:
  - track info
  - progress + time
  - transport icons (ASCII/Unicode)
  - volume
  - status indicators (shuffle/repeat)

### 5.2 Input model
**Must**
- Keyboard-first navigation with vim-ish defaults (customizable):
  - `j/k` navigate
  - `Enter` open/play
  - `Space` play/pause
  - `n/p` next/prev
  - `/` search
  - `q` back/close
  - `?` help overlay
- Fully rebindable keys via config file.

**Should**
- Mouse support (scroll, click) if feasible.

### 5.3 Color + themes
**Must**
- 24-bit color support when terminal supports it.
- A default theme with high contrast and tasteful accent colors.

**Should**
- Theme packs (TOML/YAML) and runtime switching.

### 5.4 Animations
**Must**
- Smooth progress bar (updates at ~20–60 FPS depending on terminal).
- Animated loading states (spinners).
- Spectrum analyzer animation while audio plays.

**Should**
- View transitions (subtle fades/slide effects via redraw patterns).
- Beat-synced “pulse” on now-playing elements (optional).

### 5.5 Accessibility
**Must**
- Work in monochrome terminals (fallback colors).
- No reliance on emoji for core meaning.

---

## 6. Non-Functional Requirements

### 6.1 Performance
- Startup time: < 1s to show UI (assuming config exists).
- UI should remain responsive while streaming/decoding.
- CPU target: “reasonable” on low-power machines; visualization should degrade gracefully.

### 6.2 Reliability
- Avoid crashes on malformed metadata.
- Robust handling of audio device changes/unavailable device.

### 6.3 Security
- TLS by default (https).
- Tokens stored securely:
  - Keychain on macOS, Credential Manager on Windows, Secret Service/libsecret on Linux when available.
  - File fallback only if explicitly allowed.

### 6.4 Portability / distribution
- `cargo install mad`
- Prebuilt binaries for the 3 platforms (GitHub Releases). Binary name: `mad` (Linux/macOS), `mad.exe` (Windows).
- Minimal external dependencies (avoid requiring ffmpeg if possible)

---

## 7. Technical Approach (Rust Stack)

### 7.1 TUI stack
- **ratatui** (modern tui crate) + **crossterm** (cross-platform terminal IO)
- A central `AppState` + message/event bus model

### 7.2 Async + HTTP
- **tokio** runtime
- **reqwest** + **serde** for API calls and models
- Resilience: `tower` retry policies (optional) or custom backoff

### 7.3 Audio playback pipeline
Recommended path (pure Rust):
- **symphonia** for decoding common formats (MP3, FLAC, AAC, etc.)
- **cpal** for cross-platform audio output. *Critical Note:* Ensure visualization data is synchronized with the playback cursor, not the decoding cursor, to prevent visuals appearing "ahead" of the audio.
- Alternative: **rodio** (higher-level wrapper) if it exposes enough hook points for visualization, which simplifies device management significantly.

### 7.4 Spectrum / waveform visualization
- Capture decoded PCM frames before they are written to the audio device.
- Use a ring buffer (lock-free) to share samples with the UI thread/task.
- FFT via **rustfft** (or a small DFT for low-res “bars” mode).
- Visual modes:
  - Spectrum bars
  - Oscilloscope line
  - “VU meter” fallback for slow terminals

### 7.5 CLI + configuration
- **clap** for CLI flags/subcommands
- **directories** crate to resolve standard paths (XDG on Linux, AppData on Windows, Library on macOS). Do not hardcode `~/.config`.
- `tracing` + `tracing-subscriber` for structured logs

---

## 8. High-Level Architecture

### 8.1 Core modules
- `api/` — typed API client, auth refresh, paging helpers
- `player/` — playback engine (state machine), queue, transport controls
- `audio/` — stream reader, decoder, output, buffer management
- `viz/` — spectrum/waveform computation
- `ui/` — ratatui views, layout, theming, input bindings
- `storage/` — config, secure token store, cache

### 8.2 Data flow (playback)
1. User selects a song → app resolves a **stream URL**
2. `audio::stream_reader` pulls bytes over HTTP
3. `audio::decoder` converts into PCM frames
4. `audio::output` writes to device
5. `viz` receives a copy of PCM frames and computes FFT buckets
6. `ui` renders current state at a configured refresh rate

---

## 9. Definition of Done (v1)

### 9.1 MVP acceptance criteria
- Login + persistent session
- Search songs, pick one, it plays
- Queue view works (add/remove/reorder locally)
- Basic player controls + progress/volume
- A working spectrum visualization (even low-res bars)
- Works on:
  - Linux (Alacritty, GNOME Terminal)
  - macOS (Terminal / iTerm2)
  - Windows (Windows Terminal)

### 9.2 Quality gates
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test`
- Minimal crash rate: no panics on normal usage
- Audio does not stutter on normal network conditions (best effort)

---

## 10. Roadmap

### Phase 0 — Skeleton
- Project scaffolding, UI shell, config, logging
- API client basic request/response types

### Phase 1 — Playback MVP
- Auth + search + play
- Queue + now playing + basic controls
- Minimal spectrum bars

### Phase 2 — Library power features
- Playlists (list/open/queue)
- Lyrics view
- Better caching and paging UX

### Phase 3 — “Fun club”
- Better visualization modes + theme editor
- Animations/transitions
- Scrobbling and robust play-event sync

---

## 11. Risks and Mitigations

- **Cross-platform audio quirks:** keep audio backend modular; add device selection and fallback.
- **Terminal performance:** use adaptive FPS; degrade visualization resolution under load.
- **Network jitter:** implement buffering and a “reconnect and resume” strategy where feasible.
- **Auth uncertainty:** support multiple auth strategies and token refresh.

---

## 12. Open Questions
1. Preferred login method(s) for the client (password vs token vs external OAuth)?
2. Does the server offer “remote queue sync” endpoints you want to use, or is local-only OK?
3. Should the client support downloading/caching tracks for offline playback?
4. What’s the minimum visualization fidelity you want (bars vs waveform vs both)?

---

## 13. Implementation Details for Coding Agents

### 13.1 API Integration Examples
Assuming the Melodee API follows a standard RESTful design with the provided OpenAPI spec, key endpoints to implement include:

- **Authentication**: `POST /auth/login` with JSON body `{"email": "user@example.com", "password": "pass"}` returning `{"token": "jwt_token", "refresh_token": "refresh_jwt"}`.
- **Search**: `GET /search?q={query}&type=song&limit=50&offset=0` returning paginated results. *Implementation Note:* If implementing "search-as-you-type", enforce a debounce (e.g., 300ms) and cancel stale requests to avoid UI lag and API spam.
- **Playlists**: `GET /playlists` for list, `GET /playlists/{id}/tracks` for contents.
- **Stream**: Direct HTTP GET to `stream_url` for audio data (e.g., MP3/FLAC), with Range headers for seeking.

Use `reqwest` with `serde` for deserialization. Implement token refresh by checking response status 401 and retrying with refreshed token.

### 13.2 Configuration Strategy

**File Location**:
Use the `directories` crate to resolve the platform-standard configuration directory (e.g., `~/.config/mad/config.toml` on Linux, `%APPDATA%\mad\config.toml` on Windows, `~/Library/Application Support/mad/config.toml` on macOS).
*Note: The application binary name is `mad` (or `mad.exe` on Windows), so configuration folders should default to `mad`.*

**Precedence Order (Highest to Lowest)**:
1.  **CLI Arguments** (e.g., `mad --server https://music.local`)
2.  **Environment Variables** (e.g., `MELODEE_SERVER_URL`, `MELODEE_EMAIL`)
3.  **Config File** (`config.toml`)
4.  **Hardcoded Defaults**

**Example `config.toml`**:
```toml
[server]
base_url = "https://music.example.com"
profile = "prod"

[auth]
email = "user@example.com"
# Note: Passwords/Tokens are NOT stored here. Use OS Keyring.

[ui]
theme = "default"
fps = 30
keybindings = { play_pause = "Space", next = "n", prev = "p" }

[audio]
buffer_size_ms = 2000
device = "default"
```

Load using the `config` crate, which supports layering these sources automatically.

### 13.3 Audio Pipeline Pseudocode
Core playback loop using `tokio`:

```rust
async fn playback_loop(app_state: Arc<Mutex<AppState>>) {
    loop {
        let current_track = app_state.lock().await.queue.current();
        if let Some(track) = current_track {
            let stream = reqwest::get(&track.stream_url).await?;
            let decoder = symphonia::default::get_probe().format(
                &Default::default(),
                symphonia::core::io::MediaSourceStream::new(Box::new(stream), Default::default()),
                &Default::default(),
                &Default::default(),
            )?;
            
            let mut output = cpal::default_host().default_output_device().build_output_stream(
                &cpal::StreamConfig::default(),
                move |data: &mut [f32], _| {
                    // Decode and fill buffer
                    // Also feed viz_ring_buffer for visualization
                },
                |err| eprintln!("Audio error: {:?}", err),
            )?;
            
            output.play()?;
            // Handle controls, seeking, etc.
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

### 13.4 Visualization Implementation
For spectrum: Use `rustfft` on PCM samples from a ring buffer. Compute FFT every frame, map frequencies to bars (e.g., 32 bins). Render with ratatui blocks.

```rust
fn compute_spectrum(samples: &[f32]) -> Vec<f32> {
    let mut planner = rustfft::FftPlanner::new();
    let fft = planner.plan_fft_forward(samples.len());
    let mut buffer: Vec<Complex<f32>> = samples.iter().map(|&s| Complex::new(s, 0.0)).collect();
    fft.process(&mut buffer);
    buffer.iter().take(samples.len() / 2).map(|c| c.norm()).collect()
}
```

### 13.5 Error Handling and Resilience
- Network errors: Use `tower::retry` with exponential backoff (initial 1s, max 30s).
- Audio failures: Log and skip to next track; show toast notification in UI.
- Auth expiry: Intercept 401, refresh token, retry request.
- Offline: Cache metadata with `sled` or `rusqlite`; show cached views with "offline" indicator.

### 13.6 Testing Strategy
- Unit tests: Mock API responses with `mockito`; test decoder with sample audio files.
- Integration tests: Spin up local server mock; test full playback flow.
- UI tests: Use `crossterm` simulation for key inputs; assert rendered output.
- Performance: Benchmark startup time, memory usage during playback.

### 13.7 Dependencies and Versions
Pin in `Cargo.toml`:

```toml
[dependencies]
ratatui = "0.26"
crossterm = "0.27"
tokio = { version = "1.0", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
symphonia = { version = "0.5", features = ["mp3", "flac"] }
cpal = "0.15"
rustfft = "6.1"
config = "0.14"
tracing = "0.1"
directories = "5.0"
keyring = "2" # For secure token storage
souvlaki = "0.7" # For OS media controls (MPRIS/SMTC)
```

### 13.8 Build and Deployment
- CI: Use GitHub Actions with `cargo build --release` for Linux/macOS/Windows.
- Packaging: Use `cargo-dist` for binaries; include config template.
- Distribution: Upload to GitHub Releases; consider package managers (e.g., `brew` for macOS).

This section provides actionable details to accelerate implementation while keeping the PRD focused on requirements.
