use tunez_core::models::{Track, TrackId};
use tunez_core::{PlaybackProgress, PlaybackState, ScrobbleEvent};
use tunez_core::scrobbler::{run_scrobbler_contract, ScrobblerContractSpec};
use melodee_scrobbler::MelodeeScrobbler;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_track() -> Track {
    Track {
        id: TrackId::new("track-1"),
        provider_id: "filesystem".into(),
        title: "Example".into(),
        artist: "Artist".into(),
        album: Some("Album".into()),
        duration_seconds: Some(180),
        track_number: Some(1),
    }
}

fn sample_event(state: PlaybackState, position: u64) -> ScrobbleEvent {
    ScrobbleEvent {
        track: sample_track(),
        progress: PlaybackProgress {
            position_seconds: position,
            duration_seconds: Some(180),
        },
        state,
        player_name: "Tunez".into(),
        device_id: Some("device-1".into()),
    }
}

#[tokio::test]
async fn melodee_scrobbler_contract() {
    let mock_server = MockServer::start().await;
    
    // Expect POST /api/v1/scrobble
    // We expect it to be called for each event in the contract
    Mock::given(method("POST"))
        .and(path("/api/v1/scrobble"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let scrobbler = MelodeeScrobbler::new(
        &mock_server.uri(),
        "test-token", // Mandatory token
    );
    
    // Events to submit
    let events = vec![
        sample_event(PlaybackState::Started, 0),
        sample_event(PlaybackState::Resumed, 10),
        sample_event(PlaybackState::Ended, 180),
    ];
    
    let spec = ScrobblerContractSpec {
        scrobbler: &scrobbler,
        events,
        load_persisted: None, // Network scrobbler doesn't persist itself
    };
    
    if let Err(e) = run_scrobbler_contract(spec).await {
        panic!("Contract test failed: {}", e);
    }
}
