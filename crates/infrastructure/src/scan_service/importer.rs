//! Confirmation validation and transactional import of the exact selected candidates.

use chrono::{DateTime, Utc};
use mediaatlas_domain::{normalize_relative_path, LibraryRoot, MediaKind};
use mediaatlas_media_scanner::{sidecar_fingerprint, ScannedFile};
use serde_json::Value;
use sqlx::{Row, Sqlite, Transaction};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

use crate::library::RepositoryError;

pub(super) fn validate_sources(root: &str, files: &[ScannedFile]) -> Result<(), RepositoryError> {
    let root = Path::new(root);
    if fs::symlink_metadata(root)
        .map_err(|_| RepositoryError::Conflict)?
        .file_type()
        .is_symlink()
    {
        return Err(RepositoryError::Conflict);
    }
    let canonical_root = fs::canonicalize(root).map_err(|_| RepositoryError::Conflict)?;
    for file in files {
        let source = safe_source(root, &canonical_root, &file.relative_path)?;
        let metadata = fs::metadata(source).map_err(|_| RepositoryError::Conflict)?;
        let modified = metadata
            .modified()
            .ok()
            .map(DateTime::<Utc>::from)
            .and_then(|value| DateTime::from_timestamp_millis(value.timestamp_millis()));
        if metadata.len() != file.size_bytes || modified != file.modified_at {
            return Err(RepositoryError::Conflict);
        }
        for associated in &file.associated_files {
            safe_source(root, &canonical_root, associated)?;
        }
        for artwork in &file.artworks {
            safe_source(root, &canonical_root, &artwork.relative_path)?;
        }
        if sidecar_fingerprint(root, file).map_err(|_| RepositoryError::Conflict)?
            != file.sidecar_fingerprint
        {
            return Err(RepositoryError::Conflict);
        }
    }
    Ok(())
}

fn safe_source(
    root: &Path,
    canonical_root: &Path,
    relative: &str,
) -> Result<PathBuf, RepositoryError> {
    let relative = Path::new(relative);
    let mut path = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => path.push(part),
            _ => return Err(RepositoryError::Conflict),
        }
        if fs::symlink_metadata(&path)
            .map_err(|_| RepositoryError::Conflict)?
            .file_type()
            .is_symlink()
        {
            return Err(RepositoryError::Conflict);
        }
    }
    let canonical = fs::canonicalize(&path).map_err(|_| RepositoryError::Conflict)?;
    if !canonical.starts_with(canonical_root) || !canonical.is_file() {
        return Err(RepositoryError::Conflict);
    }
    Ok(path)
}

