use chrono::{DateTime, Utc};
use mediaatlas_domain::{ArtworkKind, MediaKind, PathStyle};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
};

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub kind: Option<MediaKind>,
    pub path_style: PathStyle,
    pub exclude_patterns: Vec<String>,
    pub known_files: HashMap<String, FileStamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStamp {
    pub size_bytes: u64,
    pub modified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanStats {
    pub visited_files: u64,
    pub skipped_files: u64,
    pub candidate_count: u64,
    pub warnings: u64,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScannedFile {
    pub relative_path: String,
    pub normalized_relative_path: String,
    pub size_bytes: u64,
    pub modified_at: Option<DateTime<Utc>>,
    pub parsed: ParsedMedia,
    pub local_metadata: LocalMetadata,
    pub artworks: Vec<LocalArtwork>,
    pub associated_files: Vec<String>,
    #[serde(default)]
    pub sidecar_fingerprint: String,
    #[serde(default)]
    pub exclude_patterns_snapshot: Vec<String>,
    pub unchanged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedMedia {
    pub kind: MediaKind,
    pub title: String,
    pub year: Option<i32>,
    pub part: Option<u32>,
    pub series_title: Option<String>,
    pub season_number: Option<u32>,
    pub episode_number: Option<u32>,
    pub absolute_number: Option<u32>,
    pub catalog_number: Option<String>,
    pub group_key: String,
    pub needs_review: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalMetadata {
    pub fields: BTreeMap<String, Value>,
    pub external_ids: Vec<LocalExternalId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalExternalId {
    pub provider_id: String,
    pub namespace: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalArtwork {
    pub relative_path: String,
    pub kind: ArtworkKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanError {
    InvalidRoot,
    InvalidPattern,
    Io,
    UnsafeXml,
    InvalidMetadata,
    MetadataTooLarge,
}

impl ScanError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRoot => "invalid_root",
            Self::InvalidPattern => "invalid_exclude_pattern",
            Self::Io => "local_file_unreadable",
            Self::UnsafeXml => "unsafe_xml",
            Self::InvalidMetadata => "invalid_local_metadata",
            Self::MetadataTooLarge => "local_metadata_too_large",
        }
    }
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for ScanError {}
