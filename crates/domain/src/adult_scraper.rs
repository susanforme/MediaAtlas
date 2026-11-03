//! Independent adult-workflow wire contracts. They convey no filesystem authority.
use crate::{ArtworkCandidate, CatalogNumber, FieldDiff, ProviderSearchFailure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdultQuery {
    pub catalog_number: String,
    pub namespace: String,
    pub studio: Option<String>,
    pub language: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdultCandidate {
    pub provider_id: String,
    pub remote_id: String,
    pub catalog_number: CatalogNumber,
    pub title: String,
    pub studio: Option<String>,
    pub source_url: String,
    pub language: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdultPerson {
    pub provider_id: String,
    pub remote_id: String,
    pub name: String,
    pub aliases: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdultDetail {
    pub identity: AdultCandidate,
    pub label: Option<String>,
    pub series: Option<String>,
    pub release_date: Option<String>,
    pub runtime_minutes: Option<u32>,
    pub performers: Vec<AdultPerson>,
    pub directors: Vec<AdultPerson>,
    pub artwork: Vec<ArtworkCandidate>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdultSearchRequest {
    pub query: AdultQuery,
    pub providers: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdultMatchedCandidate {
    pub candidate: AdultCandidate,
    pub reasons: Vec<String>,
    pub needs_review: bool,
    pub duplicate_of: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdultSearchPreview {
    pub schema_version: u16,
    pub run_id: Uuid,
    pub candidates: Vec<AdultMatchedCandidate>,
    pub failures: Vec<ProviderSearchFailure>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdultMergeRequest {
    pub run_id: Uuid,
    pub candidate_index: usize,
    pub media_item_id: Uuid,
    pub selected_fields: Vec<String>,
    pub selected_artwork: Vec<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdultMergePreview {
    pub schema_version: u16,
    pub preview_id: Uuid,
    pub media_item_id: Uuid,
    pub fields: Vec<FieldDiff>,
    pub artwork: Vec<ArtworkCandidate>,
    pub confirmed: bool,
}
