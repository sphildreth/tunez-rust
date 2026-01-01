use crate::{config::LoggingConfig, paths::AppDirs};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::fmt::writer::{BoxMakeWriter, MakeWriterExt};
use tracing_subscriber::{fmt, EnvFilter};

pub struct LoggingGuard {
    _file_guard: Option<WorkerGuard>,
}

pub fn init_logging(config: &LoggingConfig, dirs: &AppDirs) -> Result<LoggingGuard, LoggingError> {
    let log_dir = dirs.log_dir().to_path_buf();
    fs::create_dir_all(&log_dir).map_err(|source| LoggingError::CreateDirectory {
        path: log_dir.clone(),
        source,
    })?;

    let env_filter = EnvFilter::try_new(config.level.as_filter_directive()).map_err(|source| {
        LoggingError::ParseLevel {
            level: config.level.as_filter_directive().to_string(),
            source,
        }
    })?;

    let (file_writer, file_guard) = build_file_writer(config, &log_dir)?;
    let writer: BoxMakeWriter = match (config.stdout, file_writer) {
        (true, Some(file)) => BoxMakeWriter::new(
            std::io::stdout
                .with_max_level(tracing::Level::TRACE)
                .and(file),
        ),
        (true, None) => BoxMakeWriter::new(std::io::stdout),
        (false, Some(file)) => BoxMakeWriter::new(file),
        (false, None) => BoxMakeWriter::new(std::io::stdout), // fallback sink to avoid dropping logs silently
    };

    fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_ansi(config.stdout)
        .with_writer(writer)
        .try_init()
        .map_err(LoggingError::SubscriberInstall)?;

    Ok(LoggingGuard {
        _file_guard: file_guard,
    })
}

fn build_file_writer(
    config: &LoggingConfig,
    log_dir: &Path,
) -> Result<(Option<NonBlocking>, Option<WorkerGuard>), LoggingError> {
    let max_files = config.max_log_files.max(1);
    let file_stem = config.file_name.as_deref().unwrap_or("tunez.log");
    cleanup_old_logs(log_dir, file_stem, max_files)?;

    let appender = tracing_appender::rolling::daily(log_dir, file_stem);
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);
    Ok((Some(non_blocking), Some(guard)))
}

fn cleanup_old_logs(dir: &Path, file_stem: &str, max_files: usize) -> Result<(), LoggingError> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|source| LoggingError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(file_stem) {
                entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|mtime| (entry.path(), mtime))
            } else {
                None
            }
        })
        .collect();

    entries.sort_by_key(|(_, modified)| *modified);
    if entries.len() <= max_files {
        return Ok(());
    }

    let remove_count = entries.len() - max_files;
    for (path, _) in entries.into_iter().take(remove_count) {
        fs::remove_file(&path).map_err(|source| LoggingError::Cleanup { path, source })?;
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum LoggingError {
    #[error("failed to create log directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse log level {level}: {source}")]
    ParseLevel {
        level: String,
        source: tracing_subscriber::filter::ParseError,
    },
    #[error("failed to install tracing subscriber: {0}")]
    SubscriberInstall(Box<dyn std::error::Error + Send + Sync>),
    #[error("failed to list log directory {path}: {source}")]
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to remove old log file {path}: {source}")]
    Cleanup {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use crate::config::LogLevel;

    #[test]
    fn filter_directive_is_lowercase() {
        assert_eq!(LogLevel::Info.as_filter_directive(), "info");
    }
}
