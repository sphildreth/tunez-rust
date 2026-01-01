use anyhow::Result;
use tunez_core::{init_logging, AppDirs, Config};

fn main() -> Result<()> {
    let dirs = AppDirs::discover()?;
    let config = Config::load_or_default(&dirs)?;
    let _logging = init_logging(&config.logging, &dirs)?;

    tracing::info!(
        "Tunez initialized (skeleton) using config dir: {}",
        dirs.config_dir().display()
    );

    Ok(())
}
