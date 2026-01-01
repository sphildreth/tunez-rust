use lofty::{Accessor, AudioFile, ItemKey, Probe, TaggedFileExt};
use std::path::Path;
use tunez_core::provider::ProviderResult;

#[derive(Debug, Clone, Default)]
pub struct ParsedTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_seconds: Option<u32>,
    pub track_number: Option<u32>,
}

pub fn parse_tags(path: &Path) -> ProviderResult<ParsedTags> {
    let tagged = match Probe::open(path).and_then(|p| p.read()) {
        Ok(tagged) => tagged,
        Err(_) => return Ok(ParsedTags::default()),
    };

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let properties = tagged.properties();

    let title = tag.and_then(|t| t.get_string(&ItemKey::TrackTitle).map(|s| s.to_string()));
    let artist = tag.and_then(|t| t.artist().map(|s| s.to_string()));
    let album = tag.and_then(|t| t.album().map(|s| s.to_string()));
    let duration_seconds = Some(properties.duration().as_secs() as u32);
    let track_number = tag.and_then(|t| t.track()).map(|n| n as u32);

    Ok(ParsedTags {
        title,
        artist,
        album,
        duration_seconds,
        track_number,
    })
}
