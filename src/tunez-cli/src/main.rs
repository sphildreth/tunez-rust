use anyhow::Result;
use clap::{Parser, Subcommand};
use tunez_core::{init_logging, AppDirs, Config};

#[derive(Debug, Parser)]
#[command(name = "tunez", version, about = "Terminal music player")]
struct Cli {
    /// Provider override (takes precedence over config)
    #[arg(long)]
    provider: Option<String>,
    /// Profile override (takes precedence over config)
    #[arg(long)]
    profile: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Provider management commands
    #[command(subcommand)]
    Providers(ProvidersCommand),
}

#[derive(Debug, Subcommand)]
enum ProvidersCommand {
    /// List configured providers and profiles
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let dirs = AppDirs::discover()?;
    let config = Config::load_or_default(&dirs)?;
    let _logging = init_logging(&config.logging, &dirs)?;

    match cli.command {
        Some(Command::Providers(ProvidersCommand::List)) => {
            print_providers(&config);
            return Ok(());
        }
        None => {
            let selection = config
                .resolve_provider_selection(cli.provider.as_deref(), cli.profile.as_deref())?;
            tracing::info!(
                "Launching Tunez with provider '{}'{} (config dir: {})",
                selection.provider_id,
                selection
                    .profile
                    .as_deref()
                    .map(|p| format!(" profile '{}'", p))
                    .unwrap_or_default(),
                dirs.config_dir().display()
            );
            // TODO: launch TUI shell once implemented.
        }
    }

    Ok(())
}

fn print_providers(config: &Config) {
    if config.providers.is_empty() {
        println!("No providers configured. Set providers.<id> in config.toml.");
        return;
    }

    for (id, provider) in &config.providers {
        let default_marker = if config.default_provider.as_deref() == Some(id) {
            " (default)"
        } else {
            ""
        };

        println!("Provider: {}{}", id, default_marker);
        if provider.profiles.is_empty() {
            println!("  profiles: (none configured)");
        } else {
            for profile in provider.profiles.keys() {
                let is_default_profile = config.profile.as_deref() == Some(profile)
                    && config.default_provider.as_deref() == Some(id);
                let marker = if is_default_profile { " (default)" } else { "" };
                println!("  - {}{}", profile, marker);
            }
        }
    }
}
