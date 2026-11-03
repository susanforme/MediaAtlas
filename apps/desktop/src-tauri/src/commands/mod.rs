mod adult;
mod music;
pub use adult::*;
pub use music::*;

use crate::state::DesktopState;
use mediaatlas_application::library::*;
use mediaatlas_desktop_bridge::{AppInfoDto, ErrorDto};
use mediaatlas_infrastructure::library::RepositoryError;
use tauri::State;
use uuid::Uuid;

pub(super) fn storage_error(error: RepositoryError) -> ErrorDto {
    let code = match error {
        RepositoryError::InvalidInput => "invalid_input",
        RepositoryError::NotFound => "not_found",
        RepositoryError::Conflict => "conflict",
        RepositoryError::ProtectedMetadata => "protected_metadata",
        RepositoryError::Database => "database_error",
        RepositoryError::Migration => "migration_error",
        RepositoryError::CorruptData => "corrupt_data",
        RepositoryError::Provider(failure) => failure.code(),
    };
    ErrorDto {
        schema_version: 1,
        code,
        message: "The local library request could not be completed.",
        correlation_id: Uuid::new_v4().to_string(),
    }
}

#[tauri::command]
pub fn get_app_info() -> Result<AppInfoDto, ErrorDto> {
    mediaatlas_application::get_app_info(env!("CARGO_PKG_VERSION"))
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn library_list_roots(
    state: State<'_, DesktopState>,
    input: RootListInput,
) -> Result<ApiResponse<Vec<RootView>>, ErrorDto> {
    if input.include_adult {
        state.adult_privacy.access().await.map_err(storage_error)?;
    }
    state
        .browse
        .list_roots(input.include_adult)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_save_root(
    state: State<'_, DesktopState>,
    input: RootInput,
) -> Result<ApiResponse<RootView>, ErrorDto> {
    if input.kind == Some(mediaatlas_domain::MediaKind::AdultVideo) {
        state.adult_privacy.access().await.map_err(storage_error)?;
    }
    if let Some(id) = input.id {
        state
            .adult_privacy
            .authorize_root(id)
            .await
            .map_err(storage_error)?;
    }
    state
        .browse
        .save_root(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_remove_root(
    state: State<'_, DesktopState>,
    input: RemoveRootInput,
) -> Result<ApiResponse<bool>, ErrorDto> {
    state
        .adult_privacy
        .authorize_root(input.root_id)
        .await
        .map_err(storage_error)?;
    state
        .browse
        .remove_root(input.root_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_list_media(
    state: State<'_, DesktopState>,
    input: BrowseQuery,
) -> Result<ApiResponse<MediaPageView>, ErrorDto> {
    if input.include_adult {
        return Err(storage_error(RepositoryError::ProtectedMetadata));
    }
    state
        .browse
        .list_media(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_get_media(
    state: State<'_, DesktopState>,
    input: MediaDetailInput,
) -> Result<ApiResponse<MediaDetailView>, ErrorDto> {
    if input.include_adult {
        return Err(storage_error(RepositoryError::ProtectedMetadata));
    }
    state
        .browse
        .get_media(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_summary(
    state: State<'_, DesktopState>,
    input: BrowseQuery,
) -> Result<ApiResponse<LibrarySummaryView>, ErrorDto> {
    if input.include_adult {
        return Err(storage_error(RepositoryError::ProtectedMetadata));
    }
    state
        .browse
        .summary(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_facets(
    state: State<'_, DesktopState>,
    input: BrowseQuery,
) -> Result<ApiResponse<LibraryFacetsView>, ErrorDto> {
    if input.include_adult {
        return Err(storage_error(RepositoryError::ProtectedMetadata));
    }
    state
        .browse
        .facets(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scan_start(
    state: State<'_, DesktopState>,
    input: StartInput,
) -> Result<ApiResponse<ScanTaskView>, ErrorDto> {
    state
        .adult_privacy
        .authorize_root(input.root_id)
        .await
        .map_err(storage_error)?;
    state
        .scans
        .start(input.root_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scan_list_tasks(
    state: State<'_, DesktopState>,
    input: ListTasksInput,
) -> Result<ApiResponse<Vec<ScanTaskView>>, ErrorDto> {
    if input.include_adult {
        state.adult_privacy.access().await.map_err(storage_error)?;
    }
    state
        .scans
        .list_tasks(input.include_adult)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scan_get_preview(
    state: State<'_, DesktopState>,
    input: PreviewInput,
) -> Result<ApiResponse<PreviewPageView>, ErrorDto> {
    if input.include_adult {
        state.adult_privacy.access().await.map_err(storage_error)?;
    }
    state
        .scans
        .preview(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scan_confirm(
    state: State<'_, DesktopState>,
    input: ConfirmInput,
) -> Result<ApiResponse<ConfirmResult>, ErrorDto> {
    let _confirmation = state.adult_privacy.confirmation_guard().await;
    state
        .adult_privacy
        .authorize_task(input.task_id)
        .await
        .map_err(storage_error)?;
    state
        .scans
        .confirm(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scan_cancel(
    state: State<'_, DesktopState>,
    input: CancelInput,
) -> Result<ApiResponse<ScanTaskView>, ErrorDto> {
    state
        .adult_privacy
        .authorize_task(input.task_id)
        .await
        .map_err(storage_error)?;
    state
        .scans
        .cancel(input.task_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEnabledInput {
    provider_id: String,
    enabled: bool,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderStartInput {
    provider_id: String,
    query: String,
    language: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCancelInput {
    task_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfigInput {
    provider_id: String,
    config: mediaatlas_domain::ProviderConfig,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSnapshotInput {}

#[tauri::command]
pub async fn provider_snapshot(
    state: State<'_, DesktopState>,
    input: ProviderSnapshotInput,
) -> Result<ApiResponse<mediaatlas_domain::ProviderSnapshot>, ErrorDto> {
    let _ = input;
    state
        .providers
        .snapshot()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn provider_set_enabled(
    state: State<'_, DesktopState>,
    input: ProviderEnabledInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    if state.providers.is_adult_provider(&input.provider_id).await {
        state.adult_privacy.access().await.map_err(storage_error)?;
    }
    state
        .providers
        .set_enabled(&input.provider_id, input.enabled)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn provider_update_config(
    state: State<'_, DesktopState>,
    input: ProviderConfigInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    if state.providers.is_adult_provider(&input.provider_id).await {
        state.adult_privacy.access().await.map_err(storage_error)?;
    }
    state
        .providers
        .update_config(&input.provider_id, &input.config)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn provider_start(
    state: State<'_, DesktopState>,
    input: ProviderStartInput,
) -> Result<ApiResponse<Uuid>, ErrorDto> {
    state
        .providers
        .start_localized(&input.provider_id, &input.query, &input.language)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn provider_cancel(
    state: State<'_, DesktopState>,
    input: ProviderCancelInput,
) -> Result<ApiResponse<bool>, ErrorDto> {
    state
        .adult_privacy
        .authorize_task(input.task_id)
        .await
        .map_err(storage_error)?;
    state
        .providers
        .cancel(input.task_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScraperDetailInput {
    run_id: Uuid,
    candidate_index: usize,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScraperPreviewIdInput {
    preview_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScraperMediaInput {
    media_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScraperArtworkInput {
    run_id: Uuid,
    candidate_index: usize,
    artwork_index: usize,
}
#[tauri::command]
pub async fn scraper_search(
    state: State<'_, DesktopState>,
    input: mediaatlas_domain::ScraperSearchRequest,
) -> Result<ApiResponse<mediaatlas_domain::SearchPreview>, ErrorDto> {
    state
        .scraper
        .search(input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn scraper_detail(
    state: State<'_, DesktopState>,
    input: ScraperDetailInput,
) -> Result<ApiResponse<mediaatlas_domain::VideoMetadata>, ErrorDto> {
    state
        .scraper
        .detail(input.run_id, input.candidate_index)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn scraper_preview(
    state: State<'_, DesktopState>,
    input: mediaatlas_domain::MergeRequest,
) -> Result<ApiResponse<mediaatlas_domain::MergePreview>, ErrorDto> {
    state
        .scraper
        .preview(input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn scraper_confirm(
    state: State<'_, DesktopState>,
    input: ScraperPreviewIdInput,
) -> Result<ApiResponse<mediaatlas_domain::MergeResult>, ErrorDto> {
    state
        .scraper
        .confirm(input.preview_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn scraper_history(
    state: State<'_, DesktopState>,
    input: ProviderSnapshotInput,
) -> Result<ApiResponse<Vec<mediaatlas_infrastructure::scraper::ScraperHistoryEntry>>, ErrorDto> {
    let _ = input;
    state
        .scraper
        .history()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn scraper_output_preview(
    state: State<'_, DesktopState>,
    input: ScraperMediaInput,
) -> Result<ApiResponse<mediaatlas_infrastructure::media_output::OutputPreview>, ErrorDto> {
    state
        .output
        .preview(input.media_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn scraper_output_execute(
    state: State<'_, DesktopState>,
    input: ScraperPreviewIdInput,
) -> Result<ApiResponse<mediaatlas_infrastructure::media_output::OutputResult>, ErrorDto> {
    state
        .output
        .execute(input.preview_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn scraper_artwork_preview(
    state: State<'_, DesktopState>,
    input: ScraperArtworkInput,
) -> Result<ApiResponse<mediaatlas_infrastructure::scraper::ArtworkPreview>, ErrorDto> {
    state
        .scraper
        .artwork_preview(input.run_id, input.candidate_index, input.artwork_index)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScraperRunInput {
    run_id: Uuid,
}
#[tauri::command]
pub async fn scraper_search_preview(
    state: State<'_, DesktopState>,
    input: ScraperRunInput,
) -> Result<ApiResponse<mediaatlas_domain::SearchPreview>, ErrorDto> {
    state
        .scraper
        .search_preview(input.run_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn scraper_media_detail(
    state: State<'_, DesktopState>,
    input: ScraperMediaInput,
) -> Result<ApiResponse<Option<mediaatlas_domain::VideoMetadata>>, ErrorDto> {
    state
        .scraper
        .media_detail(input.media_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scraper_output_discard(
    state: State<'_, DesktopState>,
    input: ScraperPreviewIdInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    state
        .output
        .discard(input.preview_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderAuthInput {
    provider_id: String,
}
#[tauri::command]
pub async fn provider_auth_status(
    state: State<'_, DesktopState>,
    input: ProviderAuthInput,
) -> Result<ApiResponse<mediaatlas_infrastructure::provider_auth::AuthStatus>, ErrorDto> {
    if state.providers.is_adult_provider(&input.provider_id).await {
        state.adult_privacy.access().await.map_err(storage_error)?;
    }
    state
        .auth
        .status(&input.provider_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn provider_auth_save(
    state: State<'_, DesktopState>,
    input: mediaatlas_infrastructure::provider_auth::AuthSaveInput,
) -> Result<ApiResponse<mediaatlas_infrastructure::provider_auth::AuthStatus>, ErrorDto> {
    if state.providers.is_adult_provider(&input.provider_id).await {
        state.adult_privacy.access().await.map_err(storage_error)?;
    }
    state
        .auth
        .save(input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn provider_auth_clear(
    state: State<'_, DesktopState>,
    input: ProviderAuthInput,
) -> Result<ApiResponse<mediaatlas_infrastructure::provider_auth::AuthStatus>, ErrorDto> {
    if state.providers.is_adult_provider(&input.provider_id).await {
        state.adult_privacy.access().await.map_err(storage_error)?;
    }
    state
        .auth
        .clear(&input.provider_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