pub(super) async fn import_file(
    tx: &mut Transaction<'_, Sqlite>,
    root: &LibraryRoot,
    file: &ScannedFile,
) -> Result<Uuid, RepositoryError> {
    let normalized = normalize_relative_path(&file.relative_path, root.path_style)
        .map_err(|_| RepositoryError::InvalidInput)?;
    if normalized != file.normalized_relative_path {
        return Err(RepositoryError::Conflict);
    }
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT media_item_id FROM media_files WHERE root_id=? AND normalized_relative_path=?",
    )
    .bind(root.id.to_string())
    .bind(&normalized)
    .fetch_optional(&mut **tx)
    .await?;
    let group_key = format!(
        "item:{}:{}",
        file.parsed.kind.as_str(),
        file.parsed.group_key
    );
    let group: Option<String> = sqlx::query_scalar(
        "SELECT media_item_id FROM scan_media_groups WHERE root_id=? AND group_key=?",
    )
    .bind(root.id.to_string())
    .bind(&group_key)
    .fetch_optional(&mut **tx)
    .await?;
    if existing.is_some() && group.is_some() && existing != group {
        return Err(RepositoryError::Conflict);
    }
    let title = file
        .local_metadata
        .fields
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&file.parsed.title);
    if title.trim().is_empty() {
        return Err(RepositoryError::InvalidInput);
    }
    let year = file
        .local_metadata
        .fields
        .get("year")
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .and_then(|value| i32::try_from(value).ok())
        .or(file.parsed.year);
    let id = if let Some(id) = existing.or(group) {
        Uuid::parse_str(&id).map_err(|_| RepositoryError::CorruptData)?
    } else {
        Uuid::new_v4()
    };
    let now = Utc::now().timestamp_millis();
    let established: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_metadata_provenance WHERE media_item_id=?) OR EXISTS(SELECT 1 FROM video_metadata_sources WHERE media_item_id=?) OR EXISTS(SELECT 1 FROM adult_metadata_sources WHERE media_item_id=?)").bind(id.to_string()).bind(id.to_string()).bind(id.to_string()).fetch_one(&mut **tx).await?;
    let preserve_title = established && !file.local_metadata.fields.contains_key("title");
    let preserve_year = established && !file.local_metadata.fields.contains_key("year");
    sqlx::query("INSERT INTO media_items(id,kind,title,original_title,year,created_at,updated_at) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO NOTHING")
        .bind(id.to_string()).bind(file.parsed.kind.as_str()).bind(title).bind(file.local_metadata.fields.get("original_title").and_then(Value::as_str)).bind(year).bind(now).bind(now).execute(&mut **tx).await?;
    // A bare rescan updates file associations without replacing previously
    // confirmed provider/NFO metadata with a filename guess.
    if !preserve_title {
        // Existing manual/locked title and year fields retain their values.
        sqlx::query("UPDATE media_items SET title=?,updated_at=? WHERE id=? AND NOT EXISTS(SELECT 1 FROM metadata_values WHERE media_item_id=? AND field_key='title' AND (is_locked=1 OR is_user_modified=1))")
        .bind(title).bind(now).bind(id.to_string()).bind(id.to_string()).execute(&mut **tx).await?;
    }
    if !preserve_year && year.is_some() {
        sqlx::query("UPDATE media_items SET year=? WHERE id=? AND NOT EXISTS(SELECT 1 FROM metadata_values WHERE media_item_id=? AND field_key='year' AND (is_locked=1 OR is_user_modified=1))")
        .bind(year).bind(id.to_string()).bind(id.to_string()).execute(&mut **tx).await?;
    }
    sqlx::query("INSERT INTO scan_media_groups(root_id,group_key,media_item_id) VALUES(?,?,?) ON CONFLICT(root_id,group_key) DO NOTHING")
        .bind(root.id.to_string()).bind(group_key).bind(id.to_string()).execute(&mut **tx).await?;
    save_file(
        tx,
        root,
        id,
        FileIndexInput {
            relative: &file.relative_path,
            size: file.size_bytes,
            modified: file.modified_at,
            part: file.parsed.part,
            shared_companion: false,
        },
    )
    .await?;
    for associated in &file.associated_files {
        let path = Path::new(&root.path).join(associated);
        let info = tokio::task::spawn_blocking(move || {
            fs::metadata(path).map(|metadata| {
                let modified = metadata
                    .modified()
                    .ok()
                    .map(DateTime::<Utc>::from)
                    .and_then(|value| DateTime::from_timestamp_millis(value.timestamp_millis()));
                (metadata.len(), modified)
            })
        })
        .await
        .map_err(|_| RepositoryError::Database)?
        .map_err(|_| RepositoryError::Conflict)?;
        save_file(
            tx,
            root,
            id,
            FileIndexInput {
                relative: associated,
                size: info.0,
                modified: info.1,
                part: None,
                shared_companion: true,
            },
        )
        .await?;
    }
    let source = format!("library://{}/{}", root.id, file.relative_path);
    let mut fields = file.local_metadata.fields.clone();
    if !preserve_title {
        fields
            .entry("title".into())
            .or_insert_with(|| Value::String(title.to_owned()));
    }
    if !preserve_year {
        if let Some(year) = year {
            fields.entry("year".into()).or_insert(Value::from(year));
        }
    }
    if let Some(value) = &file.parsed.series_title {
        fields
            .entry("series_title".into())
            .or_insert(Value::String(value.clone()));
    }
    for (key, value) in [
        ("season_number", file.parsed.season_number),
        ("episode_number", file.parsed.episode_number),
        ("absolute_number", file.parsed.absolute_number),
    ] {
        if let Some(value) = value {
            fields.entry(key.into()).or_insert(Value::from(value));
        }
    }
    if let Some(value) = &file.parsed.catalog_number {
        fields
            .entry("catalog_number".into())
            .or_insert(Value::String(value.clone()));
    }
    for (key, value) in fields {
        if key.trim() != key || key.is_empty() || key.contains('\0') {
            continue;
        }
        sqlx::query("INSERT INTO metadata_values(id,media_item_id,field_key,locale,value_json,provider_id,source_url,fetched_at,confidence,is_user_modified,is_locked) VALUES(?,?,?,'',?,'local',?,?,NULL,0,0) ON CONFLICT(media_item_id,field_key,locale) DO UPDATE SET value_json=excluded.value_json,provider_id=excluded.provider_id,source_url=excluded.source_url,fetched_at=excluded.fetched_at WHERE metadata_values.is_locked=0 AND metadata_values.is_user_modified=0")
            .bind(Uuid::new_v4().to_string()).bind(id.to_string()).bind(key).bind(value.to_string()).bind(&source).bind(now).execute(&mut **tx).await?;
    }
    for external in &file.local_metadata.external_ids {
        if external.provider_id.trim().is_empty()
            || external.namespace.trim().is_empty()
            || external.value.trim().is_empty()
        {
            continue;
        }
        sqlx::query("INSERT OR IGNORE INTO external_ids(id,media_item_id,provider_id,namespace,external_id,is_primary,created_at) VALUES(?,?,?,?,?,0,?)")
            .bind(Uuid::new_v4().to_string()).bind(id.to_string()).bind(&external.provider_id).bind(&external.namespace).bind(&external.value).bind(now).execute(&mut **tx).await?;
    }
    for artwork in &file.artworks {
        normalize_relative_path(&artwork.relative_path, root.path_style)
            .map_err(|_| RepositoryError::InvalidInput)?;
        let uri = format!("library://{}/{}", root.id, artwork.relative_path);
        sqlx::query("INSERT INTO artworks(id,media_item_id,kind,cache_path,source_url,provider_id,width,height,content_hash,created_at) SELECT ?,?,?,NULL,?,'local',NULL,NULL,NULL,? WHERE NOT EXISTS(SELECT 1 FROM artworks WHERE media_item_id=? AND kind=? AND source_url=?)")
            .bind(Uuid::new_v4().to_string()).bind(id.to_string()).bind(artwork.kind.as_str()).bind(&uri).bind(now).bind(id.to_string()).bind(artwork.kind.as_str()).bind(&uri).execute(&mut **tx).await?;
    }
    // This map contains only parsed local NFO/audio tags. Inferred filename
    // fields are added to a separate copy above and do not authorize readiness.
    if file
        .local_metadata
        .fields
        .get("title")
        .and_then(Value::as_str)
        .is_some_and(|t| !t.trim().is_empty())
    {
        let source_kind = if file
            .associated_files
            .iter()
            .any(|p| p.to_ascii_lowercase().ends_with(".nfo"))
        {
            "nfo"
        } else {
            "audio_tags"
        };
        crate::library_ready::record_local(tx, id, source_kind, &source).await?;
    }
    prune_stale_companion_indexes(tx, root, id).await?;
    add_hierarchy(tx, root, file, id, now).await?;
    Ok(id)
}

