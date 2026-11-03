//! Typed ordinary-video scraping contracts; no credentials or filesystem authority.
use crate::{ArtworkCandidate, EpisodeOrder, MetadataValue};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteId {
    pub namespace: String,
    pub value: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VideoQuery {
    Movie {
        title: String,
        year: Option<i32>,
        external_ids: Vec<RemoteId>,
        language: String,
    },
    Tv {
        title: String,
        year: Option<i32>,
        external_ids: Vec<RemoteId>,
        language: String,
    },
    Anime {
        title: String,
        year: Option<i32>,
        external_ids: Vec<RemoteId>,
        language: String,
    },
}
impl VideoQuery {
    pub fn parts(&self) -> (&str, Option<i32>, &[RemoteId], &str) {
        match self {
            Self::Movie {
                title,
                year,
                external_ids,
                language,
            }
            | Self::Tv {
                title,
                year,
                external_ids,
                language,
            }
            | Self::Anime {
                title,
                year,
                external_ids,
                language,
            } => (title, *year, external_ids, language),
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Movie { .. } => "movie",
            Self::Tv { .. } => "tv",
            Self::Anime { .. } => "anime",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateData {
    pub provider_id: String,
    pub remote_id: String,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub aliases: Vec<String>,
    pub external_ids: Vec<RemoteId>,
    pub source_url: String,
    pub language: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum VideoCandidate {
    Movie(CandidateData),
    Tv(CandidateData),
    Anime(CandidateData),
}
impl VideoCandidate {
    pub fn data(&self) -> &CandidateData {
        match self {
            Self::Movie(d) | Self::Tv(d) | Self::Anime(d) => d,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Movie(_) => "movie",
            Self::Tv(_) => "tv",
            Self::Anime(_) => "anime",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteField {
    pub key: String,
    pub value: serde_json::Value,
    pub language: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EpisodeDetail {
    pub remote_id: String,
    pub season_number: u32,
    pub episode_number: u32,
    pub title: String,
    pub overview: Option<String>,
    pub order: EpisodeOrder,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeasonDetail {
    pub remote_id: String,
    pub season_number: u32,
    pub title: String,
    pub episodes: Vec<EpisodeDetail>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MovieDetail {
    pub identity: CandidateData,
    pub fields: Vec<RemoteField>,
    pub artwork: Vec<ArtworkCandidate>,
    pub runtime_minutes: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TvDetail {
    pub identity: CandidateData,
    pub fields: Vec<RemoteField>,
    pub artwork: Vec<ArtworkCandidate>,
    pub seasons: Vec<SeasonDetail>,
    pub order: EpisodeOrder,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnimeDetail {
    pub identity: CandidateData,
    pub fields: Vec<RemoteField>,
    pub artwork: Vec<ArtworkCandidate>,
    pub format: Option<String>,
    pub season: Option<String>,
    pub episodes: Option<u32>,
    pub aliases: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum VideoMetadata {
    Movie(MovieDetail),
    Tv(TvDetail),
    Anime(AnimeDetail),
}
impl VideoMetadata {
    pub fn identity(&self) -> &CandidateData {
        match self {
            Self::Movie(d) => &d.identity,
            Self::Tv(d) => &d.identity,
            Self::Anime(d) => &d.identity,
        }
    }
    pub fn fields(&self) -> &[RemoteField] {
        match self {
            Self::Movie(d) => &d.fields,
            Self::Tv(d) => &d.fields,
            Self::Anime(d) => &d.fields,
        }
    }
    pub fn artwork(&self) -> &[ArtworkCandidate] {
        match self {
            Self::Movie(d) => &d.artwork,
            Self::Tv(d) => &d.artwork,
            Self::Anime(d) => &d.artwork,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScraperSearchRequest {
    pub query: VideoQuery,
    pub providers: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedCandidate {
    pub candidate: VideoCandidate,
    pub reasons: Vec<String>,
    pub needs_review: bool,
    pub duplicate_of: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPreview {
    pub schema_version: u16,
    pub run_id: Uuid,
    pub candidates: Vec<MatchedCandidate>,
    pub failures: Vec<ProviderSearchFailure>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSearchFailure {
    pub provider_id: String,
    pub failure: crate::ProviderFailure,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    pub run_id: Uuid,
    pub candidate_index: usize,
    pub media_item_id: Uuid,
    pub selected_fields: Vec<String>,
    pub selected_artwork: Vec<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDiff {
    pub key: String,
    pub language: Option<String>,
    pub old: Option<MetadataValue>,
    pub new: RemoteField,
    pub provider_id: String,
    pub source_url: String,
    pub protected: bool,
    pub needs_review: bool,
    pub selected: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergePreview {
    pub schema_version: u16,
    pub preview_id: Uuid,
    pub media_item_id: Uuid,
    pub fields: Vec<FieldDiff>,
    pub artwork: Vec<ArtworkCandidate>,
    pub confirmed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub schema_version: u16,
    pub preview_id: Uuid,
    pub applied_fields: usize,
    pub already_applied: bool,
}
