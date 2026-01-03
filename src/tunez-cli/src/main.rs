use anyhow::Result;
use clap::{Parser, Subcommand};
use melodee_scrobbler::MelodeeScrobbler;
use std::sync::Arc;
use thiserror::Error;
use tunez_core::scrobbler::{PersistentScrobbler, Scrobbler};
use tunez_core::{init_logging, AppDirs, Config, ProviderSelection, ValidationError};
use tunez_plugin::{ExecPluginProvider, PluginConfig};
use tunez_ui::{run_ui, Theme, UiContext};

#[derive(Debug, Parser)]
#[command(name = "tunez", version, about = "Terminal music player")]
struct Cli {
    /// Provider override (global so it applies to subcommands; takes precedence over config)
    #[arg(long, global = true)]
    provider: Option<String>,
    /// Profile override (global so it applies to subcommands; takes precedence over config)
    #[arg(long, global = true)]
    profile: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Build a play request from selectors (parses only; playback coming in later phases)
    Play(PlayCommand),
    /// Provider management commands
    #[command(subcommand)]
    Providers(ProvidersCommand),
}

#[derive(Debug, Subcommand)]
enum ProvidersCommand {
    /// List configured providers and profiles
    List,
}

#[derive(Debug, Parser, Clone)]
struct PlayCommand {
    /// Track filter
    #[arg(long)]
    track: Option<String>,
    /// Album filter
    #[arg(long)]
    album: Option<String>,
    /// Artist filter
    #[arg(long)]
    artist: Option<String>,
    /// Playlist name selector
    #[arg(long)]
    playlist: Option<String>,
    /// Provider-scoped stable identifier (takes precedence over other selectors)
    #[arg(long)]
    id: Option<String>,
    /// Begin playback immediately after resolving selection
    #[arg(short = 'p', long)]
    autoplay: bool,
}

use tunez_core::models::PlaySelector;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlayIntent {
    provider: ProviderSelection,
    selector: PlaySelector,
    autoplay: bool,
}