/// Reconcile only missing local companion indexes after explicit confirmation.
/// Source files and user-selected/remote artwork are never deleted or changed.
async fn prune_stale_companion_indexes(
    tx: &mut Transaction<'_, Sqlite>,
    root: &LibraryRoot,
    media_id: Uuid,
) -> Result<(), RepositoryError> {
    let rows =
        sqlx::query("SELECT id,relative_path FROM media_files WHERE root_id=? AND media_item_id=?")
            .bind(root.id.to_string())
            .bind(media_id.to_string())
            .fetch_all(&mut **tx)
            .await?;
    let mut files = Vec::new();
    for row in rows {
        let relative: String = row
            .try_get("relative_path")
            .map_err(|_| RepositoryError::CorruptData)?;
        let extension = Path::new(&relative)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(
            extension.as_str(),
            "nfo" | "srt" | "ass" | "ssa" | "sub" | "idx" | "vtt" | "jpg" | "jpeg" | "png" | "webp"
        ) {
            files.push((
                row.try_get::<String, _>("id")
                    .map_err(|_| RepositoryError::CorruptData)?,
                relative,
            ));
        }
    }
    let rows = sqlx::query(
        "SELECT id,source_url FROM artworks WHERE media_item_id=? AND provider_id='local'",
    )
    .bind(media_id.to_string())
    .fetch_all(&mut **tx)
    .await?;
    let prefix = format!("library://{}/", root.id);
    let mut artworks = Vec::new();
    for row in rows {
        let source: Option<String> = row
            .try_get("source_url")
            .map_err(|_| RepositoryError::CorruptData)?;
        if let Some(relative) = source
            .as_deref()
            .and_then(|value| value.strip_prefix(&prefix))
        {
            artworks.push((
                row.try_get::<String, _>("id")
                    .map_err(|_| RepositoryError::CorruptData)?,
                relative.to_owned(),
            ));
        }
    }
    let root_path = root.path.clone();
    let (stale_files, stale_artworks) = tokio::task::spawn_blocking(move || {
        let root = Path::new(&root_path);
        let canonical_root = root.canonicalize().map_err(|_| RepositoryError::Conflict)?;
        let mut stale_files = Vec::new();
        let mut stale_artworks = Vec::new();
        for (id, relative) in files {
            if stale_companion(root, &canonical_root, &relative)? {
                stale_files.push(id);
            }
        }
        for (id, relative) in artworks {
            if stale_companion(root, &canonical_root, &relative)? {
                stale_artworks.push(id);
            }
        }
        Ok::<_, RepositoryError>((stale_files, stale_artworks))
    })
    .await
    .map_err(|_| RepositoryError::Database)??;
    for id in stale_files {
        sqlx::query("DELETE FROM media_files WHERE id=? AND root_id=? AND media_item_id=?")
            .bind(id)
            .bind(root.id.to_string())
            .bind(media_id.to_string())
            .execute(&mut **tx)
            .await?;
    }
    for id in stale_artworks {
        sqlx::query("DELETE FROM artworks WHERE id=? AND media_item_id=? AND provider_id='local'")
            .bind(id)
            .bind(media_id.to_string())
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

fn stale_companion(
    root: &Path,
    canonical_root: &Path,
    relative: &str,
) -> Result<bool, RepositoryError> {
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(part) = component else {
            return Ok(true);
        };
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                return Ok(true)
            }
            Err(_) => return Err(RepositoryError::Conflict),
        }
    }
    let canonical = path.canonicalize().map_err(|_| RepositoryError::Conflict)?;
    Ok(!canonical.starts_with(canonical_root) || !canonical.is_file())
}

