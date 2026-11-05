//! Phase 2 application wire contracts. No SQL, desktop framework or I/O dependency.

use chrono::{DateTime, Utc};
use mediaatlas_domain::{
    Artwork, ExternalId, LibraryRoot, MediaFile, MediaItem, MediaKind, MetadataValue, NfoExportMode, TaskState,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PHASE2_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub schema_version: u16,
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            schema_version: PHASE2_SCHEMA_VERSION,
            data,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootInput {
    pub id: Option<Uuid>,
    pub name: String,
    pub path: String,
    pub kind: Option<MediaKind>,
    pub enabled: bool,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default)]
    pub metadata_providers: Option<Vec<String>>,
    #[serde(default)]
    pub nfo_export_mode: Option<NfoExportMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootView {
    #[serde(flatten)]
    pub root: LibraryRoot,
    pub exclude_patterns: Vec<String>,
    pub last_scan_at: Option<DateTime<Utc>>,
    pub metadata_providers: Option<Vec<String>>,
    #[serde(default)]
    pub nfo_export_mode: NfoExportMode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RootListInput {
    #[serde(default)]
    pub include_adult: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveRootInput {
    pub root_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowseQuery {
    pub root_id: Option<Uuid>,
    pub kind: Option<MediaKind>,
    pub query: Option<String>,
    pub health: Option<String>,
    pub genre: Option<String>,
    pub person: Option<String>,
    pub studio: Option<String>,
    pub year: Option<i32>,
    #[serde(default = "default_sort")]
    pub sort: String,
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub include_adult: bool,
}

fn default_sort() -> String {
    "updated_desc".into()
}
fn default_limit() -> u32 {
    50
}

impl Default for BrowseQuery {
    fn default() -> Self {
        Self {
            root_id: None,
            kind: None,
            query: None,
            health: None,
            genre: None,
            person: None,
            studio: None,
            year: None,
            sort: default_sort(),
            cursor: None,
            limit: default_limit(),
            include_adult: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaCardView {
    pub media: MediaItem,
    pub poster_uri: Option<String>,
    pub health: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPageView {
    pub items: Vec<MediaCardView>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDetailInput {
    pub media_id: Uuid,
    #[serde(default)]
    pub include_adult: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRelationView {
    pub id: Uuid,
    pub kind: MediaKind,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDetailView {
    pub media: MediaItem,
    pub files: Vec<MediaFile>,
    pub artworks: Vec<Artwork>,
    pub external_ids: Vec<ExternalId>,
    pub metadata_values: Vec<MetadataValue>,
    pub relations: Vec<MediaRelationView>,
    pub health: Vec<String>,
    pub poster_uri: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibrarySummaryView {
    pub total_media: u64,
    pub total_roots: u64,
    pub pending_tasks: u64,
    pub failed_tasks: u64,
    pub missing_poster: u64,
    pub missing_nfo: u64,
    pub incomplete_metadata: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetOption {
    pub value: String,
    pub count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryFacetsView {
    pub genres: Vec<FacetOption>,
    pub people: Vec<FacetOption>,
    pub studios: Vec<FacetOption>,
    pub years: Vec<FacetOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanTaskView {
    pub id: Uuid,
    pub root_id: Uuid,
    pub state: TaskState,
    pub visited_files: u64,
    pub skipped_files: u64,
    pub candidate_count: u64,
    pub imported_count: u64,
    pub warnings: u64,
    pub error_code: Option<String>,
    pub mode: String,
    pub phase: String,
    pub matched_count: u64,
    pub review_count: u64,
    /// Subset of review_count awaiting an enabled, configured metadata source.
    pub unconfigured_count: u64,
    pub metadata_failed_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanCandidateView {
    pub id: Uuid,
    pub root_id: Uuid,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_at: Option<DateTime<Utc>>,
    pub kind: MediaKind,
    pub title: String,
    pub year: Option<i32>,
    pub part: Option<u32>,
    pub series_title: Option<String>,
    pub season_number: Option<u32>,
    pub episode_number: Option<u32>,
    pub absolute_number: Option<u32>,
    pub catalog_number: Option<String>,
    pub needs_review: bool,
    pub warnings: Vec<String>,
    pub existing_media_id: Option<Uuid>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartInput {
    pub root_id: Uuid,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListTasksInput {
    #[serde(default)]
    pub include_adult: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewInput {
    pub task_id: Uuid,
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub include_adult: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewPageView {
    pub task_id: Uuid,
    pub items: Vec<ScanCandidateView>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmInput {
    pub task_id: Uuid,
    pub candidate_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfirmResult {
    pub imported: u64,
    pub unchanged: u64,
    pub needs_review: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelInput {
    pub task_id: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mediaatlas_domain::PathStyle;
    use serde_json::json;

    #[test]
    fn omitted_adult_access_defaults_to_hidden_in_read_queries() {
        let query: BrowseQuery = serde_json::from_value(json!({})).unwrap();
        assert!(!query.include_adult);
        assert_eq!(query.limit, 50);
        assert_eq!(query.sort, "updated_desc");
        let input: RootListInput = serde_json::from_value(json!({})).unwrap();
        assert!(!input.include_adult);
    }

    #[test]
    fn root_view_and_new_response_use_the_frozen_flat_shape() {
        let now = Utc::now();
        let root = RootView {
            metadata_providers: None,
            nfo_export_mode: NfoExportMode::Online,
            root: LibraryRoot {
                id: Uuid::new_v4(),
                name: "Fixture".into(),
                path: "/fixture".into(),
                path_style: PathStyle::Posix,
                kind: Some(MediaKind::Movie),
                enabled: true,
                created_at: now,
                updated_at: now,
            },
            exclude_patterns: vec!["*.tmp".into()],
            last_scan_at: None,
        };
        let wire = serde_json::to_value(ApiResponse::new(root)).unwrap();
        assert_eq!(wire["schema_version"], 1);
        assert_eq!(wire["data"]["name"], "Fixture");
        assert!(wire["data"].get("root").is_none());
        assert_eq!(wire["data"]["exclude_patterns"], json!(["*.tmp"]));
        assert_eq!(wire["data"]["nfo_export_mode"], "online");
        let mut legacy = wire["data"].clone();
        legacy.as_object_mut().unwrap().remove("nfo_export_mode");
        assert_eq!(serde_json::from_value::<RootView>(legacy).unwrap().nfo_export_mode, NfoExportMode::Online);
    }

    #[test]
    fn root_nfo_mode_defaults_and_validates_the_wire_values() {
        let mut input = json!({"id":null,"name":"Movie","path":"/fixture","kind":"movie","enabled":true});
        let legacy: RootInput = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(legacy.nfo_export_mode, None);
        input["nfo_export_mode"] = json!("offline");
        let offline: RootInput = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(offline.nfo_export_mode, Some(NfoExportMode::Offline));
        input["nfo_export_mode"] = json!("unknown");
        assert!(serde_json::from_value::<RootInput>(input).is_err());
    }
}
