//! Dedicated adult commands: the process-local grant is checked on every entry.
use super::{storage_error, DesktopState};
use mediaatlas_application::library::*;
use mediaatlas_desktop_bridge::ErrorDto;
use mediaatlas_domain::*;
use mediaatlas_infrastructure::{
    adult_output::{AdultBatchPreview, AdultBatchResult},
    adult_privacy::{AdultSettings, AdultSettingsInput},
    media_output::{OutputPreview, OutputResult},
};
use tauri::State;
use uuid::Uuid;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyInput {}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailInput {
    run_id: Uuid,
    candidate_index: usize,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewIdInput {
    preview_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunInput {
    run_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaInput {
    media_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtworkInput {
    run_id: Uuid,
    candidate_index: usize,
    artwork_index: usize,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchPreviewInput {
    media_ids: Vec<Uuid>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchIdInput {
    batch_id: Uuid,
}

#[tauri::command]
pub async fn adult_settings(
    state: State<'_, DesktopState>,
    input: EmptyInput,
) -> Result<ApiResponse<AdultSettings>, ErrorDto> {
    let _ = input;
    state
        .adult_privacy
        .settings()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn adult_save_settings(
    state: State<'_, DesktopState>,
    input: AdultSettingsInput,
) -> Result<ApiResponse<AdultSettings>, ErrorDto> {
    state
        .adult_privacy
        .save(input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn adult_unlock(
    state: State<'_, DesktopState>,
    input: EmptyInput,
) -> Result<ApiResponse<AdultSettings>, ErrorDto> {
    let _ = input;
    state
        .adult_privacy
        .unlock()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn adult_lock(
    state: State<'_, DesktopState>,
    input: EmptyInput,
) -> Result<ApiResponse<AdultSettings>, ErrorDto> {
    let _ = input;
    state
        .adult_privacy
        .lock()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn adult_search(
    state: State<'_, DesktopState>,
    input: AdultSearchRequest,
) -> Result<ApiResponse<AdultSearchPreview>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let result = state
        .adult_scraper
        .clone()
        .with_access(access.clone())
        .search(input)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_search_preview(
    state: State<'_, DesktopState>,
    input: RunInput,
) -> Result<ApiResponse<AdultSearchPreview>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let result = state
        .adult_scraper
        .clone()
        .with_access(access.clone())
        .search_preview(input.run_id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_detail(
    state: State<'_, DesktopState>,
    input: DetailInput,
) -> Result<ApiResponse<AdultDetail>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let result = state
        .adult_scraper
        .clone()
        .with_access(access.clone())
        .detail(input.run_id, input.candidate_index)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_preview(
    state: State<'_, DesktopState>,
    input: AdultMergeRequest,
) -> Result<ApiResponse<AdultMergePreview>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let result = state
        .adult_scraper
        .clone()
        .with_access(access.clone())
        .preview(input)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_confirm(
    state: State<'_, DesktopState>,
    input: PreviewIdInput,
) -> Result<ApiResponse<MergeResult>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let result = state
        .adult_scraper
        .clone()
        .with_access(access.clone())
        .confirm(input.preview_id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_media_detail(
    state: State<'_, DesktopState>,
    input: MediaInput,
) -> Result<ApiResponse<Option<AdultDetail>>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let result = state
        .adult_scraper
        .clone()
        .with_access(access.clone())
        .media_detail(input.media_id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_artwork_preview(
    state: State<'_, DesktopState>,
    input: ArtworkInput,
) -> Result<ApiResponse<mediaatlas_infrastructure::scraper::ArtworkPreview>, ErrorDto> {
    let access = state
        .adult_privacy
        .require_covers()
        .await
        .map_err(storage_error)?;
    let result = state
        .adult_scraper
        .clone()
        .with_access(access.clone())
        .artwork_preview(input.run_id, input.candidate_index, input.artwork_index)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    state
        .adult_privacy
        .require_covers()
        .await
        .map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_output_preview(
    state: State<'_, DesktopState>,
    input: MediaInput,
) -> Result<ApiResponse<OutputPreview>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    let result = state
        .adult_output()
        .await
        .map_err(storage_error)?
        .preview(input.media_id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_output_execute(
    state: State<'_, DesktopState>,
    input: PreviewIdInput,
) -> Result<ApiResponse<OutputResult>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    let result = state
        .adult_output()
        .await
        .map_err(storage_error)?
        .execute(input.preview_id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_output_discard(
    state: State<'_, DesktopState>,
    input: PreviewIdInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    state
        .adult_output()
        .await
        .map_err(storage_error)?
        .discard(input.preview_id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(()))
}

#[tauri::command]
pub async fn adult_batch_preview(
    state: State<'_, DesktopState>,
    input: BatchPreviewInput,
) -> Result<ApiResponse<AdultBatchPreview>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    let result = state
        .adult_batch()
        .await
        .map_err(storage_error)?
        .preview(input.media_ids)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_batch_execute(
    state: State<'_, DesktopState>,
    input: BatchIdInput,
) -> Result<ApiResponse<AdultBatchResult>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    let result = state
        .adult_batch()
        .await
        .map_err(storage_error)?
        .execute(input.batch_id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_batch_cancel(
    state: State<'_, DesktopState>,
    input: BatchIdInput,
) -> Result<ApiResponse<AdultBatchResult>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    let result = state
        .adult_batch()
        .await
        .map_err(storage_error)?
        .cancel(input.batch_id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_batch_status(
    state: State<'_, DesktopState>,
    input: BatchIdInput,
) -> Result<ApiResponse<AdultBatchResult>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    let result = state
        .adult_batch()
        .await
        .map_err(storage_error)?
        .status(input.batch_id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_providers(
    state: State<'_, DesktopState>,
    input: EmptyInput,
) -> Result<ApiResponse<ProviderSnapshot>, ErrorDto> {
    let _ = input;
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    let result = state
        .providers
        .adult_snapshot()
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_list_media(
    state: State<'_, DesktopState>,
    mut input: BrowseQuery,
) -> Result<ApiResponse<MediaPageView>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    mediaatlas_infrastructure::library_ready::backfill_adult_local_nfo(
        &state.repository,
        access.clone(),
    )
    .await
    .map_err(storage_error)?;
    input.include_adult = true;
    input.kind = Some(MediaKind::AdultVideo);
    let covers = state
        .adult_privacy
        .settings()
        .await
        .map_err(storage_error)?
        .covers_visible;
    let browse = state
        .adult_browse
        .clone()
        .with_adult_access(access.clone(), covers);
    let mut page = browse.list_media(&input).await.map_err(storage_error)?;
    if !state
        .adult_privacy
        .settings()
        .await
        .map_err(storage_error)?
        .covers_visible
    {
        for item in &mut page.items {
            item.poster_uri = None;
        }
    }
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(page))
}
#[tauri::command]
pub async fn adult_get_media(
    state: State<'_, DesktopState>,
    input: MediaInput,
) -> Result<ApiResponse<MediaDetailView>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    mediaatlas_infrastructure::library_ready::backfill_adult_local_nfo(
        &state.repository,
        access.clone(),
    )
    .await
    .map_err(storage_error)?;
    let covers = state
        .adult_privacy
        .settings()
        .await
        .map_err(storage_error)?
        .covers_visible;
    let browse = state
        .adult_browse
        .clone()
        .with_adult_access(access.clone(), covers);
    let mut detail = browse
        .get_media(&MediaDetailInput {
            media_id: input.media_id,
            include_adult: true,
        })
        .await
        .map_err(storage_error)?;
    if detail.media.kind != MediaKind::AdultVideo {
        return Err(storage_error(
            mediaatlas_infrastructure::library::RepositoryError::ProtectedMetadata,
        ));
    }
    if !state
        .adult_privacy
        .settings()
        .await
        .map_err(storage_error)?
        .covers_visible
    {
        detail.poster_uri = None;
    }
    access.ensure().map_err(storage_error)?;
    state
        .adult_privacy
        .record_visit(input.media_id)
        .await
        .map_err(storage_error)?;
    Ok(ApiResponse::new(detail))
}
#[tauri::command]
pub async fn adult_list_roots(
    state: State<'_, DesktopState>,
    input: EmptyInput,
) -> Result<ApiResponse<Vec<RootView>>, ErrorDto> {
    let _ = input;
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let roots = state
        .adult_browse
        .list_roots(true)
        .await
        .map_err(storage_error)?
        .into_iter()
        .filter(|r| r.root.kind == Some(MediaKind::AdultVideo))
        .collect();
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(roots))
}

#[tauri::command]
pub async fn adult_tasks(
    state: State<'_, DesktopState>,
    input: EmptyInput,
) -> Result<ApiResponse<Vec<ScanTaskView>>, ErrorDto> {
    let _ = input;
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let roots = state
        .adult_browse
        .list_roots(true)
        .await
        .map_err(storage_error)?;
    let ids: std::collections::HashSet<_> = roots
        .into_iter()
        .filter(|r| r.root.kind == Some(MediaKind::AdultVideo))
        .map(|r| r.root.id)
        .collect();
    let tasks = state
        .scans
        .list_tasks(true)
        .await
        .map_err(storage_error)?
        .into_iter()
        .filter(|t| ids.contains(&t.root_id))
        .collect();
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(tasks))
}

#[tauri::command]
pub async fn adult_recent(
    state: State<'_, DesktopState>,
    input: EmptyInput,
) -> Result<ApiResponse<Vec<mediaatlas_infrastructure::adult_privacy::AdultRecentVisit>>, ErrorDto>
{
    let _ = input;
    state
        .adult_privacy
        .recent()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