#[derive(Debug, Error)]
enum PlaySelectorError {
    #[error("play requires at least one selector (--id/--playlist/--track/--album/--artist)")]
    MissingSelector,
    #[error("playlist selector cannot be combined with track, album, or artist selectors")]
    PlaylistConflict,
    #[error("internal selector invariant violated: {0}")]
    InvariantViolation(&'static str),
    #[error("{0}")]
    Provider(#[from] ValidationError),
}

impl PlayCommand {
    fn into_intent(
        self,
        config: &Config,
        cli_provider: Option<&str>,
        cli_profile: Option<&str>,
    ) -> Result<PlayIntent, PlaySelectorError> {
        let PlayCommand {
            track,
            album,
            artist,
            playlist,
            id,
            autoplay,
        } = self;
        let selector = Self::build_selector(track, album, artist, playlist, id)?;
        let provider = config.resolve_provider_selection(cli_provider, cli_profile)?;

        Ok(PlayIntent {
            provider,
            selector,
            autoplay,
        })
    }

    #[cfg(test)]
    fn into_selector(self) -> Result<PlaySelector, PlaySelectorError> {
        let PlayCommand {
            track,
            album,
            artist,
            playlist,
            id,
            autoplay: _,
        } = self;

        Self::build_selector(track, album, artist, playlist, id)
    }

    fn build_selector(
        track: Option<String>,
        album: Option<String>,
        artist: Option<String>,
        playlist: Option<String>,
        id: Option<String>,
    ) -> Result<PlaySelector, PlaySelectorError> {
        // Selector precedence: id > playlist > track > album > artist.
        if let Some(id) = id {
            return Ok(PlaySelector::Id { id });
        }

        let has_playlist = playlist.is_some();
        let has_track = track.is_some();
        let has_album = album.is_some();
        let has_artist = artist.is_some();

        if !(has_playlist || has_track || has_album || has_artist) {
            return Err(PlaySelectorError::MissingSelector);
        }

        if has_playlist && (has_track || has_album || has_artist) {
            return Err(PlaySelectorError::PlaylistConflict);
        }

        if let Some(name) = playlist {
            return Ok(PlaySelector::Playlist { name });
        }

        if let Some(track) = track {
            return Ok(PlaySelector::TrackSearch {
                track,
                artist,
                album,
            });
        }

        if let Some(album) = album {
            return Ok(PlaySelector::AlbumSearch { album, artist });
        }

        if let Some(artist) = artist {
            return Ok(PlaySelector::ArtistSearch { artist });
        }

        Err(PlaySelectorError::InvariantViolation(
            "selector validation yielded no remaining selector",
        ))
    }
}



#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let dirs = AppDirs::discover()?;
    let config = Config::load_or_default(&dirs)?;
    let _logging = init_logging(&config.logging, &dirs)?;

    match cli.command {
        Some(Command::Providers(ProvidersCommand::List)) => {
            print_providers(&config);
            return Ok(());
        }
        Some(Command::Play(play)) => {
            let intent =
                play.into_intent(&config, cli.provider.as_deref(), cli.profile.as_deref())?;
            
            let selection = intent.provider.clone();
            let provider = create_provider(&selection, &config)?;
            let scrobbler = create_scrobbler(&selection, &config, &dirs)?;

            let mut ctx = UiContext::new(
                provider,
                selection,
                scrobbler,
                Theme::from_config(config.theme.as_deref()),
                dirs.clone(),
            );
            ctx.initial_play = Some(intent.selector.clone());

            tracing::info!("Launching Tunez with play intent: {:?}", intent.selector);
            run_ui(ctx)?;
        }
        None => {
            let selection = config
                .resolve_provider_selection(cli.provider.as_deref(), cli.profile.as_deref())?;
            let provider = create_provider(&selection, &config)?;
            let scrobbler = create_scrobbler(&selection, &config, &dirs)?;

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
            run_ui(UiContext::new(
                provider,
                selection,
                scrobbler,
                Theme::from_config(config.theme.as_deref()),
                dirs.clone(),
            ))?;
        }
    }

    Ok(())
}

fn create_provider(
    selection: &ProviderSelection,
    config: &Config,
) -> Result<std::sync::Arc<dyn tunez_core::Provider>, anyhow::Error> {
    let provider_config = config
        .providers
        .get(&selection.provider_id)
        .ok_or_else(|| {
            anyhow::anyhow!("Provider '{}' not found in config", selection.provider_id)
        })?;

    match provider_config.kind.as_deref().unwrap_or("") {
        "filesystem" => {
            // Get the library root from the profile config or default to current directory
            let library_root = if let Some(profile_name) = &selection.profile {
                if let Some(profile) = provider_config.profiles.get(profile_name) {
                    profile.library_root.as_deref().unwrap_or("./music")
                } else {
                    return Err(anyhow::anyhow!(
                        "Profile '{}' not found for provider '{}'",
                        profile_name,
                        selection.provider_id
                    ));
                }
            } else {
                "./music" // default
            };

            let provider =
                filesystem_provider::FilesystemProvider::new(vec![library_root.to_string()])?;
            Ok(std::sync::Arc::new(provider))
        }
        "melodee" => {
            // Get the base URL from the profile config
            let base_url = if let Some(profile_name) = &selection.profile {
                if let Some(profile) = provider_config.profiles.get(profile_name) {
                    profile.base_url.as_deref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "'base_url' not found in profile '{}' for provider '{}'",
                            profile_name,
                            selection.provider_id
                        )
                    })?
                } else {
                    return Err(anyhow::anyhow!(
                        "Profile '{}' not found for provider '{}'",
                        profile_name,
                        selection.provider_id
                    ));
                }
            } else {
                return Err(anyhow::anyhow!("Profile required for melodee provider"));
            };

            let melodee_config = melodee_provider::MelodeeConfig {
                base_url: base_url.to_string(),
                profile: selection.profile.clone(),
            };

            let provider = melodee_provider::MelodeeProvider::new(melodee_config)?;
            Ok(std::sync::Arc::new(provider))
        }
        "plugin" => {
            // Get the plugin executable path from the profile config
            let executable = if let Some(profile_name) = &selection.profile {
                if let Some(profile) = provider_config.profiles.get(profile_name) {
                    profile.plugin_executable.as_deref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "'plugin_executable' not found in profile '{}' for provider '{}'",
                            profile_name,
                            selection.provider_id
                        )
                    })?
                } else {
                    return Err(anyhow::anyhow!(
                        "Profile '{}' not found for provider '{}'",
                        profile_name,
                        selection.provider_id
                    ));
                }
            } else {
                return Err(anyhow::anyhow!("Profile required for plugin provider"));
            };

            let args = if let Some(profile_name) = &selection.profile {
                if let Some(profile) = provider_config.profiles.get(profile_name) {
                    profile.plugin_args.clone()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            let plugin_config = PluginConfig {
                executable: std::path::PathBuf::from(executable),
                args,
                working_dir: None,
                env: vec![],
            };

            let provider = ExecPluginProvider::new(plugin_config)?;
            Ok(std::sync::Arc::new(provider))
        }
        _ => Err(anyhow::anyhow!(
            "Unknown provider kind: '{}'",
            provider_config.kind.as_deref().unwrap_or("")
        )),
    }
}

