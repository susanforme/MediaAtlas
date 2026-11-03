//! Music edition matching never authorizes an automatic import or tag write.
use mediaatlas_domain::{MusicIdentifierKind, MusicReleaseGraph};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MusicMatchInput {
    pub release_mbid: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub date: Option<String>,
    pub country: Option<String>,
    pub edition: Option<String>,
    pub disc_count: Option<u32>,
    /// Ordered per-disc track titles; disc boundaries are significant.
    #[serde(default)]
    pub tracklists: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicMatchAssessment {
    pub exact_release_id: bool,
    pub score: u32,
    /// Explicit edition selection remains required, including exact ID matches.
    pub needs_review: bool,
    pub reasons: Vec<String>,
    pub conflicts: Vec<String>,
}

fn normalized(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn assess_music_release(
    input: &MusicMatchInput,
    graph: &MusicReleaseGraph,
) -> MusicMatchAssessment {
    let mut result = MusicMatchAssessment {
        exact_release_id: false,
        score: 0,
        needs_review: true,
        reasons: vec![],
        conflicts: vec![],
    };
    if let Some(wanted) = &input.release_mbid {
        let matched = graph.identifiers.iter().any(|id| {
            id.media_item_id == graph.release.media_item_id
                && id.kind == MusicIdentifierKind::MusicBrainz
                && id.namespace == "release"
                && id.value.eq_ignore_ascii_case(wanted)
        });
        result.exact_release_id = matched;
        if matched {
            result.score += 100;
            result.reasons.push("release_mbid".into());
        } else {
            result.conflicts.push("release_mbid".into());
        }
    }
    let mut compare = |field: &str, expected: Option<&str>, actual: Option<&str>, weight: u32| {
        if let Some(expected) = expected.filter(|v| !v.trim().is_empty()) {
            if actual.is_some_and(|actual| normalized(actual) == normalized(expected)) {
                result.score += weight;
                result.reasons.push(field.into());
            } else {
                result.conflicts.push(field.into());
            }
        }
    };
    compare(
        "album",
        input.album.as_deref(),
        Some(&graph.release.title),
        20,
    );
    let artist = graph
        .release
        .artist_credits
        .iter()
        .map(|c| format!("{}{}", c.credited_name, c.join_phrase))
        .collect::<String>();
    compare("artist", input.artist.as_deref(), Some(&artist), 15);
    let date = graph
        .release_date_text
        .clone()
        .or_else(|| graph.release.release_date.map(|d| d.to_string()));
    compare("date", input.date.as_deref(), date.as_deref(), 10);
    compare(
        "country",
        input.country.as_deref(),
        graph.release.country.as_deref(),
        10,
    );
    compare(
        "edition",
        input.edition.as_deref(),
        graph.release_disambiguation.as_deref(),
        15,
    );
    let mut media = graph.media.iter().collect::<Vec<_>>();
    media.sort_by_key(|m| m.position);
    if let Some(count) = input.disc_count {
        if usize::try_from(count).ok() == Some(media.len()) {
            result.score += 10;
            result.reasons.push("disc_count".into());
        } else {
            result.conflicts.push("disc_count".into());
        }
    }
    if !input.tracklists.is_empty() {
        let actual = media
            .iter()
            .map(|m| {
                let mut tracks = graph
                    .tracks
                    .iter()
                    .filter(|t| t.medium_id == m.id)
                    .collect::<Vec<_>>();
                tracks.sort_by_key(|t| t.position);
                tracks
                    .iter()
                    .map(|t| normalized(&t.title))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let wanted = input
            .tracklists
            .iter()
            .map(|disc| disc.iter().map(|t| normalized(t)).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        if actual == wanted {
            result.score += 25;
            result.reasons.push("tracklist".into());
        } else {
            result.conflicts.push("tracklist".into());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use mediaatlas_domain::*;
    use uuid::Uuid;

    fn edition() -> MusicReleaseGraph {
        let group = Uuid::new_v4();
        let release = Uuid::new_v4();
        let artist = Uuid::new_v4();
        let credit = ArtistCredit {
            artist_id: artist,
            credited_name: "Artist".into(),
            join_phrase: String::new(),
        };
        let media = vec![
            MusicMedium {
                id: Uuid::new_v4(),
                release_id: release,
                position: 1,
                title: None,
                format: Some("CD".into()),
            },
            MusicMedium {
                id: Uuid::new_v4(),
                release_id: release,
                position: 2,
                title: None,
                format: Some("CD".into()),
            },
        ];
        let tracks = media
            .iter()
            .enumerate()
            .map(|(index, m)| MusicTrack {
                media_item_id: Uuid::new_v4(),
                medium_id: m.id,
                recording_id: Uuid::new_v4(),
                position: 1,
                number: "1".into(),
                title: format!("Song {}", index + 1),
                length_ms: None,
                artist_credits: vec![credit.clone()],
            })
            .collect();
        MusicReleaseGraph {
            artists: vec![MusicArtist {
                media_item_id: artist,
                name: "Artist".into(),
                sort_name: None,
                aliases: vec![],
            }],
            release_group: MusicReleaseGroup {
                media_item_id: group,
                title: "Album".into(),
                artist_credits: vec![credit.clone()],
                primary_type: Some("Album".into()),
                secondary_types: vec![],
                first_release_date: None,
            },
            release: MusicRelease {
                media_item_id: release,
                release_group_id: group,
                title: "Album".into(),
                artist_credits: vec![credit],
                country: Some("JP".into()),
                release_date: None,
                status: Some("Official".into()),
                labels: vec![],
            },
            media,
            tracks,
            recordings: vec![],
            identifiers: vec![MusicIdentifier {
                media_item_id: release,
                kind: MusicIdentifierKind::MusicBrainz,
                namespace: "release".into(),
                value: release.to_string(),
            }],
            source_url: format!("https://musicbrainz.org/release/{release}"),
            fetched_at: Utc::now(),
            locale: None,
            files: vec![],
            field_sources: vec![],
            release_date_text: Some("1999-02".into()),
            group_first_release_date_text: Some("1999".into()),
            release_disambiguation: Some("remastered deluxe".into()),
        }
    }

    #[test]
    fn title_or_group_identity_never_authorizes_automatic_release_merge() {
        let first = edition();
        let mut second = first.clone();
        second.release.media_item_id = Uuid::new_v4();
        second.identifiers[0].media_item_id = second.release.media_item_id;
        second.identifiers[0].value = second.release.media_item_id.to_string();
        second.release.country = Some("US".into());
        let input = MusicMatchInput {
            release_mbid: Some(first.release.media_item_id.to_string()),
            album: Some("Album".into()),
            country: Some("JP".into()),
            ..Default::default()
        };
        let result = assess_music_release(&input, &second);
        assert!(!result.exact_release_id);
        assert!(result.needs_review);
        assert!(result.conflicts.contains(&"release_mbid".into()));
        assert!(result.conflicts.contains(&"country".into()));
        assert_eq!(
            first.release_group.media_item_id,
            second.release_group.media_item_id
        );
    }

    #[test]
    fn ordered_discs_and_tracklists_distinguish_deluxe_editions() {
        let graph = edition();
        let input = MusicMatchInput {
            disc_count: Some(2),
            tracklists: vec![vec!["Song 1".into()], vec!["Song 2".into()]],
            ..Default::default()
        };
        let exact = assess_music_release(&input, &graph);
        assert!(exact.reasons.contains(&"tracklist".into()));
        assert!(exact.needs_review);
        let flattened = MusicMatchInput {
            disc_count: Some(1),
            tracklists: vec![vec!["Song 1".into(), "Song 2".into()]],
            ..Default::default()
        };
        let result = assess_music_release(&flattened, &graph);
        assert!(result.conflicts.contains(&"tracklist".into()));
        assert!(result.conflicts.contains(&"disc_count".into()));
    }

    #[test]
    fn matching_retains_explicit_remaster_and_deluxe_evidence() {
        let graph = edition();
        let exact = MusicMatchInput {
            edition: Some("remastered deluxe".into()),
            ..Default::default()
        };
        assert!(assess_music_release(&exact, &graph)
            .reasons
            .contains(&"edition".into()));
        let other = MusicMatchInput {
            edition: Some("original master".into()),
            ..Default::default()
        };
        assert!(assess_music_release(&other, &graph)
            .conflicts
            .contains(&"edition".into()));
    }

    #[test]
    fn partial_dates_match_only_their_actual_precision() {
        let mut graph = edition();
        for date in ["1999", "1999-02", "1999-02-03"] {
            graph.release_date_text = Some(date.into());
            let input = MusicMatchInput {
                date: Some(date.into()),
                ..Default::default()
            };
            assert!(assess_music_release(&input, &graph)
                .reasons
                .contains(&"date".into()));
        }
        let input = MusicMatchInput {
            date: Some("1999".into()),
            ..Default::default()
        };
        assert!(assess_music_release(&input, &graph)
            .conflicts
            .contains(&"date".into()));
    }
}
