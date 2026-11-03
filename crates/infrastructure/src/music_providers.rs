//! Independent MusicBrainz / Cover Art Archive adapters; all HTTP uses NetworkEngine.
use crate::{
    media_output::{CachedArtwork, MediaOutputService},
    network::{CancellationToken, EngineKind, NetworkEngine, RequestSpec},
    providers::ProviderService,
};
use chrono::{NaiveDate, Utc};
use mediaatlas_domain::*;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

const PROJECT_UA: &str = "MediaAtlas/0.1.0 (https://github.com/susanforme/MediaAtlas)";
const MAX_TRACKS: usize = 2000;
const MAX_ARTISTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicSearchCandidate {
    pub mbid: String,
    pub title: String,
    pub artist: String,
    pub country: Option<String>,
    pub date: Option<String>,
    pub disambiguation: Option<String>,
    pub medium_count: u32,
    pub track_count: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicCover {
    pub url: String,
    pub source_url: String,
    pub license: String,
}
#[derive(Clone)]
pub struct MusicProviderHub {
    engine: Arc<NetworkEngine>,
    providers: Option<ProviderService>,
    mb_base: String,
    caa_base: String,
}
impl MusicProviderHub {
    pub fn new(engine: Arc<NetworkEngine>) -> Self {
        Self {
            engine,
            providers: None,
            mb_base: "https://musicbrainz.org/ws/2".into(),
            caa_base: "https://coverartarchive.org".into(),
        }
    }
    /// Trusted desktop composition snapshots the user's saved enabled/limit settings.
    pub fn with_provider_service(mut self, providers: ProviderService) -> Self {
        self.providers = Some(providers);
        self
    }
    pub fn descriptors() -> Vec<ProviderDescriptor> {
        vec![descriptor("musicbrainz", "MusicBrainz", "https://musicbrainz.org", "https://musicbrainz.org/doc/About/Data_License", "Core metadata CC0; supplementary data CC BY-SA 3.0", vec!["search", "release", "artist", "recording"]),
            descriptor("coverartarchive", "Cover Art Archive", "https://coverartarchive.org", "https://musicbrainz.org/doc/Cover_Art_Archive", "Individual cover images remain subject to their own copyright; no blanket image reuse license", vec!["artwork"])]
    }
    async fn job(
        &self,
        provider: &str,
        operation: &str,
        query: &str,
    ) -> Result<ProviderJob, ProviderFailure> {
        let mut config = if let Some(providers) = &self.providers {
            providers
                .config(provider)
                .await
                .map_err(|_| ProviderFailure::InvalidConfiguration)?
        } else {
            ProviderConfig {
                enabled: true,
                ..ProviderConfig::default()
            }
        };
        if config.credential_ref.is_some() {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        config.max_response_bytes = config.max_response_bytes.min(8 * 1024 * 1024);
        if provider == "musicbrainz" {
            config.interval_ms = config.interval_ms.max(1000);
            config.concurrency = 1;
        }
        Ok(ProviderJob {
            id: Uuid::new_v4(),
            provider_id: provider.into(),
            media_item_id: None,
            media_kind: Some(MediaKind::MusicRelease),
            file_identity: None,
            region: None,
            correlation_id: Uuid::new_v4(),
            operation: operation.into(),
            query: query.into(),
            language: "en-US".into(),
            config,
            created_at: Utc::now(),
        })
    }
    async fn json<T>(
        &self,
        provider: &str,
        url: Url,
        cancel: &CancellationToken,
        decode: impl FnOnce(Value) -> Result<T, ProviderFailure>,
    ) -> Result<T, ProviderFailure> {
        let job = self.job(provider, "music_metadata", "").await?;
        let mut spec = RequestSpec::get(url.as_str(), EngineKind::Api);
        spec.user_agent = Some(PROJECT_UA.into());
        let token = if let Some(providers) = &self.providers {
            providers
                .begin_music_job(&job, cancel.clone())
                .await
                .map_err(|_| ProviderFailure::InvalidConfiguration)?
        } else {
            cancel.clone()
        };
        let result = self
            .engine
            .execute(&job, spec, &token)
            .await
            .and_then(|response| response.json())
            .and_then(decode);
        if let Some(providers) = &self.providers {
            providers
                .finish_music_job(&job, result.as_ref().err().copied())
                .await
                .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        } else {
            self.engine.take_records(job.id).await;
        }
        result
    }
    pub async fn search(
        &self,
        query: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<MusicSearchCandidate>, ProviderFailure> {
        if query.trim().is_empty() || query.len() > 512 || query.chars().any(char::is_control) {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let mut url = Url::parse(&format!("{}/release", self.mb_base))
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        url.query_pairs_mut()
            .append_pair("query", query)
            .append_pair("fmt", "json")
            .append_pair("limit", "25");
        self.json("musicbrainz", url, cancel, |data| {
            let rows = array(&data, "releases", 25)?;
            rows.iter()
                .map(|row| {
                    let mbid = id(row)?;
                    let media = row.get("media").and_then(Value::as_array);
                    let medium_count = media
                        .map(|v| u32::try_from(v.len()).unwrap_or(u32::MAX))
                        .unwrap_or(0);
                    let track_count = media
                        .and_then(|v| {
                            v.iter().try_fold(0_u32, |acc, m| {
                                acc.checked_add(
                                    m.get("track-count")
                                        .and_then(Value::as_u64)
                                        .and_then(|n| u32::try_from(n).ok())
                                        .unwrap_or(0),
                                )
                            })
                        })
                        .unwrap_or(0);
                    Ok(MusicSearchCandidate {
                        mbid: mbid.to_string(),
                        title: text(row, "title")?,
                        artist: credit_text(row),
                        country: optional(row, "country"),
                        date: optional(row, "date"),
                        disambiguation: optional(row, "disambiguation"),
                        medium_count,
                        track_count,
                    })
                })
                .collect()
        })
        .await
    }
    pub async fn release(
        &self,
        mbid: &str,
        cancel: &CancellationToken,
    ) -> Result<MusicReleaseGraph, ProviderFailure> {
        let mbid = parse_mbid(mbid)?;
        let mut url = Url::parse(&format!("{}/release/{mbid}", self.mb_base))
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        url.query_pairs_mut()
            .append_pair("fmt", "json")
            .append_pair(
                "inc",
                "artists+artist-credits+recordings+release-groups+labels+isrcs",
            );
        let mut graph = self
            .json("musicbrainz", url, cancel, |data| {
                parse_release(&data, mbid)
            })
            .await?;
        // Artist aliases are a separate real API endpoint. Keep the set bounded,
        // and retain each credit's join phrase and credited name independently.
        for artist in &mut graph.artists {
            let mut url = Url::parse(&format!("{}/artist/{}", self.mb_base, artist.media_item_id))
                .map_err(|_| ProviderFailure::InvalidConfiguration)?;
            url.query_pairs_mut()
                .append_pair("fmt", "json")
                .append_pair("inc", "aliases");
            let expected = artist.media_item_id;
            *artist = self
                .json("musicbrainz", url, cancel, |data| {
                    if id(&data)? != expected {
                        return Err(ProviderFailure::IdentityMismatch);
                    }
                    parse_artist(&data)
                })
                .await?;
        }
        Ok(graph)
    }
    pub async fn cover(
        &self,
        mbid: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<MusicCover>, ProviderFailure> {
        let mbid = parse_mbid(mbid)?;
        let url = Url::parse(&format!("{}/release/{mbid}", self.caa_base))
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        self.json("coverartarchive",url,cancel,|data| {
        let rows = array(&data, "images", 100)?;
        let mut result = Vec::new();
        for row in rows {
            if row.get("front").and_then(Value::as_bool) != Some(true) {
                continue;
            }
            let image_id = row
                .get("id")
                .and_then(|v| {
                    v.as_u64()
                        .map(|n| n.to_string())
                        .or_else(|| v.as_str().map(str::to_owned))
                })
                .ok_or(ProviderFailure::ParseError)?;
            if image_id.is_empty()
                || image_id.len() > 32
                || !image_id.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(ProviderFailure::ParseError);
            }
            // Construct the CAA resource from validated identity. Never trust an
            // arbitrary metadata image URL or turn a broad archive host into a fetch proxy.
            result.push(MusicCover {url:format!("{}/release/{mbid}/{image_id}.jpg",self.caa_base),source_url:format!("https://coverartarchive.org/release/{mbid}"),license:"Cover copyright belongs to its rights holders; CAA does not grant a blanket reuse license".into()});
        }
        Ok(result)
        }).await
    }
    pub async fn cache_cover(
        &self,
        output: &MediaOutputService,
        cover: &MusicCover,
        cancel: &CancellationToken,
    ) -> Result<CachedArtwork, ProviderFailure> {
        let url = Url::parse(&cover.url).map_err(|_| ProviderFailure::UnsafeTarget)?;
        #[cfg(not(test))]
        if url.host_str() != Some("coverartarchive.org") || url.scheme() != "https" {
            return Err(ProviderFailure::UnsafeTarget);
        }
        let parts = url
            .path_segments()
            .ok_or(ProviderFailure::UnsafeTarget)?
            .collect::<Vec<_>>();
        if parts.len() != 3
            || parts[0] != "release"
            || parse_mbid(parts[1]).is_err()
            || !parts[2].ends_with(".jpg")
            || !parts[2]
                .trim_end_matches(".jpg")
                .bytes()
                .all(|b| b.is_ascii_digit())
        {
            return Err(ProviderFailure::UnsafeTarget);
        }
        let mut job = self.job("coverartarchive", "music_cover", "").await?;
        job.config.max_response_bytes = job.config.max_response_bytes.min(8 * 1024 * 1024);
        let token = if let Some(providers) = &self.providers {
            providers
                .begin_music_job(&job, cancel.clone())
                .await
                .map_err(|_| ProviderFailure::InvalidConfiguration)?
        } else {
            cancel.clone()
        };
        let result = output
            .cache_artwork(&self.engine, &job, &cover.url, &token)
            .await;
        if let Some(providers) = &self.providers {
            providers
                .finish_music_job(&job, result.as_ref().err().copied())
                .await
                .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        } else {
            self.engine.take_records(job.id).await;
        }
        result
    }
}
fn descriptor(
    id: &str,
    name: &str,
    website: &str,
    terms: &str,
    license: &str,
    capabilities: Vec<&str>,
) -> ProviderDescriptor {
    ProviderDescriptor {
        id: id.into(),
        name: name.into(),
        description: format!("Independent music {name} API"),
        website: Some(website.into()),
        terms_url: Some(terms.into()),
        privacy_url: Some("https://metabrainz.org/privacy".into()),
        license: license.into(),
        media_kinds: vec![
            MediaKind::MusicArtist,
            MediaKind::MusicReleaseGroup,
            MediaKind::MusicRelease,
            MediaKind::MusicTrack,
            MediaKind::MusicRecording,
        ],
        languages: vec!["zh-CN".into(), "en-US".into()],
        capabilities: capabilities.into_iter().map(str::to_owned).collect(),
        fixture: false,
        documentation_url: Some(terms.into()),
        engine_types: vec!["api".into()],
        requires_api_key: false,
        requires_login: false,
        rate_limit_policy: if id == "musicbrainz" {
            "At most one request per second; non-configurable site minimum; honors Retry-After"
                .into()
        } else {
            "Bounded shared image concurrency, redirects and Retry-After".into()
        },
        attribution: format!("Metadata/source: {name}; {website}"),
        data_usage_notes: if id == "musicbrainz" {
            "Core metadata CC0. Supplementary metadata may be CC BY-SA. No audio is downloaded."
                .into()
        } else {
            "Cover images have individual copyright; retain source and rights notice.".into()
        },
    }
}
fn parse_mbid(value: &str) -> Result<Uuid, ProviderFailure> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or(ProviderFailure::InvalidConfiguration)
}
fn id(value: &Value) -> Result<Uuid, ProviderFailure> {
    parse_mbid(
        value
            .get("id")
            .and_then(Value::as_str)
            .ok_or(ProviderFailure::ParseError)?,
    )
    .map_err(|_| ProviderFailure::ParseError)
}
fn text(value: &Value, key: &str) -> Result<String, ProviderFailure> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 8192 && !s.contains('\0'))
        .map(str::to_owned)
        .ok_or(ProviderFailure::ParseError)
}
fn optional(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 8192 && !s.contains('\0'))
        .map(str::to_owned)
}
fn array<'a>(value: &'a Value, key: &str, max: usize) -> Result<&'a Vec<Value>, ProviderFailure> {
    let rows = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or(ProviderFailure::ParseError)?;
    if rows.len() > max {
        Err(ProviderFailure::ResponseTooLarge)
    } else {
        Ok(rows)
    }
}
fn date(value: &Value, key: &str) -> Option<NaiveDate> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| s.len() == 10)
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
}
fn credit_text(value: &Value) -> String {
    value
        .get("artist-credit")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|v| {
                    v.get("name")
                        .or_else(|| v.get("artist").and_then(|a| a.get("name")))
                })
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .unwrap_or_default()
}
fn parse_artist(value: &Value) -> Result<MusicArtist, ProviderFailure> {
    let aliases = if let Some(rows) = value.get("aliases").and_then(Value::as_array) {
        if rows.len() > 500 {
            return Err(ProviderFailure::ResponseTooLarge);
        }
        rows.iter()
            .map(|v| text(v, "name"))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![]
    };
    Ok(MusicArtist {
        media_item_id: id(value)?,
        name: text(value, "name")?,
        sort_name: optional(value, "sort-name"),
        aliases,
    })
}
fn credits(
    value: &Value,
    artists: &mut BTreeMap<Uuid, MusicArtist>,
) -> Result<Vec<ArtistCredit>, ProviderFailure> {
    let Some(rows) = value.get("artist-credit").and_then(Value::as_array) else {
        return Ok(vec![]);
    };
    if rows.len() > MAX_ARTISTS {
        return Err(ProviderFailure::ResponseTooLarge);
    }
    let mut result = Vec::new();
    for row in rows {
        let artist = parse_artist(row.get("artist").ok_or(ProviderFailure::ParseError)?)?;
        let artist_id = artist.media_item_id;
        let credited_name = optional(row, "name").unwrap_or_else(|| artist.name.clone());
        artists.entry(artist_id).or_insert(artist);
        result.push(ArtistCredit {
            artist_id,
            credited_name,
            join_phrase: optional(row, "joinphrase").unwrap_or_default(),
        });
    }
    if artists.len() > MAX_ARTISTS {
        return Err(ProviderFailure::ResponseTooLarge);
    }
    Ok(result)
}
fn mb_identifier(media_item_id: Uuid, namespace: &str) -> MusicIdentifier {
    MusicIdentifier {
        media_item_id,
        kind: MusicIdentifierKind::MusicBrainz,
        namespace: namespace.into(),
        value: media_item_id.to_string(),
    }
}
fn medium_id(release: Uuid, position: u32) -> Uuid {
    let hash = Sha256::digest(format!("mediaatlas:music:medium:{release}:{position}").as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
fn position(value: &Value) -> Result<u32, ProviderFailure> {
    value
        .get("position")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|p| *p > 0)
        .ok_or(ProviderFailure::ParseError)
}
fn parse_release(value: &Value, expected: Uuid) -> Result<MusicReleaseGraph, ProviderFailure> {
    let release_id = id(value)?;
    if release_id != expected {
        return Err(ProviderFailure::IdentityMismatch);
    }
    let group = value
        .get("release-group")
        .ok_or(ProviderFailure::ParseError)?;
    let group_id = id(group)?;
    let mut artists = BTreeMap::new();
    let release_credits = credits(value, &mut artists)?;
    let mut group_credits = credits(group, &mut artists)?;
    if group_credits.is_empty() {
        group_credits = release_credits.clone();
    }
    let release_group = MusicReleaseGroup {
        media_item_id: group_id,
        title: text(group, "title")?,
        artist_credits: group_credits,
        primary_type: optional(group, "primary-type"),
        secondary_types: group
            .get("secondary-types")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        first_release_date: date(group, "first-release-date"),
    };
    let mut identifiers = vec![
        mb_identifier(release_id, "release"),
        mb_identifier(group_id, "release_group"),
    ];
    if let Some(barcode) = optional(value, "barcode") {
        // MusicBrainz may contain nonstandard barcode notes. Retain valid GTIN
        // identifiers without rejecting otherwise useful release metadata.
        if barcode.bytes().all(|b| b.is_ascii_digit()) && matches!(barcode.len(), 8 | 12 | 13 | 14)
        {
            identifiers.push(MusicIdentifier {
                media_item_id: release_id,
                kind: if barcode.len() == 12 {
                    MusicIdentifierKind::Upc
                } else {
                    MusicIdentifierKind::Ean
                },
                namespace: if barcode.len() == 14 {
                    "gtin14".into()
                } else {
                    "barcode".into()
                },
                value: barcode,
            });
        }
    }
    let mut labels = Vec::new();
    if let Some(rows) = value.get("label-info").and_then(Value::as_array) {
        if rows.len() > 100 {
            return Err(ProviderFailure::ResponseTooLarge);
        }
        for label in rows {
            if let Some(label) = label.get("label").filter(|v| !v.is_null()) {
                labels.push(id(label)?);
            }
            if let Some(catalog) = optional(label, "catalog-number") {
                identifiers.push(MusicIdentifier {
                    media_item_id: release_id,
                    kind: MusicIdentifierKind::CatalogNumber,
                    namespace: label
                        .get("label")
                        .and_then(|v| v.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or("label")
                        .into(),
                    value: catalog,
                });
            }
        }
    }
    let release = MusicRelease {
        media_item_id: release_id,
        release_group_id: group_id,
        title: text(value, "title")?,
        artist_credits: release_credits,
        country: optional(value, "country"),
        release_date: date(value, "date"),
        status: optional(value, "status"),
        labels,
    };
    let mut media = Vec::new();
    let mut tracks = Vec::new();
    let mut recordings = BTreeMap::new();
    for m in array(value, "media", 100)? {
        let disc = position(m)?;
        let mid = medium_id(release_id, disc);
        media.push(MusicMedium {
            id: mid,
            release_id,
            position: disc,
            title: optional(m, "title"),
            format: optional(m, "format"),
        });
        for t in array(m, "tracks", MAX_TRACKS)? {
            if tracks.len() >= MAX_TRACKS {
                return Err(ProviderFailure::ResponseTooLarge);
            }
            let track_id = id(t)?;
            let recording = t.get("recording").ok_or(ProviderFailure::ParseError)?;
            let recording_id = id(recording)?;
            let mut track_credits = credits(t, &mut artists)?;
            let mut recording_credits = credits(recording, &mut artists)?;
            if track_credits.is_empty() {
                track_credits = recording_credits.clone();
            }
            if recording_credits.is_empty() {
                recording_credits = track_credits.clone();
            }
            tracks.push(MusicTrack {
                media_item_id: track_id,
                medium_id: mid,
                recording_id,
                position: position(t)?,
                number: text(t, "number")?,
                title: optional(t, "title").unwrap_or(text(recording, "title")?),
                length_ms: t
                    .get("length")
                    .and_then(Value::as_u64)
                    .or_else(|| recording.get("length").and_then(Value::as_u64)),
                artist_credits: track_credits,
            });
            if let std::collections::btree_map::Entry::Vacant(entry) =
                recordings.entry(recording_id)
            {
                identifiers.push(mb_identifier(recording_id, "recording"));
                if let Some(isrcs) = recording.get("isrcs").and_then(Value::as_array) {
                    if isrcs.len() > 100 {
                        return Err(ProviderFailure::ResponseTooLarge);
                    }
                    for isrc in isrcs {
                        let isrc = isrc.as_str().ok_or(ProviderFailure::ParseError)?;
                        if isrc.len() != 12 || !isrc.bytes().all(|b| b.is_ascii_alphanumeric()) {
                            return Err(ProviderFailure::ParseError);
                        }
                        identifiers.push(MusicIdentifier {
                            media_item_id: recording_id,
                            kind: MusicIdentifierKind::Isrc,
                            namespace: "isrc".into(),
                            value: isrc.into(),
                        });
                    }
                }
                entry.insert(MusicRecording {
                    media_item_id: recording_id,
                    title: text(recording, "title")?,
                    length_ms: recording.get("length").and_then(Value::as_u64),
                    artist_credits: recording_credits,
                });
            }
            identifiers.push(mb_identifier(track_id, "track"));
        }
    }
    identifiers.extend(artists.keys().map(|id| mb_identifier(*id, "artist")));
    Ok(MusicReleaseGraph {
        artists: artists.into_values().collect(),
        release_group,
        release,
        media,
        tracks,
        recordings: recordings.into_values().collect(),
        identifiers,
        source_url: format!("https://musicbrainz.org/release/{release_id}"),
        fetched_at: Utc::now(),
        locale: None,
        files: vec![],
        field_sources: vec![],
        release_date_text: optional(value, "date"),
        group_first_release_date_text: optional(group, "first-release-date"),
        release_disambiguation: optional(value, "disambiguation"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{library::LibraryRepository, providers::TypedVideoRegistration};
    use serde_json::json;
    use std::{
        sync::Mutex,
        time::{Duration, Instant},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };
    const RELEASE: &str = "11111111-1111-4111-8111-111111111111";
    const ARTIST: &str = "22222222-2222-4222-8222-222222222222";
    const GROUP: &str = "33333333-3333-4333-8333-333333333333";
    const RECORDING: &str = "44444444-4444-4444-8444-444444444444";
    const TRACK: &str = "55555555-5555-4555-8555-555555555555";
    fn release_json() -> Value {
        json!({"id":RELEASE,"title":"Album","country":"JP","date":"2003-05","disambiguation":"Deluxe remaster","status":"Official","barcode":"012345678905","label-info":[{"catalog-number":"CAT-12","label":{"id":"66666666-6666-4666-8666-666666666666"}}],"artist-credit":[{"name":"Credited Artist","joinphrase":" feat. ","artist":{"id":ARTIST,"name":"Artist","sort-name":"Artist, The"}}],"release-group":{"id":GROUP,"title":"Album","primary-type":"Album","first-release-date":"2001"},"media":[{"position":1,"format":"CD","tracks":[{"id":TRACK,"position":1,"number":"A1","title":"Track one","length":1234,"recording":{"id":RECORDING,"title":"Recording","length":1234,"isrcs":["JPAAA0300001"]}}]},{"position":2,"title":"Bonus disc","format":"CD","tracks":[{"id":"77777777-7777-4777-8777-777777777777","position":1,"number":"1","title":"Bonus","recording":{"id":RECORDING,"title":"Recording","length":1234,"isrcs":["JPAAA0300001"]}}]}]})
    }
    type Requests = Arc<Mutex<Vec<(Instant, String)>>>;
    async fn server() -> (String, Requests, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let captured = Arc::new(Mutex::new(Vec::new()));
        let requests = captured.clone();
        let image = {
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(4, 4)
                .write_to(&mut bytes, image::ImageFormat::Png)
                .unwrap();
            bytes.into_inner()
        };
        let task = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let requests = requests.clone();
                let image = image.clone();
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    loop {
                        let mut buf = [0_u8; 4096];
                        let n = stream.read(&mut buf).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&buf[..n]);
                        if bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                            break;
                        }
                        if bytes.len() > 65536 {
                            return;
                        }
                    }
                    let request = String::from_utf8_lossy(&bytes).into_owned();
                    requests
                        .lock()
                        .unwrap()
                        .push((Instant::now(), request.clone()));
                    let path = request.lines().next().unwrap_or_default();
                    let (status, mime, body, headers) = if path.contains("/artist/") {
                        (200,"application/json",serde_json::to_vec(&json!({"id":ARTIST,"name":"Artist","sort-name":"Artist, The","aliases":[{"name":"別名"}]})).unwrap(),"")
                    } else if path.contains("99999.jpg") {
                        (
                            302,
                            "application/json",
                            b"{}".to_vec(),
                            "Location: https://example.org/picture.jpg\r\n",
                        )
                    } else if path.contains(".jpg") {
                        (200, "image/png", image, "")
                    } else if path.contains("/ws/2/release/") {
                        (
                            200,
                            "application/json",
                            serde_json::to_vec(&release_json()).unwrap(),
                            "",
                        )
                    } else if path.contains("query=retry") {
                        (
                            429,
                            "application/json",
                            b"{}".to_vec(),
                            "Retry-After: 2\r\n",
                        )
                    } else if path.contains("query=redirect") {
                        (
                            302,
                            "application/json",
                            b"{}".to_vec(),
                            "Location: /ws/2/release?query=x\r\n",
                        )
                    } else if path.contains("/ws/2/release?") {
                        (200,"application/json",serde_json::to_vec(&json!({"releases":[{"id":RELEASE,"title":"Album","country":"JP","date":"2003-05","disambiguation":"Deluxe remaster","artist-credit":[{"name":"Artist"}],"media":[{"track-count":1},{"track-count":1}]}]})).unwrap(),"")
                    } else {
                        (200,"application/json",serde_json::to_vec(&json!({"images":[{"id":12345,"front":true,"image":"http://evil.example/not-trusted.jpg"},{"id":12346,"front":false}]})).unwrap(),"")
                    };
                    let head=format!("HTTP/1.1 {status} Mock\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",body.len());
                    let _ = stream.write_all(head.as_bytes()).await;
                    let _ = stream.write_all(&body).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        (base, captured, task)
    }
    fn mock_hub(base: &str) -> MusicProviderHub {
        let mut hub = MusicProviderHub::new(Arc::new(NetworkEngine::for_loopback(4)));
        hub.mb_base = format!("{base}/ws/2");
        hub.caa_base = base.into();
        hub
    }
    #[test]
    fn graph_retains_concrete_editions_multidisc_recording_identity_and_partial_dates() {
        let graph = parse_release(&release_json(), parse_mbid(RELEASE).unwrap()).unwrap();
        assert_eq!(graph.media.len(), 2);
        assert_eq!(graph.tracks.len(), 2);
        assert_eq!(graph.recordings.len(), 1);
        assert_eq!(graph.tracks[0].number, "A1");
        assert_ne!(graph.media[0].id, graph.media[1].id);
        assert_eq!(graph.release.release_date, None);
        assert_eq!(graph.release_date_text.as_deref(), Some("2003-05"));
        assert_eq!(graph.group_first_release_date_text.as_deref(), Some("2001"));
        assert_eq!(
            graph.release_disambiguation.as_deref(),
            Some("Deluxe remaster")
        );
        assert_eq!(graph.release.artist_credits[0].join_phrase, " feat. ");
        assert!(graph
            .identifiers
            .iter()
            .any(|v| v.kind == MusicIdentifierKind::Upc && v.value == "012345678905"));
        assert!(graph
            .identifiers
            .iter()
            .any(|v| v.kind == MusicIdentifierKind::CatalogNumber && v.value == "CAT-12"));
        assert!(graph
            .identifiers
            .iter()
            .any(|v| v.kind == MusicIdentifierKind::Isrc));
        let mut other = release_json();
        let other_id = "88888888-8888-4888-8888-888888888888";
        other["id"] = json!(other_id);
        other["country"] = json!("US");
        other["disambiguation"] = json!("Original edition");
        let other = parse_release(&other, parse_mbid(other_id).unwrap()).unwrap();
        assert_eq!(
            graph.release_group.media_item_id,
            other.release_group.media_item_id
        );
        assert_ne!(graph.release.media_item_id, other.release.media_item_id);
        assert_ne!(graph.media[0].id, other.media[0].id);
        let mut ean8 = release_json();
        ean8["barcode"] = json!("96385074");
        let ean8 = parse_release(&ean8, parse_mbid(RELEASE).unwrap()).unwrap();
        assert!(ean8
            .identifiers
            .iter()
            .any(|v| v.kind == MusicIdentifierKind::Ean && v.value == "96385074"));
        let mut nonstandard = release_json();
        nonstandard["barcode"] = json!("none");
        assert!(parse_release(&nonstandard, parse_mbid(RELEASE).unwrap()).is_ok());
        assert_eq!(
            parse_release(&release_json(), Uuid::new_v4()),
            Err(ProviderFailure::IdentityMismatch)
        );
    }
    #[tokio::test]
    async fn mock_real_search_details_aliases_rate_floor_saved_config_and_request_records() {
        let (base, requests, task) = server().await;
        let directory = crate::test_support::tempdir().unwrap();
        let repo = LibraryRepository::open(&directory.path().join("library.sqlite"))
            .await
            .unwrap();
        let providers = ProviderService::with_engine(repo, NetworkEngine::for_loopback(4))
            .await
            .unwrap();
        for desc in MusicProviderHub::descriptors() {
            providers
                .register_adapter(Arc::new(TypedVideoRegistration(desc)))
                .await
                .unwrap();
        }
        let mut hub = mock_hub(&base);
        hub.engine = providers.engine();
        let hub = hub.with_provider_service(providers.clone());
        let cancel = CancellationToken::new();
        assert_eq!(
            hub.search("Album", &cancel).await,
            Err(ProviderFailure::Disabled)
        );
        assert!(requests.lock().unwrap().is_empty());
        let mut config = providers.config("musicbrainz").await.unwrap();
        config.enabled = true;
        config.interval_ms = 0;
        config.max_retries = 0;
        providers
            .update_config("musicbrainz", &config)
            .await
            .unwrap();
        let candidates = hub.search("Album", &cancel).await.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].medium_count, 2);
        assert_eq!(candidates[0].track_count, 2);
        let graph = hub.release(RELEASE, &cancel).await.unwrap();
        assert_eq!(graph.artists[0].aliases, vec!["別名"]);
        assert_eq!(graph.locale, None);
        {
            let requests = requests.lock().unwrap();
            assert_eq!(requests.len(), 3);
            for pair in requests.windows(2) {
                assert!(pair[1].0.duration_since(pair[0].0) >= Duration::from_millis(950));
            }
            assert!(requests
                .iter()
                .all(|(_, r)| r.to_ascii_lowercase().contains(
                    "user-agent: mediaatlas/0.1.0 (https://github.com/susanforme/mediaatlas)"
                )));
        }
        let snapshot = providers.snapshot().await.unwrap();
        assert_eq!(snapshot.requests.len(), 4);
        assert!(snapshot
            .requests
            .iter()
            .any(|r| r.failure == Some(ProviderFailure::Disabled)));
        assert!(snapshot
            .tasks
            .iter()
            .all(|t| matches!(t.state, TaskState::Succeeded | TaskState::Failed)));
        assert!(hub.engine.records().await.is_empty());
        task.abort();
    }
    #[tokio::test]
    async fn mock_caa_download_reuses_bounded_hash_cache_and_ignores_arbitrary_remote_image_url() {
        let (base, requests, task) = server().await;
        let hub = mock_hub(&base);
        let cancel = CancellationToken::new();
        let covers = hub.cover(RELEASE, &cancel).await.unwrap();
        assert_eq!(covers.len(), 1);
        assert_eq!(covers[0].url, format!("{base}/release/{RELEASE}/12345.jpg"));
        assert!(covers[0].license.contains("copyright"));
        let directory = crate::test_support::tempdir().unwrap();
        let repo = LibraryRepository::open(&directory.path().join("library.sqlite"))
            .await
            .unwrap();
        let output = MediaOutputService::new(repo, directory.path().join("cache"));
        let image = hub.cache_cover(&output, &covers[0], &cancel).await.unwrap();
        assert_eq!(image.mime, "image/png");
        assert_eq!(image.width, 4);
        let second = hub.cache_cover(&output, &covers[0], &cancel).await.unwrap();
        assert_eq!(second.content_hash, image.content_hash);
        assert_eq!(requests.lock().unwrap().len(), 2);
        assert!(output.thumbnail_bytes(&image).is_ok());
        let unsafe_cover = MusicCover {
            url: format!("{base}/release/{RELEASE}/99999.jpg"),
            source_url: covers[0].source_url.clone(),
            license: covers[0].license.clone(),
        };
        assert!(matches!(
            hub.cache_cover(&output, &unsafe_cover, &cancel).await,
            Err(ProviderFailure::UnsafeTarget)
        ));
        assert_eq!(requests.lock().unwrap().len(), 3);
        task.abort();
    }
    #[tokio::test]
    async fn cancellation_redirect_rejection_and_retry_after_are_real_http_behaviors() {
        let (base, requests, task) = server().await;
        let hub = mock_hub(&base);
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            hub.search("Album", &cancel).await,
            Err(ProviderFailure::Cancelled)
        );
        assert!(requests.lock().unwrap().is_empty());
        let cancel = CancellationToken::new();
        assert_eq!(
            hub.search("redirect", &cancel).await,
            Err(ProviderFailure::UnsafeTarget)
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
        let waiter = CancellationToken::new();
        let child = waiter.clone();
        let pending = tokio::spawn(async move { hub.search("Album", &child).await });
        tokio::time::sleep(Duration::from_millis(30)).await;
        waiter.cancel();
        assert_eq!(pending.await.unwrap(), Err(ProviderFailure::Cancelled));
        assert_eq!(requests.lock().unwrap().len(), 1);
        let hub = mock_hub(&base);
        let started = Instant::now();
        assert!(matches!(
            hub.search("retry", &cancel).await,
            Err(ProviderFailure::RateLimited | ProviderFailure::CoolingDown)
        ));
        assert!(started.elapsed() >= Duration::from_secs(2));
        let requests = requests.lock().unwrap();
        for pair in requests[1..].windows(2) {
            assert!(pair[1].0.duration_since(pair[0].0) >= Duration::from_millis(1900));
        }
        drop(requests);
        task.abort();
    }
}
