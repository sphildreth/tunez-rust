pub mod config;
pub mod logging;
pub mod models;
pub mod paths;
pub mod provider;
pub mod scrobbler;

pub use config::{
    Config, ConfigError, LogLevel, LoggingConfig, ProviderConfig, ProviderProfile,
    ProviderSelection, ValidationError,
};
pub use logging::{init_logging, LoggingError, LoggingGuard};
pub use models::*;
pub use paths::{AppDirs, DirsError};
pub use provider::*;
pub use scrobbler::*;

pub const APP_NAME: &str = "tunez";
pub const APP_AUTHOR: &str = "Tunez";
pub const APP_QUALIFIER: &str = "io";