struct FileIndexInput<'a> {
    relative: &'a str,
    size: u64,
    modified: Option<DateTime<Utc>>,
    part: Option<u32>,
    shared_companion: bool,
}

async fn save_file(
    tx: &mut Transaction<'_, Sqlite>,
    root: &LibraryRoot,
    media_id: Uuid,
    input: FileIndexInput<'_>,
) -> Result<(), RepositoryError> {
    let FileIndexInput {
        relative,
        size,
        modified,
        part,
        shared_companion,
    } = input;
    let normalized = normalize_relative_path(relative, root.path_style)
        .map_err(|_| RepositoryError::InvalidInput)?;
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT media_item_id FROM media_files WHERE root_id=? AND normalized_relative_path=?",
    )
    .bind(root.id.to_string())
    .bind(&normalized)
    .fetch_optional(&mut **tx)
    .await?;
    // Shared directory companions belong to their first indexed owner; artwork/source references still associate them with every work.
    if existing
        .as_deref()
        .is_some_and(|owner| owner != media_id.to_string())
    {
        if shared_companion {
            return Ok(());
        }
        return Err(RepositoryError::Conflict);
    }
    sqlx::query("INSERT INTO media_files(id,media_item_id,root_id,relative_path,normalized_relative_path,size_bytes,modified_at,part) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(root_id,normalized_relative_path) DO UPDATE SET size_bytes=excluded.size_bytes,modified_at=excluded.modified_at,part=COALESCE(excluded.part,media_files.part)")
        .bind(Uuid::new_v4().to_string()).bind(media_id.to_string()).bind(root.id.to_string()).bind(relative).bind(normalized)
        .bind(i64::try_from(size).map_err(|_|RepositoryError::InvalidInput)?).bind(modified.map(|value|value.timestamp_millis())).bind(part.map(i64::from)).execute(&mut **tx).await?;
    Ok(())
}

