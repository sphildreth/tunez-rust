use crate::{APP_AUTHOR, APP_NAME, APP_QUALIFIER};
use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AppDirs {
    config_dir: PathBuf,
    data_dir: PathBuf,
    cache_dir: PathBuf,
    log_dir: PathBuf,
}

impl AppDirs {
    pub fn discover() -> Result<Self, DirsError> {
        let dirs = ProjectDirs::from(APP_QUALIFIER, APP_AUTHOR, APP_NAME)
            .ok_or(DirsError::MissingProjectDirs)?;
        let log_dir = dirs.data_dir().join("logs");
        Ok(Self {
            config_dir: dirs.config_dir().to_path_buf(),
            data_dir: dirs.data_dir().to_path_buf(),
            cache_dir: dirs.cache_dir().to_path_buf(),
            log_dir,
        })
    }

    pub fn ensure_exists(&self) -> Result<(), DirsError> {
        for dir in [
            &self.config_dir,
            &self.data_dir,
            &self.cache_dir,
            &self.log_dir,
        ] {
            std::fs::create_dir_all(dir).map_err(|source| DirsError::CreateDirectory {
                path: dir.clone(),
                source,
            })?;
        }
        Ok(())
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }
}

#[derive(Debug, Error)]
pub enum DirsError {
    #[error("unable to determine project directories for Tunez")]
    MissingProjectDirs,
    #[error("failed to create directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_returns_dirs() {
        let dirs = AppDirs::discover().expect("should build dirs");
        assert!(dirs.config_dir().ends_with(APP_NAME));
        assert!(dirs.log_dir().ends_with("logs"));
    }
}
