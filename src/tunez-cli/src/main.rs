use anyhow::Result;
use clap::{Parser, Subcommand};
use thiserror::Error;
use tunez_core::{init_logging, AppDirs, Config, ProviderSelection, ValidationError};

#[derive(Debug, Parser)]
#[command(name = "tunez", version, about = "Terminal music player")]
struct Cli {
    /// Provider override (takes precedence over config)
    #[arg(long, global = true)]
    provider: Option<String>,
    /// Profile override (takes precedence over config)
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlayIntent {
    provider: ProviderSelection,
    selector: PlaySelector,
    autoplay: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PlaySelector {
    Id {
        id: String,
    },
    Playlist {
        name: String,
    },
    TrackSearch {
        track: String,
        artist: Option<String>,
        album: Option<String>,
    },
    AlbumSearch {
        album: String,
        artist: Option<String>,
    },
    ArtistSearch {
        artist: String,
    },
}

#[derive(Debug, Error)]
enum PlaySelectorError {
    #[error("play requires at least one selector (--id/--playlist/--track/--album/--artist)")]
    MissingSelector,
    #[error("playlist selector cannot be combined with other selectors")]
    PlaylistConflict,
    #[error("{0}")]
    Provider(#[from] ValidationError),
}

impl PlayCommand {
    fn intent(
        &self,
        config: &Config,
        cli_provider: Option<&str>,
        cli_profile: Option<&str>,
    ) -> Result<PlayIntent, PlaySelectorError> {
        let provider = config.resolve_provider_selection(cli_provider, cli_profile)?;
        let selector = self.selector()?;

        Ok(PlayIntent {
            provider,
            selector,
            autoplay: self.autoplay,
        })
    }

    fn selector(&self) -> Result<PlaySelector, PlaySelectorError> {
        if let Some(id) = &self.id {
            return Ok(PlaySelector::Id { id: id.clone() });
        }

        let has_playlist = self.playlist.is_some();
        let has_track = self.track.is_some();
        let has_album = self.album.is_some();
        let has_artist = self.artist.is_some();

        if !(has_playlist || has_track || has_album || has_artist) {
            return Err(PlaySelectorError::MissingSelector);
        }

        if has_playlist && (has_track || has_album || has_artist) {
            return Err(PlaySelectorError::PlaylistConflict);
        }

        if let Some(name) = &self.playlist {
            return Ok(PlaySelector::Playlist { name: name.clone() });
        }

        if let Some(track) = &self.track {
            return Ok(PlaySelector::TrackSearch {
                track: track.clone(),
                artist: self.artist.clone(),
                album: self.album.clone(),
            });
        }

        if let Some(album) = &self.album {
            return Ok(PlaySelector::AlbumSearch {
                album: album.clone(),
                artist: self.artist.clone(),
            });
        }

        if let Some(artist) = &self.artist {
            return Ok(PlaySelector::ArtistSearch {
                artist: artist.clone(),
            });
        }

        Err(PlaySelectorError::MissingSelector)
    }
}

impl PlaySelector {
    fn describe(&self) -> String {
        match self {
            PlaySelector::Id { id } => format!("id={id}"),
            PlaySelector::Playlist { name } => format!("playlist=\"{name}\""),
            PlaySelector::TrackSearch {
                track,
                artist,
                album,
            } => {
                let mut parts = vec![format!("track=\"{track}\"")];
                if let Some(artist) = artist {
                    parts.push(format!("artist=\"{artist}\""));
                }
                if let Some(album) = album {
                    parts.push(format!("album=\"{album}\""));
                }
                parts.join(", ")
            }
            PlaySelector::AlbumSearch { album, artist } => {
                let mut parts = vec![format!("album=\"{album}\"")];
                if let Some(artist) = artist {
                    parts.push(format!("artist=\"{artist}\""));
                }
                parts.join(", ")
            }
            PlaySelector::ArtistSearch { artist } => format!("artist=\"{artist}\""),
        }
    }
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
        Some(Command::Play(play)) => {
            let intent = play.intent(&config, cli.provider.as_deref(), cli.profile.as_deref())?;
            let profile_suffix = intent
                .provider
                .profile
                .as_deref()
                .map(|p| format!(" (profile '{p}')"))
                .unwrap_or_default();
            tracing::info!(
                "Play request: provider '{}'{} | selector: {} | autoplay: {}",
                intent.provider.provider_id,
                profile_suffix,
                intent.selector.describe(),
                intent.autoplay
            );
            println!(
                "Play request resolved for provider '{}'{}: {} (autoplay: {})",
                intent.provider.provider_id,
                profile_suffix,
                intent.selector.describe(),
                intent.autoplay
            );
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

        let err = play.selector().expect_err("selector should be required");
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

        let selector = play.selector().expect("id should be accepted");
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

        let selector = play.selector().expect("track selector should be valid");
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

        let err = play.selector().expect_err("conflicting playlist selector");
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
            .intent(&config, Some("filesystem"), Some("home"))
            .expect("intent should resolve");

        assert_eq!(intent.provider.provider_id, "filesystem");
        assert_eq!(intent.provider.profile.as_deref(), Some("home"));
        assert_eq!(intent.selector.describe(), "track=\"song\"".to_string());
        assert!(intent.autoplay);
    }
}
