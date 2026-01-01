use crate::paths::AppDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

const CURRENT_CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_config_version")]
    pub config_version: u32,
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub default_scrobbler: Option<String>,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: default_config_version(),
            default_provider: None,
            profile: None,
            default_scrobbler: None,
            logging: LoggingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: LogLevel,
    #[serde(default = "default_max_log_files")]
    pub max_log_files: usize,
    #[serde(default = "default_stdout_enabled")]
    pub stdout: bool,
    #[serde(default)]
    pub file_name: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            max_log_files: default_max_log_files(),
            stdout: default_stdout_enabled(),
            file_name: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_filter_directive(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse config at {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("config validation failed: {0}")]
    Validation(ValidationError),
    #[error("failed to prepare configuration directories: {0}")]
    Directories(#[from] crate::paths::DirsError),
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("unsupported config_version {found}, expected {expected}")]
    UnsupportedVersion { found: u32, expected: u32 },
}

impl Config {
    pub fn load_or_default(dirs: &AppDirs) -> Result<Self, ConfigError> {
        dirs.ensure_exists()?;
        let path = Self::config_path(dirs);
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path).map_err(|source| ConfigError::Io {
            path: path.clone(),
            source,
        })?;
        let config: Config = toml::from_str(&contents).map_err(|source| ConfigError::Parse {
            path: path.clone(),
            source,
        })?;
        config.validate().map_err(ConfigError::Validation)?;
        Ok(config)
    }

    pub fn config_path(dirs: &AppDirs) -> PathBuf {
        dirs.config_dir().join("config.toml")
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.config_version != CURRENT_CONFIG_VERSION {
            return Err(ValidationError::UnsupportedVersion {
                found: self.config_version,
                expected: CURRENT_CONFIG_VERSION,
            });
        }
        Ok(())
    }
}

fn default_config_version() -> u32 {
    CURRENT_CONFIG_VERSION
}

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

fn default_max_log_files() -> usize {
    7
}

fn default_stdout_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.logging.max_log_files, 7);
        assert!(config.logging.stdout);
        assert_eq!(config.logging.level, LogLevel::Info);
    }

    #[test]
    fn invalid_version_rejected() {
        let mut config = Config::default();
        config.config_version = CURRENT_CONFIG_VERSION + 1;
        let result = config.validate();
        assert!(matches!(
            result,
            Err(ValidationError::UnsupportedVersion { .. })
        ));
    }
}