async fn add_hierarchy(
    tx: &mut Transaction<'_, Sqlite>,
    root: &LibraryRoot,
    file: &ScannedFile,
    child_id: Uuid,
    now: i64,
) -> Result<(), RepositoryError> {
    let Some(series_title) = &file.parsed.series_title else {
        return Ok(());
    };
    if !matches!(file.parsed.kind, MediaKind::TvEpisode | MediaKind::Anime) {
        return Ok(());
    }
    let parent = Path::new(&file.normalized_relative_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let last = parent
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let context = if last.starts_with("season") || last.starts_with("s0") {
        parent.parent().unwrap_or_else(|| Path::new(""))
    } else {
        parent
    };
    let context = context.to_str().ok_or(RepositoryError::InvalidInput)?;
    let series_key = format!("series:{context}:{series_title}");
    let series_id = ensure_group(
        tx,
        root.id,
        &series_key,
        MediaKind::TvSeries,
        series_title,
        now,
    )
    .await?;
    let parent_id = if let Some(season) = file.parsed.season_number {
        let season_id = ensure_group(
            tx,
            root.id,
            &format!("{series_key}:season:{season}"),
            MediaKind::TvSeason,
            &format!("{series_title} — {season}"),
            now,
        )
        .await?;
        sqlx::query("INSERT OR IGNORE INTO media_relations(parent_id,child_id,relation_type) VALUES(?,?,'season')").bind(series_id.to_string()).bind(season_id.to_string()).execute(&mut **tx).await?;
        season_id
    } else {
        series_id
    };
    sqlx::query("INSERT OR IGNORE INTO media_relations(parent_id,child_id,relation_type) VALUES(?,?,'episode')").bind(parent_id.to_string()).bind(child_id.to_string()).execute(&mut **tx).await?;
    Ok(())
}

async fn ensure_group(
    tx: &mut Transaction<'_, Sqlite>,
    root_id: Uuid,
    key: &str,
    kind: MediaKind,
    title: &str,
    now: i64,
) -> Result<Uuid, RepositoryError> {
    if let Some(id) = sqlx::query_scalar::<_, String>(
        "SELECT media_item_id FROM scan_media_groups WHERE root_id=? AND group_key=?",
    )
    .bind(root_id.to_string())
    .bind(key)
    .fetch_optional(&mut **tx)
    .await?
    {
        return Uuid::parse_str(&id).map_err(|_| RepositoryError::CorruptData);
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO media_items(id,kind,title,created_at,updated_at) VALUES(?,?,?,?,?)")
        .bind(id.to_string())
        .bind(kind.as_str())
        .bind(title)
        .bind(now)
        .bind(now)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO scan_media_groups(root_id,group_key,media_item_id) VALUES(?,?,?)")
        .bind(root_id.to_string())
        .bind(key)
        .bind(id.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(id)
}
