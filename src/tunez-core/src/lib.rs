pub mod cache;
pub mod config;
pub mod logging;
pub mod models;
pub mod paths;
pub mod provider;
pub mod provider_contract;
pub mod redact;
pub mod scrobbler;
pub mod secrets;

pub use cache::{CacheError, CacheManager, CachePolicy, CacheResult, CacheStats};
pub use config::{
    CacheConfig, Config, ConfigError, LogLevel, LoggingConfig, ProviderConfig, ProviderProfile,
    ProviderSelection, ValidationError,
};
pub use logging::{init_logging, LoggingError, LoggingGuard};
pub use models::*;
pub use paths::{AppDirs, DirsError};
pub use provider::*;
pub use redact::{contains_sensitive, redact_secrets};
pub use scrobbler::*;
pub use secrets::{CredentialStore, SecretKind, SecretsError, SecretsResult};

pub const APP_NAME: &str = "tunez";
pub const APP_AUTHOR: &str = "Tunez";
pub const APP_QUALIFIER: &str = "io";
