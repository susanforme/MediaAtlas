use crate::library::RepositoryError;
use mediaatlas_domain::{MediaKind, NfoExportMode};
use sqlx::SqlitePool;
use std::collections::HashSet;
use uuid::Uuid;

pub(crate) fn default_providers(kind: MediaKind) -> Vec<String> {
    match kind {
        MediaKind::Movie | MediaKind::TvSeries => vec!["tmdb".into()],
        MediaKind::Anime => vec!["anilist".into()],
        MediaKind::AdultVideo => vec!["dmm".into(), "wikidata".into()],
        _ => vec![],
    }
}

pub(crate) fn validate_providers(
    kind: Option<MediaKind>,
    providers: &[String],
) -> Result<(), RepositoryError> {
    let mut seen = HashSet::new();
    for provider in providers {
        let supported = match provider.as_str() {
            "tmdb" | "omdb" | "douban" => kind.is_none() || matches!(kind, Some(MediaKind::Movie | MediaKind::TvSeries)),
            "anilist" | "bangumi" => kind.is_none() || kind == Some(MediaKind::Anime),
            "tvmaze" => kind.is_none() || kind == Some(MediaKind::TvSeries),
            "dmm" | "wikidata" => kind == Some(MediaKind::AdultVideo),
            _ => false,
        };
        if !supported || !seen.insert(provider) || providers.len() > 8 {
            return Err(RepositoryError::InvalidInput);
        }
    }
    Ok(())
}

pub(crate) async fn load(
    pool: &SqlitePool,
    root_id: Uuid,
) -> Result<Option<Vec<String>>, RepositoryError> {
    let value = sqlx::query_scalar::<_, Option<String>>(
        "SELECT metadata_providers_json FROM root_settings WHERE root_id=?",
    )
    .bind(root_id.to_string())
    .fetch_optional(pool)
    .await?
    .flatten();
    value
        .map(|value| serde_json::from_str(&value).map_err(|_| RepositoryError::CorruptData))
        .transpose()
}

pub(crate) fn parse_nfo_export_mode(value: &str) -> Result<NfoExportMode, RepositoryError> {
    match value {
        "online" => Ok(NfoExportMode::Online),
        "offline" => Ok(NfoExportMode::Offline),
        _ => Err(RepositoryError::CorruptData),
    }
}

pub(crate) async fn load_nfo_export_mode(
    pool: &SqlitePool,
    root_id: Uuid,
) -> Result<NfoExportMode, RepositoryError> {
    let value: Option<String> = sqlx::query_scalar("SELECT nfo_export_mode FROM root_settings WHERE root_id=?")
        .bind(root_id.to_string()).fetch_optional(pool).await?;
    value.as_deref().map(parse_nfo_export_mode).transpose().map(Option::unwrap_or_default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfo_mode_uses_only_the_stored_enum_values() {
        assert_eq!(parse_nfo_export_mode("online"), Ok(NfoExportMode::Online));
        assert_eq!(parse_nfo_export_mode("offline"), Ok(NfoExportMode::Offline));
        assert_eq!(parse_nfo_export_mode("automatic"), Err(RepositoryError::CorruptData));
    }

    #[test]
    fn directory_sources_reject_duplicates_unknown_and_cross_scope_providers() {
        let names = |values: &[&str]| values.iter().map(|value| (*value).into()).collect::<Vec<_>>();
        assert!(validate_providers(Some(MediaKind::TvSeries), &names(&["tvmaze", "tmdb"])).is_ok());
        assert!(validate_providers(Some(MediaKind::Movie), &names(&["douban", "omdb", "tmdb"])).is_ok());
        assert!(validate_providers(Some(MediaKind::Anime), &names(&["bangumi", "anilist"])).is_ok());
        assert!(validate_providers(None, &names(&["tmdb", "omdb", "douban", "anilist", "bangumi", "tvmaze"])).is_ok());
        assert!(validate_providers(Some(MediaKind::Movie), &[]).is_ok());
        assert!(validate_providers(Some(MediaKind::AdultVideo), &names(&["wikidata", "dmm"])).is_ok());
        for (kind, providers) in [
            (Some(MediaKind::Movie), names(&["tmdb", "tmdb"])),
            (Some(MediaKind::Movie), names(&["tvmaze"])),
            (None, names(&["dmm"])),
            (Some(MediaKind::AdultVideo), names(&["tmdb"])),
            (None, names(&["unknown"])),
        ] {
            assert_eq!(validate_providers(kind, &providers), Err(RepositoryError::InvalidInput));
        }
    }
}
