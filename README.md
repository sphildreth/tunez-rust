# Tunez

Terminal music player in full ANSI color.

![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)

## Status
✅ **Phase 1 (Foundation) - COMPLETE**  
✅ **Phase 2 (External Plugins) - COMPLETE**  
🔄 **Phase 3 (Polish & Extras) - IN PROGRESS**

**Recent Enhancements:**
- ✅ Multiple themes (Default, Monochrome, Afterdark, Solarized)
- ✅ Enhanced visualization modes (Spectrum, Oscilloscope, VU Meter, Particles)
- ✅ Cache management with automatic eviction
- ✅ Offline download support (providers with capability)
- ✅ Full plugin host implementation
- ✅ Comprehensive test coverage

## What is Tunez?

Tunez is a fast, keyboard-first terminal music player with a modular "Provider" architecture (multiple backends), a rich TUI, and real-time audio visualization.

## Quick links

- PRD / Requirements: [docs/tunez-requirements.md](docs/tunez-requirements.md)
- TUI mockups: [docs/tunez-tui-mockups.md](docs/tunez-tui-mockups.md)
- License: [LICENSE](LICENSE)

## Getting Started

### Quick Start
```bash
# Run quality gates
cargo fmt
cargo clippy -- -D warnings
cargo test

# Launch the TUI
cargo run -p tunez-cli
```

### CLI Commands
```bash
# List providers
cargo run -p tunez-cli -- providers list

# Build a play request
cargo run -p tunez-cli -- play --provider filesystem --track "song name" -p

# Launch TUI with specific provider
cargo run -p tunez-cli -- --provider melodee --profile home
```

## Keyboard Shortcuts

### Navigation
- `j/k` or `↑/↓`: Move selection
- `h/l` or `←/→`: Switch tabs
- `Tab/Shift+Tab`: Cycle tabs
- `1-8`: Jump to tab
- `?`: Toggle help
- `q` or `Esc`: Quit

### Playback
- `Space`: Play/Pause
- `n/p`: Next/Previous
- `←/→`: Seek

### Customization
- `v`: Cycle visualization modes
- `t`: Cycle themes

## Configuration

Default: `${CONFIG_DIR}/tunez/config.toml`

```toml
config_version = 1
default_provider = "filesystem"
theme = "afterdark"

[cache]
max_size_bytes = 10737418240  # 10 GB
max_age_seconds = 2592000     # 30 days

[providers.filesystem.profiles.default]
library_root = "./music-library"
```

## Architecture

### Providers
- **Filesystem**: Local files with metadata caching
- **Melodee**: Remote API with authentication
- **Plugin**: External executables via JSON protocol

### Scrobbling
- Opt-in, non-blocking
- Persistent queue with retry
- Multiple backends supported

### Visualization
- Spectrum analyzer
- Oscilloscope
- VU Meter
- Particle system

## Testing

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

All tests pass with comprehensive coverage.

## License

MIT - see LICENSE file
