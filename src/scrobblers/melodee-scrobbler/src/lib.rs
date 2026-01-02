use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde_json::json;
use std::time::Duration;
use tunez_core::scrobbler::{
    PlaybackState, ScrobbleEvent, Scrobbler, ScrobblerError, ScrobblerResult,
};

use std::sync::{Arc, RwLock};
use tunez_core::secrets::CredentialStore;

pub struct MelodeeScrobbler {
    client: Client,
    base_url: String,
    profile: Option<String>,
    creds: CredentialStore,
    token: Arc<RwLock<Option<String>>>,
}

impl MelodeeScrobbler {
    pub fn new(
        base_url: impl Into<String>,
        profile: Option<String>,
        initial_token: Option<String>,
    ) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            base_url: base_url.into(),
            profile,
            creds: CredentialStore::new(),
            token: Arc::new(RwLock::new(initial_token)),
        }
    }

    fn get_token(&self) -> Option<String> {
        if let Ok(guard) = self.token.read() {
            if let Some(token) = guard.as_ref() {
                return Some(token.clone());
            }
        }

        if let Ok(token) = self
            .creds
            .get_access_token("melodee", self.profile.as_deref())
        {
            if let Ok(mut guard) = self.token.write() {
                *guard = Some(token.clone());
            }
            return Some(token);
        }

        None
    }
}

#[async_trait]
impl Scrobbler for MelodeeScrobbler {
    fn id(&self) -> &str {
        "melodee"
    }

    async fn submit(&self, event: &ScrobbleEvent) -> ScrobblerResult<()> {
        // Melodee API expects:
        // POST /api/v1/scrobble
        // {
        //   "songId": "uuid",
        //   "playerName": "Tunez",
        //   "scrobbleType": "NowPlaying|Submission",
        //   "timestamp": double,
        //   "playedDuration": double
        // }

        // We only scrobble on Started (NowPlaying) or Ended (Submission)
        let scrobble_type = match event.state {
            PlaybackState::Started => "NowPlaying",
            PlaybackState::Ended => "Submission",
            _ => return Ok(()), // Ignore other states for now
        };

        // For this implementation, we assume the track ID is a UUID string valid for Melodee.
        // In a real multi-provider system, we'd need to check if the track source is actually Melodee
        // or support some form of lookup/matching.
        // Verify track ID format (simple heuristic)
        if event.track.provider_id != "melodee" {
            // Skip non-melodee tracks for the specific Melodee scrobbler?
            // Or should we try to fuzzy match?
            // Requirement says "Scrobble to Melodee".
            // If the track CAME from Melodee, it has a UUID.
            // If it came from Local, we can't scrobble by ID unless we search first.
            // Phase 1 MVP: assume we only scrobble if we have a valid ID or just try.
            // For now, let's assume if it looks like a UUID, we try.
            // But actually, `event.track.id` is the `TrackId` newtype.
            // Let's assume the ID string is the API key if provider is melodee.

            // NOTE: Robust implementation would do search-and-match here.
            tracing::debug!(
                "Skipping non-melodee track for Melodee scrobbler: {:?}",
                event.track.id
            );
            return Ok(());
        }

        let url = format!("{}/api/v1/scrobble", self.base_url.trim_end_matches('/'));

        let payload = json!({
            "songId": event.track.id.0, // Assuming TrackId wraps the UUID
            "playerName": event.player_name,
            "scrobbleType": scrobble_type,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            "playedDuration": event.progress.position_seconds as f64
        });

        let mut request = self.client.post(&url).json(&payload);

        if let Some(token) = self.get_token() {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let res = request.send().await.map_err(|e| ScrobblerError::Network {
            message: e.to_string(),
        })?;

        match res.status() {
            StatusCode::OK
            | StatusCode::CREATED
            | StatusCode::ACCEPTED
            | StatusCode::NO_CONTENT => Ok(()),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(ScrobblerError::Authentication {
                    message: "Invalid API token".into(),
                })
            }
            StatusCode::TOO_MANY_REQUESTS => Err(ScrobblerError::RateLimited {
                message: "Rate limited".into(),
            }),
            s => Err(ScrobblerError::Other {
                message: format!("API error: {}", s),
            }),
        }
    }
}
