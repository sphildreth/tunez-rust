# Tunez Design — Phase 1A

## Architecture
- Cargo workspace with two crates:
  - `tunez-core`: shared utilities (config loading/validation, paths discovery, logging bootstrap).
  - `tunez-cli`: binary entrypoint that loads config, initializes logging, and will later launch CLI/TUI.

## Data Flow
1. `tunez-cli` discovers application directories via `AppDirs`.
2. `tunez-core::Config::load_or_default` reads `config.toml` (TOML + serde); missing files use defaults.
3. Logging is initialized with `tracing`/`tracing-subscriber`, emitting to stdout (optional) and to a daily-rotated file in `data/logs/`.
4. Old log files beyond `max_log_files` are pruned to satisfy bounded retention.

## Interfaces
- `Config::load_or_default(dirs: &AppDirs) -> Result<Config, ConfigError>`: loads + validates configuration.
- `init_logging(config: &LoggingConfig, dirs: &AppDirs) -> Result<LoggingGuard, LoggingError>`: installs tracing subscriber with retention.
- `AppDirs::discover() -> Result<AppDirs, DirsError>`: OS-appropriate config/data/cache/log directories.

## Error Handling
- Config validation enforces `config_version`; mismatches produce `ValidationError::UnsupportedVersion`.
- IO and parse errors surface clear paths; directory creation failures bubble via `DirsError`.
- Logging init errors are explicit (directory creation, level parsing, subscriber installation, cleanup failures).

## Testing
- Unit tests cover directory discovery assumptions, config defaults/validation, and log level directives.
- Workspace tests run via `cargo test`; future phases will add integration/contract tests.
