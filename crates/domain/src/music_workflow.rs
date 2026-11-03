//! Music-specific persisted graph and presentation contracts.
use crate::{
    MetadataValue, MusicArtist, MusicIdentifier, MusicMedium, MusicRecording, MusicRelease,
    MusicReleaseGroup, MusicTrack,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicReleaseGraph {
    pub artists: Vec<MusicArtist>,
    pub release_group: MusicReleaseGroup,
    pub release: MusicRelease,
    pub media: Vec<MusicMedium>,
    pub tracks: Vec<MusicTrack>,
    pub recordings: Vec<MusicRecording>,
    pub identifiers: Vec<MusicIdentifier>,
    pub source_url: String,
    pub fetched_at: DateTime<Utc>,
    pub locale: Option<String>,
    #[serde(default)]
    pub release_date_text: Option<String>,
    #[serde(default)]
    pub release_disambiguation: Option<String>,
    #[serde(default)]
    pub group_first_release_date_text: Option<String>,
    #[serde(default)]
    pub files: Vec<MusicTrackFile>,
    #[serde(default)]
    pub field_sources: Vec<MetadataValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicTrackFile {
    pub track_id: Uuid,
    pub file_id: Uuid,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicAlbumView {
    pub release_id: Uuid,
    pub title: String,
    pub artist: String,
    pub country: Option<String>,
    pub date: Option<String>,
    pub medium_count: u32,
    pub track_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicArtistView {
    pub artist: MusicArtist,
    pub releases: Vec<MusicAlbumView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicLocalFile {
    pub file_id: Uuid,
    pub media_item_id: Uuid,
    pub relative_path: String,
}
