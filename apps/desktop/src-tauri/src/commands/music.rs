//! Ordinary music commands never grant access to the separate adult lane.
use super::{storage_error, DesktopState};
use mediaatlas_application::{
    library::ApiResponse,
    music::{assess_music_release, MusicMatchAssessment, MusicMatchInput},
};
use mediaatlas_desktop_bridge::ErrorDto;
use mediaatlas_domain::{MusicAlbumView, MusicArtistView, MusicLocalFile, MusicReleaseGraph};
use mediaatlas_infrastructure::{
    library::RepositoryError,
    music_providers::{MusicCover, MusicSearchCandidate},
    music_tags::{MusicTagPreview, MusicTagRead, MusicTagResult},
};
use std::collections::BTreeMap;
use tauri::State;
use uuid::Uuid;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicEmptyInput {}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicReleaseInput {
    release_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicArtistInput {
    artist_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicMbidInput {
    mbid: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicQueryInput {
    query: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicFileInput {
    file_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicLinkInput {
    track_id: Uuid,
    file_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicTagInput {
    file_id: Uuid,
    changes: BTreeMap<String, String>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicPreviewInput {
    preview_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicOperationInput {
    operation_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicCoverInput {
    mbid: String,
    cover_index: usize,
    release_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicMatchRequest {
    input: MusicMatchInput,
    release_id: Uuid,
}

#[tauri::command]
pub async fn music_library(
    state: State<'_, DesktopState>,
    input: MusicEmptyInput,
) -> Result<ApiResponse<Vec<MusicAlbumView>>, ErrorDto> {
    let _ = input;
    state
        .music
        .list()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_album(
    state: State<'_, DesktopState>,
    input: MusicReleaseInput,
) -> Result<ApiResponse<MusicReleaseGraph>, ErrorDto> {
    state
        .music
        .album(input.release_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_artist(
    state: State<'_, DesktopState>,
    input: MusicArtistInput,
) -> Result<ApiResponse<MusicArtistView>, ErrorDto> {
    state
        .music
        .artist(input.artist_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_local_files(
    state: State<'_, DesktopState>,
    input: MusicEmptyInput,
) -> Result<ApiResponse<Vec<MusicLocalFile>>, ErrorDto> {
    let _ = input;
    state
        .music
        .available_files()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_search(
    state: State<'_, DesktopState>,
    input: MusicQueryInput,
) -> Result<ApiResponse<Vec<MusicSearchCandidate>>, ErrorDto> {
    state
        .music_providers
        .search(
            &input.query,
            &mediaatlas_infrastructure::network::CancellationToken::new(),
        )
        .await
        .map(ApiResponse::new)
        .map_err(|e| storage_error(RepositoryError::Provider(e)))
}
#[tauri::command]
pub async fn music_release(
    state: State<'_, DesktopState>,
    input: MusicMbidInput,
) -> Result<ApiResponse<MusicReleaseGraph>, ErrorDto> {
    state
        .music_providers
        .release(
            &input.mbid,
            &mediaatlas_infrastructure::network::CancellationToken::new(),
        )
        .await
        .map(ApiResponse::new)
        .map_err(|e| storage_error(RepositoryError::Provider(e)))
}
#[tauri::command]
pub async fn music_import(
    state: State<'_, DesktopState>,
    input: MusicMbidInput,
) -> Result<ApiResponse<MusicReleaseGraph>, ErrorDto> {
    // Fetch trusted provider data after the user selected a concrete edition. The frontend
    // cannot submit invented provider values or provenance as an import graph.
    let graph = state
        .music_providers
        .release(
            &input.mbid,
            &mediaatlas_infrastructure::network::CancellationToken::new(),
        )
        .await
        .map_err(|e| storage_error(RepositoryError::Provider(e)))?;
    state
        .music
        .import_release(graph)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_cover(
    state: State<'_, DesktopState>,
    input: MusicMbidInput,
) -> Result<ApiResponse<Vec<MusicCover>>, ErrorDto> {
    state
        .music_providers
        .cover(
            &input.mbid,
            &mediaatlas_infrastructure::network::CancellationToken::new(),
        )
        .await
        .map(ApiResponse::new)
        .map_err(|e| storage_error(RepositoryError::Provider(e)))
}
#[tauri::command]
pub async fn music_link_track(
    state: State<'_, DesktopState>,
    input: MusicLinkInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    state
        .music
        .link_track(input.track_id, input.file_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_match(
    state: State<'_, DesktopState>,
    input: MusicMatchRequest,
) -> Result<ApiResponse<MusicMatchAssessment>, ErrorDto> {
    let graph = state
        .music
        .album(input.release_id)
        .await
        .map_err(storage_error)?;
    Ok(ApiResponse::new(assess_music_release(&input.input, &graph)))
}
#[tauri::command]
pub async fn music_tag_read(
    state: State<'_, DesktopState>,
    input: MusicFileInput,
) -> Result<ApiResponse<MusicTagRead>, ErrorDto> {
    state
        .music_tags
        .read(input.file_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_tag_history(
    state: State<'_, DesktopState>,
    input: MusicFileInput,
) -> Result<ApiResponse<Vec<MusicTagResult>>, ErrorDto> {
    state
        .music_tags
        .history(input.file_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_tag_preview(
    state: State<'_, DesktopState>,
    input: MusicTagInput,
) -> Result<ApiResponse<MusicTagPreview>, ErrorDto> {
    state
        .music_tags
        .preview(input.file_id, input.changes)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_tag_execute(
    state: State<'_, DesktopState>,
    input: MusicPreviewInput,
) -> Result<ApiResponse<MusicTagResult>, ErrorDto> {
    state
        .music_tags
        .execute(input.preview_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_restore_preview(
    state: State<'_, DesktopState>,
    input: MusicOperationInput,
) -> Result<ApiResponse<MusicTagPreview>, ErrorDto> {
    state
        .music_tags
        .restore_preview(input.operation_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_restore_execute(
    state: State<'_, DesktopState>,
    input: MusicPreviewInput,
) -> Result<ApiResponse<MusicTagResult>, ErrorDto> {
    state
        .music_tags
        .restore_execute(input.preview_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_tag_discard(
    state: State<'_, DesktopState>,
    input: MusicPreviewInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    state
        .music_tags
        .discard(input.preview_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn music_cover_preview(
    state: State<'_, DesktopState>,
    input: MusicCoverInput,
) -> Result<ApiResponse<mediaatlas_infrastructure::music_artwork::MusicCoverPreview>, ErrorDto> {
    state
        .music_artwork
        .select(
            input.release_id,
            &input.mbid,
            input.cover_index,
            &mediaatlas_infrastructure::network::CancellationToken::new(),
        )
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn music_cached_cover(
    state: State<'_, DesktopState>,
    input: MusicReleaseInput,
) -> Result<
    ApiResponse<Option<mediaatlas_infrastructure::music_artwork::MusicCoverPreview>>,
    ErrorDto,
> {
    state
        .music_artwork
        .cached(input.release_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn music_open_file(
    state: State<'_, DesktopState>,
    input: MusicFileInput,
) -> Result<ApiResponse<mediaatlas_infrastructure::music_open::MusicOpenResult>, ErrorDto> {
    state
        .music_open
        .open(input.file_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
