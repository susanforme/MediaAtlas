use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::PathStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Movie,
    TvSeries,
    TvSeason,
    TvEpisode,
    Anime,
    AdultVideo,
    MusicArtist,
    MusicReleaseGroup,
    MusicRelease,
    MusicTrack,
    MusicRecording,
    MusicVideo,
    Concert,
}

impl MediaKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::TvSeries => "tv_series",
            Self::TvSeason => "tv_season",
            Self::TvEpisode => "tv_episode",
            Self::Anime => "anime",
            Self::AdultVideo => "adult_video",
            Self::MusicArtist => "music_artist",
            Self::MusicReleaseGroup => "music_release_group",
            Self::MusicRelease => "music_release",
            Self::MusicTrack => "music_track",
            Self::MusicRecording => "music_recording",
            Self::MusicVideo => "music_video",
            Self::Concert => "concert",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryRoot {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub path_style: PathStyle,
    /// None denotes an explicitly selected mixed library.
    pub kind: Option<MediaKind>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: Uuid,
    pub kind: MediaKind,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaFile {
    pub id: Uuid,
    pub media_item_id: Uuid,
    pub root_id: Uuid,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_at: Option<DateTime<Utc>>,
    pub part: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkKind {
    Poster,
    Backdrop,
    Logo,
    Thumb,
    Scene,
    Artist,
    AlbumCover,
    Cover,
    BackCover,
    Sample,
}

impl ArtworkKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Poster => "poster",
            Self::Backdrop => "backdrop",
            Self::Logo => "logo",
            Self::Thumb => "thumb",
            Self::Scene => "scene",
            Self::Artist => "artist",
            Self::AlbumCover => "album_cover",
            Self::Cover => "cover",
            Self::BackCover => "back_cover",
            Self::Sample => "sample",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artwork {
    pub id: Uuid,
    pub media_item_id: Uuid,
    pub kind: ArtworkKind,
    pub cache_path: Option<String>,
    pub source_url: Option<String>,
    pub provider_id: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub content_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalId {
    pub id: Uuid,
    pub media_item_id: Uuid,
    pub provider_id: String,
    pub namespace: String,
    pub external_id: String,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub id: Uuid,
    pub name: String,
    pub aliases: Vec<String>,
}

/// Alias identity belongs to one person; matching text never merges two people.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonAlias {
    pub id: Uuid,
    pub person_id: Uuid,
    pub alias: String,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum CreditTarget {
    Person(Uuid),
    Organization(Uuid),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credit {
    pub id: Uuid,
    pub media_item_id: Uuid,
    pub target: CreditTarget,
    pub role: String,
    pub character_name: Option<String>,
    pub sort_order: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_aliases_retain_distinct_person_identities() {
        let first = PersonAlias {
            id: Uuid::new_v4(),
            person_id: Uuid::new_v4(),
            alias: "Fixture name".into(),
            locale: Some("en".into()),
        };
        let second = PersonAlias {
            id: Uuid::new_v4(),
            person_id: Uuid::new_v4(),
            ..first.clone()
        };
        let aliases = vec![first, second];
        let decoded: Vec<PersonAlias> =
            serde_json::from_value(serde_json::to_value(&aliases).unwrap()).unwrap();
        assert_eq!(decoded, aliases);
        assert_ne!(decoded[0].person_id, decoded[1].person_id);
    }

    #[test]
    fn kind_ids_are_stable_snake_case_and_artist_credits_distinguish_organizations() {
        for kind in [
            MediaKind::Movie,
            MediaKind::TvSeries,
            MediaKind::TvSeason,
            MediaKind::TvEpisode,
            MediaKind::Anime,
            MediaKind::AdultVideo,
            MediaKind::MusicArtist,
            MediaKind::MusicReleaseGroup,
            MediaKind::MusicRelease,
            MediaKind::MusicTrack,
            MediaKind::MusicRecording,
            MediaKind::MusicVideo,
            MediaKind::Concert,
        ] {
            assert_eq!(serde_json::to_value(kind).unwrap(), kind.as_str());
            assert_eq!(
                serde_json::from_value::<MediaKind>(serde_json::json!(kind.as_str())).unwrap(),
                kind
            );
        }
        let id = Uuid::new_v4();
        assert_ne!(
            serde_json::to_value(CreditTarget::Person(id)).unwrap(),
            serde_json::to_value(CreditTarget::Organization(id)).unwrap()
        );
    }
}
