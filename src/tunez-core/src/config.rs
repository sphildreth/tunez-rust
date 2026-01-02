use crate::paths::AppDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    pub theme: Option<String>,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderConfig>,
    #[serde(default)]
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Directory where downloaded tracks are stored
    #[serde(default)]
    pub download_dir: Option<String>,
    /// Maximum cache size in bytes (0 = no limit)
    #[serde(default = "default_max_cache_size")]
    pub max_size_bytes: u64,
    /// Maximum age of cached files in seconds (0 = no limit)
    #[serde(default = "default_max_cache_age")]
    pub max_age_seconds: u64,
    /// Whether to automatically clean up old files on startup
    #[serde(default = "default_auto_cleanup")]
    pub auto_cleanup: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            download_dir: None,
            max_size_bytes: default_max_cache_size(),
            max_age_seconds: default_max_cache_age(),
            auto_cleanup: default_auto_cleanup(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: default_config_version(),
            default_provider: None,
            profile: None,
            default_scrobbler: None,
            theme: None,
            logging: LoggingConfig::default(),
            providers: BTreeMap::new(),
            cache: CacheConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: LogLevel,
    #[serde(default = "default_max_log_files")]
    pub max_log_files: usize,
    /// Maximum size of each log file in bytes before rotation.
    /// Default is 10 MB. Set to 0 to disable size-based rotation.
    #[serde(default = "default_max_log_file_size")]
    pub max_log_file_size: u64,
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
            max_log_file_size: default_max_log_file_size(),
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
    #[error("no providers configured; set default_provider and providers.<id> blocks")]
    NoProvidersConfigured,
    #[error("default_provider '{provider_id}' not found in providers config")]
    MissingProvider { provider_id: String },
    #[error("profile '{profile}' not found under provider '{provider_id}'")]
    MissingProfile {
        provider_id: String,
        profile: String,
    },
    #[error("provider selection is required (set default_provider or pass --provider)")]
    MissingProviderSelection,
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

        if let Some(provider_id) = &self.default_provider {
            let provider = self.providers.get(provider_id).ok_or_else(|| {
                if self.providers.is_empty() {
                    ValidationError::NoProvidersConfigured
                } else {
                    ValidationError::MissingProvider {
                        provider_id: provider_id.clone(),
                    }
                }
            })?;

            if let Some(profile) = &self.profile {
                if !provider.profiles.contains_key(profile) {
                    return Err(ValidationError::MissingProfile {
                        provider_id: provider_id.clone(),
                        profile: profile.clone(),
                    });
                }
            }
        } else if self.profile.is_some() {
            return Err(ValidationError::MissingProviderSelection);
        }

        Ok(())
    }

    pub fn resolve_provider_selection(
        &self,
        cli_provider: Option<&str>,
        cli_profile: Option<&str>,
    ) -> Result<ProviderSelection, ValidationError> {
        let provider_id = cli_provider
            .or(self.default_provider.as_deref())
            .ok_or(ValidationError::MissingProviderSelection)?;

        let provider = self.providers.get(provider_id).ok_or_else(|| {
            if self.providers.is_empty() {
                ValidationError::NoProvidersConfigured
            } else {
                ValidationError::MissingProvider {
                    provider_id: provider_id.to_string(),
                }
            }
        })?;

        let profile = cli_profile
            .or(self.profile.as_deref())
            .map(|p| p.to_string());

        if let Some(profile_id) = &profile {
            if !provider.profiles.contains_key(profile_id) {
                return Err(ValidationError::MissingProfile {
                    provider_id: provider_id.to_string(),
                    profile: profile_id.clone(),
                });
            }
        }

        Ok(ProviderSelection {
            provider_id: provider_id.to_string(),
            profile,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, ProviderProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderProfile {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub library_root: Option<String>,
    /// Path to external plugin executable (for plugin-type providers).
    #[serde(default)]
    pub plugin_executable: Option<String>,
    /// Arguments to pass to the plugin executable.
    #[serde(default)]
    pub plugin_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelection {
    pub provider_id: String,
    pub profile: Option<String>,
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

/// Default max log file size: 10 MB
fn default_max_log_file_size() -> u64 {
    10 * 1024 * 1024
}

fn default_stdout_enabled() -> bool {
    true
}

fn default_max_cache_size() -> u64 {
    10 * 1024 * 1024 * 1024 // 10 GB
}

fn default_max_cache_age() -> u64 {
    30 * 24 * 60 * 60 // 30 days
}

fn default_auto_cleanup() -> bool {
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
        assert_eq!(config.logging.max_log_file_size, 10 * 1024 * 1024); // 10 MB
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

    #[test]
    fn missing_provider_when_default_set_is_invalid() {
        let mut config = Config::default();
        config.default_provider = Some("melodee".into());
        let result = config.validate();
        assert!(matches!(
            result,
            Err(ValidationError::NoProvidersConfigured)
        ));
    }

    #[test]
    fn missing_profile_is_invalid() {
        let mut providers = BTreeMap::new();
        providers.insert("filesystem".into(), ProviderConfig::default());

        let mut config = Config::default();
        config.default_provider = Some("filesystem".into());
        config.profile = Some("home".into());
        config.providers = providers;

        let result = config.validate();
        assert!(matches!(
            result,
            Err(ValidationError::MissingProfile { .. })
        ));
    }

    #[test]
    fn resolve_provider_prefers_cli_over_default() {
        let mut profiles = BTreeMap::new();
        profiles.insert("home".into(), ProviderProfile::default());

        let mut providers = BTreeMap::new();
        providers.insert(
            "filesystem".into(),
            ProviderConfig {
                kind: Some("filesystem".into()),
                profiles,
            },
        );

        let mut config = Config::default();
        config.default_provider = Some("filesystem".into());
        config.providers = providers;

        let selection = config
            .resolve_provider_selection(Some("filesystem"), Some("home"))
            .expect("selection should succeed");

        assert_eq!(selection.provider_id, "filesystem");
        assert_eq!(selection.profile.as_deref(), Some("home"));
    }
}
