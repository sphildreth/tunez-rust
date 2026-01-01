pub mod config;
pub mod logging;
pub mod paths;

pub use config::{Config, ConfigError, LogLevel, LoggingConfig, ValidationError};
pub use logging::{init_logging, LoggingError, LoggingGuard};
pub use paths::{AppDirs, DirsError};

pub const APP_NAME: &str = "tunez";
pub const APP_AUTHOR: &str = "Tunez";
pub const APP_QUALIFIER: &str = "io";