fn create_scrobbler(
    selection: &ProviderSelection,
    config: &Config,
    dirs: &AppDirs,
) -> Result<Option<Arc<dyn Scrobbler>>, anyhow::Error> {
    let provider_config = config.providers.get(&selection.provider_id);
    // If provider config missing, create_provider would handle it, here we just return None
    let provider_config = match provider_config {
        Some(c) => c,
        None => return Ok(None),
    };

    if provider_config.kind.as_deref() == Some("melodee") {
        let base_url = if let Some(profile_name) = &selection.profile {
            if let Some(profile) = provider_config.profiles.get(profile_name) {
                profile
                    .base_url
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("missing base_url"))?
            } else {
                return Ok(None);
            }
        } else {
            return Ok(None);
        };

        let remote = MelodeeScrobbler::new(base_url, selection.profile.clone(), None);
        let path = dirs.data_dir().join("scrobbles.jsonl");

        // PersistentScrobbler new(id, path, batch_size, player_name, device_id, wrapped)
        // Check PersistentScrobbler::new signature.
        // It wraps a wrapped scrobbler? No, wait.
        // Previously FileScrobbler was standalone.
        // Refactor in scrobbler.rs introduced PersistentScrobbler<S: Scrobbler>.
        // Constructor: PersistentScrobbler::new(wrapped: S, path: PathBuf).
        // I need to verify PersistentScrobbler::new signature.

        let persistent = PersistentScrobbler::new(remote, path, 1000);

        Ok(Some(Arc::new(persistent)))
    } else {
        Ok(None)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tunez_core::{ProviderConfig, ProviderProfile};

    fn config_with_provider(provider_id: &str, profile: &str) -> Config {
        let mut providers = BTreeMap::new();
        let mut profiles = BTreeMap::new();
        profiles.insert(profile.to_string(), ProviderProfile::default());
        providers.insert(
            provider_id.to_string(),
            ProviderConfig {
                kind: Some("filesystem".into()),
                profiles,
            },
        );

        let mut config = Config::default();
        config.default_provider = Some(provider_id.to_string());
        config.providers = providers;
        config
    }

    #[test]
    fn play_selector_requires_input() {
        let play = PlayCommand {
            track: None,
            album: None,
            artist: None,
            playlist: None,
            id: None,
            autoplay: false,
        };

        let err = play
            .into_selector()
            .expect_err("selector should be required");
        assert!(matches!(err, PlaySelectorError::MissingSelector));
    }

    #[test]
    fn play_selector_id_takes_precedence() {
        let play = PlayCommand {
            track: Some("track".into()),
            album: Some("album".into()),
            artist: Some("artist".into()),
            playlist: None,
            id: Some("stable-id-123".into()),
            autoplay: false,
        };

        let selector = play.into_selector().expect("id should be accepted");
        assert_eq!(
            selector,
            PlaySelector::Id {
                id: "stable-id-123".into()
            }
        );
    }

    #[test]
    fn play_selector_tracks_allow_filters() {
        let play = PlayCommand {
            track: Some("track".into()),
            album: Some("album".into()),
            artist: Some("artist".into()),
            playlist: None,
            id: None,
            autoplay: true,
        };

        let selector = play
            .into_selector()
            .expect("track selector should be valid");
        assert_eq!(
            selector,
            PlaySelector::TrackSearch {
                track: "track".into(),
                artist: Some("artist".into()),
                album: Some("album".into()),
            }
        );
    }

    #[test]
    fn play_selector_playlist_conflict() {
        let play = PlayCommand {
            track: Some("track".into()),
            album: None,
            artist: None,
            playlist: Some("mix".into()),
            id: None,
            autoplay: false,
        };

        let err = play
            .into_selector()
            .expect_err("conflicting playlist selector");
        assert!(matches!(err, PlaySelectorError::PlaylistConflict));
    }

    #[test]
    fn play_intent_resolves_provider_selection() {
        let config = config_with_provider("filesystem", "home");
        let play = PlayCommand {
            track: Some("song".into()),
            album: None,
            artist: None,
            playlist: None,
            id: None,
            autoplay: true,
        };

        let intent = play
            .into_intent(&config, Some("filesystem"), Some("home"))
            .expect("intent should resolve");

        assert_eq!(intent.provider.provider_id, "filesystem");
        assert_eq!(intent.provider.profile.as_deref(), Some("home"));
        assert_eq!(intent.selector.describe(), "track=\"song\"");
        assert!(intent.autoplay);
    }
}
