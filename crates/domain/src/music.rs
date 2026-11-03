//! MusicBrainz-style entities preserve release, medium, track and recording identity.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicArtist {
    pub media_item_id: Uuid,
    pub name: String,
    pub sort_name: Option<String>,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistCredit {
    pub artist_id: Uuid,
    pub credited_name: String,
    pub join_phrase: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicReleaseGroup {
    pub media_item_id: Uuid,
    pub title: String,
    pub artist_credits: Vec<ArtistCredit>,
    pub primary_type: Option<String>,
    pub secondary_types: Vec<String>,
    pub first_release_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicRelease {
    pub media_item_id: Uuid,
    pub release_group_id: Uuid,
    pub title: String,
    pub artist_credits: Vec<ArtistCredit>,
    pub country: Option<String>,
    pub release_date: Option<NaiveDate>,
    pub status: Option<String>,
    pub labels: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicMedium {
    pub id: Uuid,
    pub release_id: Uuid,
    pub position: u32,
    pub title: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicTrack {
    pub media_item_id: Uuid,
    pub medium_id: Uuid,
    pub recording_id: Uuid,
    pub position: u32,
    pub number: String,
    pub title: String,
    pub length_ms: Option<u64>,
    pub artist_credits: Vec<ArtistCredit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicRecording {
    pub media_item_id: Uuid,
    pub title: String,
    pub length_ms: Option<u64>,
    pub artist_credits: Vec<ArtistCredit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicIdentifierKind {
    MusicBrainz,
    Isrc,
    Upc,
    Ean,
    CatalogNumber,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicIdentifier {
    pub media_item_id: Uuid,
    pub kind: MusicIdentifierKind,
    pub namespace: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_keep_region_and_medium_identity_separate_from_recordings() {
        let release_group_id = Uuid::new_v4();
        let first = MusicRelease {
            media_item_id: Uuid::new_v4(),
            release_group_id,
            title: "Album".into(),
            artist_credits: vec![],
            country: Some("JP".into()),
            release_date: None,
            status: Some("official".into()),
            labels: vec![],
        };
        let mut second = first.clone();
        second.media_item_id = Uuid::new_v4();
        second.country = Some("US".into());
        let releases = vec![first, second];
        let decoded: Vec<MusicRelease> =
            serde_json::from_value(serde_json::to_value(&releases).unwrap()).unwrap();
        assert_eq!(decoded, releases);
        assert_ne!(decoded[0].media_item_id, decoded[1].media_item_id);
        assert_eq!(decoded[0].release_group_id, decoded[1].release_group_id);
        let recording_id = Uuid::new_v4();
        let track = MusicTrack {
            media_item_id: Uuid::new_v4(),
            medium_id: Uuid::new_v4(),
            recording_id,
            position: 1,
            number: "A1".into(),
            title: "Track".into(),
            length_ms: Some(180_000),
            artist_credits: vec![],
        };
        let decoded: MusicTrack =
            serde_json::from_value(serde_json::to_value(&track).unwrap()).unwrap();
        assert_ne!(decoded.media_item_id, decoded.recording_id);
        assert_eq!(decoded.recording_id, recording_id);
        assert_eq!(decoded.number, "A1");
    }

    #[test]
    fn music_graph_retains_disc_and_artist_credits_without_flattening_ids() {
        let artist = MusicArtist {
            media_item_id: Uuid::new_v4(),
            name: "Fixture artist".into(),
            sort_name: None,
            aliases: vec!["別名".into()],
        };
        let credit = ArtistCredit {
            artist_id: artist.media_item_id,
            credited_name: "Fixture artist".into(),
            join_phrase: " feat. ".into(),
        };
        let group = MusicReleaseGroup {
            media_item_id: Uuid::new_v4(),
            title: "Fixture album".into(),
            artist_credits: vec![credit.clone()],
            primary_type: Some("album".into()),
            secondary_types: vec![],
            first_release_date: None,
        };
        let release = MusicRelease {
            media_item_id: Uuid::new_v4(),
            release_group_id: group.media_item_id,
            title: "Fixture album".into(),
            artist_credits: vec![credit.clone()],
            country: None,
            release_date: None,
            status: None,
            labels: vec![],
        };
        let first = MusicMedium {
            id: Uuid::new_v4(),
            release_id: release.media_item_id,
            position: 1,
            title: None,
            format: Some("CD".into()),
        };
        let mut second = first.clone();
        second.id = Uuid::new_v4();
        second.position = 2;
        let recording = MusicRecording {
            media_item_id: Uuid::new_v4(),
            title: "Fixture recording".into(),
            length_ms: Some(1000),
            artist_credits: vec![credit],
        };
        let graph = (artist, group, release, vec![first, second], recording);
        let decoded: (
            MusicArtist,
            MusicReleaseGroup,
            MusicRelease,
            Vec<MusicMedium>,
            MusicRecording,
        ) = serde_json::from_value(serde_json::to_value(&graph).unwrap()).unwrap();
        assert_eq!(decoded, graph);
        assert_eq!(decoded.3[0].release_id, decoded.2.media_item_id);
        assert_eq!(decoded.3[1].release_id, decoded.2.media_item_id);
        assert_ne!(decoded.3[0].id, decoded.3[1].id);
        assert_eq!(
            decoded.4.artist_credits[0].artist_id,
            decoded.0.media_item_id
        );
    }
}
