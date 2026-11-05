//! Resolve native events to a durable source-file scope before starting a scan.
use super::*;
use globset::{Glob, GlobSet, GlobSetBuilder};
use mediaatlas_domain::normalize_relative_path;
use std::{collections::BTreeMap, fs, path::{Component, Path, PathBuf}};

pub(super) fn normalized_selection(snapshot: &RootSnapshot) -> Result<Option<Vec<String>>, RepositoryError> {
    snapshot.selected_sources.as_ref().map(|paths| paths.iter().map(|path| {
        normalize_relative_path(path, snapshot.root.path_style).map_err(|_| RepositoryError::CorruptData)
    }).collect()).transpose()
}

pub(super) fn selected(snapshot: &RootSnapshot, normalized: &str) -> Result<bool, RepositoryError> {
    Ok(normalized_selection(snapshot)?.is_none_or(|paths| paths.iter().any(|path| path == normalized)))
}

fn video(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()).is_some_and(|ext| {
        let ext = ext.to_ascii_lowercase();
        ext == "strm" || crate::library_ready::VIDEO_EXTENSIONS.contains(&ext.as_str())
    })
}

fn private(path: &Path) -> bool {
    path.components().any(|part| part.as_os_str().to_str().is_some_and(|part| {
        let part = part.to_ascii_lowercase();
        part.starts_with(".mediaatlas-") || part.starts_with(".mediaatlas_")
    }))
}

fn excluded(relative: &str, excludes: &GlobSet) -> bool {
    let path = Path::new(relative);
    path.ancestors().filter(|path| !path.as_os_str().is_empty()).any(|path| {
        let path = path.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
        excludes.is_match(&path) || excludes.is_match(format!("{path}/"))
    })
}

pub(super) fn validate_selection(snapshot: &RootSnapshot) -> Result<(), RepositoryError> {
    let Some(paths) = &snapshot.selected_sources else { return Ok(()); };
    let mut builder = GlobSetBuilder::new();
    for pattern in &snapshot.exclude_patterns {
        builder.add(Glob::new(pattern).map_err(|_| RepositoryError::InvalidInput)?);
    }
    let excludes = builder.build().map_err(|_| RepositoryError::InvalidInput)?;
    let mut seen = HashSet::new();
    for path in paths {
        let normalized = normalize_relative_path(path, snapshot.root.path_style).map_err(|_| RepositoryError::InvalidInput)?;
        if !video(Path::new(path)) || private(Path::new(path)) || excluded(path, &excludes) || !seen.insert(normalized) {
            return Err(RepositoryError::InvalidInput);
        }
    }
    Ok(())
}

/// Mirrors the scanner's file-stem and directory-wide companion associations.
pub(super) fn companion_matches(source: &str, companion: &str) -> bool {
    let source = Path::new(source);
    let companion = Path::new(companion);
    if source.parent() != companion.parent() { return false; }
    let Some(stem) = source.file_stem().and_then(|part| part.to_str()) else { return false; };
    let Some(name) = companion.file_name().and_then(|part| part.to_str()) else { return false; };
    let name = name.to_ascii_lowercase();
    let stem = stem.to_ascii_lowercase();
    let same_stem = name.starts_with(&format!("{stem}.")) || name.starts_with(&format!("{stem}-"));
    let ext = companion.extension().and_then(|part| part.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "nfo" => same_stem || matches!(name.as_str(), "movie.nfo" | "tvshow.nfo" | "album.nfo"),
        "srt" | "ass" | "ssa" | "sub" | "idx" | "vtt" => same_stem,
        "jpg" | "jpeg" | "png" | "webp" => same_stem || matches!(name.as_str(),
            "poster.jpg" | "poster.png" | "poster.jpeg" | "folder.jpg" | "cover.jpg" | "cover.png")
            || name.starts_with("fanart.") || name.starts_with("backdrop."),
        _ => false,
    }
}

fn safe_event(root: &Path, relative: &str) -> Result<Option<fs::Metadata>, RepositoryError> {
    let mut absolute = root.to_path_buf();
    let parts = Path::new(relative).components().collect::<Vec<_>>();
    for (index, part) in parts.iter().enumerate() {
        let Component::Normal(part) = part else { return Err(RepositoryError::InvalidInput); };
        absolute.push(part);
        match fs::symlink_metadata(&absolute) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() { return Err(RepositoryError::InvalidInput); }
                if index + 1 == parts.len() { return Ok(Some(metadata)); }
                if !metadata.is_dir() { return Err(RepositoryError::Conflict); }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && index + 1 == parts.len() => return Ok(None),
            Err(_) => return Err(RepositoryError::Conflict),
        }
    }
    Err(RepositoryError::Conflict)
}

fn resolve(snapshot: &RootSnapshot, paths: &[PathBuf], indexed: &[String]) -> Result<Option<Vec<String>>, RepositoryError> {
    let root = Path::new(&snapshot.root.path);
    let metadata = fs::symlink_metadata(root).map_err(|_| RepositoryError::Conflict)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() { return Err(RepositoryError::Conflict); }
    let mut builder = GlobSetBuilder::new();
    for pattern in &snapshot.exclude_patterns {
        builder.add(Glob::new(pattern).map_err(|_| RepositoryError::InvalidInput)?);
    }
    let excludes = builder.build().map_err(|_| RepositoryError::InvalidInput)?;
    let mut selected = BTreeMap::new();
    for path in paths {
        if !path.is_absolute() { return Err(RepositoryError::InvalidInput); }
        let relative = path.strip_prefix(root).map_err(|_| RepositoryError::InvalidInput)?;
        if relative.as_os_str().is_empty() { return Ok(None); }
        let relative = relative.to_str().ok_or(RepositoryError::InvalidInput)?.replace(std::path::MAIN_SEPARATOR, "/");
        normalize_relative_path(&relative, snapshot.root.path_style).map_err(|_| RepositoryError::InvalidInput)?;
        if private(Path::new(&relative)) || excluded(&relative, &excludes) { continue; }
        let metadata = match safe_event(root, &relative) {
            Ok(metadata) => metadata,
            Err(RepositoryError::Conflict) => return Ok(None),
            Err(error) => return Err(error),
        };
        if metadata.as_ref().is_some_and(|metadata| metadata.is_dir()) { return Ok(None); }
        if metadata.as_ref().is_some_and(|metadata| !metadata.is_file()) { continue; }
        let normalized = normalize_relative_path(&relative, snapshot.root.path_style).map_err(|_| RepositoryError::InvalidInput)?;
        if video(Path::new(&relative)) {
            if metadata.is_some() || indexed.iter().any(|path| normalize_relative_path(path, snapshot.root.path_style).is_ok_and(|path| path == normalized)) {
                selected.insert(normalized, relative);
                if selected.len() > 128 { return Ok(None); }
            }
            continue;
        }
        let extension = Path::new(&relative).extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
        if !matches!(extension.as_str(), "nfo" | "jpg" | "jpeg" | "png" | "webp" | "srt" | "ass" | "ssa" | "sub" | "idx" | "vtt") {
            // A vanished extensionless path can be a removed/renamed directory.
            if metadata.is_none() && (extension.is_empty() || indexed.iter().any(|source| source.starts_with(&format!("{relative}/")))) { return Ok(None); }
            continue;
        }
        let series = Path::new(&relative).file_name().and_then(|name| name.to_str()).is_some_and(|name| name.eq_ignore_ascii_case("tvshow.nfo"));
        let actor_directory = Path::new(&relative).parent().filter(|parent| parent.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.eq_ignore_ascii_case(".actors")));
        let parent = actor_directory.and_then(Path::parent).or_else(|| Path::new(&relative).parent()).unwrap_or_else(|| Path::new(""));
        let mut candidates = indexed.iter().filter(|path| (Path::new(path).parent() == Some(parent) || (series && Path::new(path).starts_with(parent))) && video(Path::new(path))).cloned().collect::<Vec<_>>();
        let mut directories = vec![parent.to_path_buf()];
        let mut visited = 0usize;
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(root.join(&directory)).map_err(|_| RepositoryError::Conflict)? {
                visited += 1;
                if visited > 65_536 { return Ok(None); }
                let entry = entry.map_err(|_| RepositoryError::Conflict)?;
                let metadata = entry.file_type().map_err(|_| RepositoryError::Conflict)?;
                let source = directory.join(entry.file_name());
                let Some(source_text) = source.to_str() else { continue; };
                let source_text = source_text.replace(std::path::MAIN_SEPARATOR, "/");
                if private(&source) || excluded(&source_text, &excludes) { continue; }
                if series && metadata.is_dir() { directories.push(source); }
                else if metadata.is_file() && video(&entry.path()) {
                    if series || actor_directory.is_some() || companion_matches(&source_text, &relative) {
                        candidates.push(source_text);
                        if candidates.len() > 256 { return Ok(None); }
                    }
                }
            }
        }
        for source in candidates {
            if private(Path::new(&source)) || excluded(&source, &excludes) { continue; }
            if series || actor_directory.is_some() || companion_matches(&source, &relative) {
                let key = normalize_relative_path(&source, snapshot.root.path_style).map_err(|_| RepositoryError::InvalidInput)?;
                selected.insert(key, source);
                if selected.len() > 128 { return Ok(None); }
            }
        }
    }
    Ok(Some(selected.into_values().collect()))
}

impl ScanService {
    pub(super) async fn reconcile_selected_reviews(&self, task_id: Uuid, root_id: Uuid, media_id: Uuid) -> Result<(), RepositoryError> {
        let mut tx = self.repo.pool().begin().await?;
        let affected: Vec<String> = sqlx::query_scalar("SELECT s.task_id FROM scan_sessions s JOIN scrape_tasks t ON t.id=s.task_id JOIN library_refresh_items i ON i.task_id=s.task_id WHERE s.root_id=? AND s.mode='automatic' AND t.state='waiting_for_user' AND i.media_item_id=? AND i.state IN ('needs_review','failed') AND s.task_id<>?")
            .bind(root_id.to_string()).bind(media_id.to_string()).bind(task_id.to_string()).fetch_all(&mut *tx).await?;
        for old in &affected {
            sqlx::query("UPDATE library_refresh_items SET (state,reason,run_id,updated_at)=(SELECT state,reason,run_id,updated_at FROM library_refresh_items WHERE task_id=? AND media_item_id=?) WHERE task_id=? AND media_item_id=?")
                .bind(task_id.to_string()).bind(media_id.to_string()).bind(old).bind(media_id.to_string()).execute(&mut *tx).await?;
            sqlx::query("UPDATE scan_sessions SET matched_count=(SELECT count(*) FROM library_refresh_items WHERE task_id=? AND state='matched'),review_count=(SELECT count(*) FROM library_refresh_items WHERE task_id=? AND state='needs_review'),metadata_failed_count=(SELECT count(*) FROM library_refresh_items WHERE task_id=? AND state='failed') WHERE task_id=?")
                .bind(old).bind(old).bind(old).bind(old).execute(&mut *tx).await?;
            sqlx::query("UPDATE scrape_tasks SET state='succeeded',updated_at=MAX(updated_at,?) WHERE id=? AND state='waiting_for_user' AND EXISTS(SELECT 1 FROM scan_sessions s WHERE s.task_id=? AND s.review_count=0 AND s.metadata_failed_count=0) AND NOT EXISTS(SELECT 1 FROM scan_candidates WHERE task_id=? AND status='pending')")
                .bind(Utc::now().timestamp_millis()).bind(old).bind(old).bind(old).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        for old in affected {
            let old = Uuid::parse_str(&old).map_err(|_| RepositoryError::CorruptData)?;
            self.emit(old).await;
        }
        Ok(())
    }

    pub(crate) async fn start_watched_paths(&self, expected: &LibraryRoot, paths: &[PathBuf]) -> Result<Option<ScanTaskView>, RepositoryError> {
        if paths.len() > 128 { return Ok(Some(self.start_watched(expected).await?)); }
        let snapshot = self.snapshot(expected.id).await?;
        if &snapshot.root != expected || !snapshot.root.enabled || snapshot.root.kind.is_none() || snapshot.root.kind == Some(MediaKind::AdultVideo) {
            return Err(RepositoryError::Conflict);
        }
        if !matches!(snapshot.root.kind, Some(MediaKind::Movie | MediaKind::TvSeries | MediaKind::Anime)) {
            return self.start_watched(expected).await.map(Some);
        }
        let mut builder = GlobSetBuilder::new();
        for pattern in &snapshot.exclude_patterns {
            builder.add(Glob::new(pattern).map_err(|_| RepositoryError::InvalidInput)?);
        }
        let excludes = builder.build().map_err(|_| RepositoryError::InvalidInput)?;
        let mut indexed = BTreeMap::new();
        for path in paths {
            let relative = path.strip_prefix(Path::new(&expected.path)).map_err(|_| RepositoryError::InvalidInput)?;
            if relative.as_os_str().is_empty() { return self.start_watched(expected).await.map(Some); }
            let relative = relative.to_str().ok_or(RepositoryError::InvalidInput)?.replace(std::path::MAIN_SEPARATOR, "/");
            let normalized = normalize_relative_path(&relative, expected.path_style).map_err(|_| RepositoryError::InvalidInput)?;
            if private(Path::new(&relative)) || excluded(&relative, &excludes) { continue; }
            if video(Path::new(&relative)) {
                let source: Option<String> = sqlx::query_scalar("SELECT relative_path FROM media_files WHERE root_id=? AND normalized_relative_path=?")
                    .bind(expected.id.to_string()).bind(normalized).fetch_optional(self.repo.pool()).await?;
                if let Some(source) = source { indexed.insert(source.clone(), source); }
                continue;
            }
            let path = Path::new(&relative);
            let filename = path.file_name().and_then(|part| part.to_str()).unwrap_or("").to_ascii_lowercase();
            let actors = path.parent().filter(|parent| parent.file_name().and_then(|part| part.to_str()).is_some_and(|part| part.eq_ignore_ascii_case(".actors")));
            let parent = actors.and_then(Path::parent).or_else(|| path.parent()).unwrap_or_else(|| Path::new(""));
            let parent = parent.to_str().ok_or(RepositoryError::InvalidInput)?;
            let prefix = if parent.is_empty() { String::new() } else { format!("{}/", normalize_relative_path(parent, expected.path_style).map_err(|_| RepositoryError::InvalidInput)?) };
            let generic = actors.is_some() || matches!(filename.as_str(), "movie.nfo" | "tvshow.nfo" | "album.nfo" | "poster.jpg" | "poster.png" | "poster.jpeg" | "folder.jpg" | "cover.jpg" | "cover.png") || filename.starts_with("fanart.") || filename.starts_with("backdrop.");
            let sources: Vec<String> = sqlx::query_scalar("WITH candidates AS (SELECT relative_path,normalized_relative_path,substr(normalized_relative_path,1,length(normalized_relative_path)-CASE WHEN lower(relative_path) LIKE '%.ts' THEN 3 WHEN lower(relative_path) LIKE '%.mpeg' OR lower(relative_path) LIKE '%.webm' OR lower(relative_path) LIKE '%.m2ts' OR lower(relative_path) LIKE '%.strm' THEN 5 ELSE 4 END) AS stem FROM media_files WHERE root_id=? AND normalized_relative_path>=? AND normalized_relative_path<? AND (? OR instr(substr(normalized_relative_path,length(?)+1),'/')=0) AND (lower(relative_path) LIKE '%.mkv' OR lower(relative_path) LIKE '%.mp4' OR lower(relative_path) LIKE '%.avi' OR lower(relative_path) LIKE '%.mov' OR lower(relative_path) LIKE '%.m4v' OR lower(relative_path) LIKE '%.wmv' OR lower(relative_path) LIKE '%.webm' OR lower(relative_path) LIKE '%.ts' OR lower(relative_path) LIKE '%.m2ts' OR lower(relative_path) LIKE '%.mpg' OR lower(relative_path) LIKE '%.mpeg' OR lower(relative_path) LIKE '%.strm')) SELECT relative_path FROM candidates WHERE ? OR substr(lower(?),1,length(stem)+1) IN (lower(stem)||'.',lower(stem)||'-') LIMIT 129")
                .bind(expected.id.to_string()).bind(&prefix).bind(format!("{prefix}\u{10ffff}"))
                .bind(filename == "tvshow.nfo").bind(&prefix).bind(generic).bind(normalized).fetch_all(self.repo.pool()).await?;
            for source in sources {
                if !private(Path::new(&source)) && !excluded(&source, &excludes) { indexed.insert(source.clone(), source); }
            }
            if indexed.len() > 128 { return self.start_watched(expected).await.map(Some); }
        }
        let indexed = indexed.into_values().collect::<Vec<_>>();
        let paths = paths.to_vec();
        let selection = tokio::task::spawn_blocking(move || resolve(&snapshot, &paths, &indexed)).await.map_err(|_| RepositoryError::Database)??;
        match selection {
            Some(selected) if selected.is_empty() => Ok(None),
            Some(selected) => self.start_inner(expected.id, Some(expected), self.scraper.is_some(), None, Some(selected)).await.map(Some),
            None => self.start_watched(expected).await.map(Some),
        }
    }

    pub(super) async fn remove_missing_selected(&self, task_id: Uuid, snapshot: &RootSnapshot, cancel: &Arc<AtomicBool>) -> Result<(), RepositoryError> {
        let Some(paths) = normalized_selection(snapshot)? else { return Ok(()); };
        if paths.is_empty() { return Ok(()); }
        let paths = serde_json::to_string(&paths).map_err(|_| RepositoryError::CorruptData)?;
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("UPDATE library_roots SET updated_at=updated_at WHERE id=?").bind(snapshot.root.id.to_string()).execute(&mut *tx).await?;
        let current = snapshot_tx(&mut tx, snapshot.root.id).await?;
        let saved: String = sqlx::query_scalar("SELECT root_snapshot_json FROM scan_sessions WHERE task_id=?")
            .bind(task_id.to_string()).fetch_one(&mut *tx).await?;
        let saved: RootSnapshot = serde_json::from_str(&saved).map_err(|_| RepositoryError::CorruptData)?;
        let running: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM scrape_tasks t JOIN scan_sessions s ON s.task_id=t.id WHERE t.id=? AND t.state='running' AND s.mode='automatic' AND s.root_id=?)")
            .bind(task_id.to_string()).bind(snapshot.root.id.to_string()).fetch_one(&mut *tx).await?;
        let ancestors = crate::library_ready::ancestor_isolation("m");
        let isolated: bool = sqlx::query_scalar(&format!("SELECT EXISTS(SELECT 1 FROM media_files f JOIN media_items m ON m.id=f.media_item_id WHERE f.root_id=? AND (m.adult_isolated=1 OR m.kind='adult_video' OR NOT ({ancestors}) OR EXISTS(SELECT 1 FROM media_files other JOIN library_roots r ON r.id=other.root_id WHERE other.media_item_id=m.id AND (r.kind='adult_video' OR r.enabled=0))))"))
            .bind(snapshot.root.id.to_string()).fetch_one(&mut *tx).await?;
        if cancel.load(Ordering::Acquire) {
            return Err(RepositoryError::Provider(mediaatlas_domain::ProviderFailure::Cancelled));
        }
        if !running || isolated || !current.root.enabled || !same_config(snapshot, &current) || saved.selected_sources != snapshot.selected_sources {
            return Err(RepositoryError::Conflict);
        }
        let rows = sqlx::query("SELECT id,relative_path,normalized_relative_path FROM media_files WHERE root_id=? AND normalized_relative_path IN (SELECT value FROM json_each(?))")
            .bind(snapshot.root.id.to_string()).bind(paths).fetch_all(&mut *tx).await?;
        for row in rows {
            if cancel.load(Ordering::Acquire) { return Err(RepositoryError::Provider(mediaatlas_domain::ProviderFailure::Cancelled)); }
            let relative: String = row.try_get("relative_path")?;
            if !video(Path::new(&relative)) { continue; }
            if crate::media_output::source_is_missing(Path::new(&snapshot.root.path), &relative)? {
                let removed = sqlx::query("DELETE FROM media_files WHERE id=? AND root_id=? AND normalized_relative_path=?")
                    .bind(row.try_get::<String, _>("id")?).bind(snapshot.root.id.to_string()).bind(row.try_get::<String, _>("normalized_relative_path")?).execute(&mut *tx).await?;
                if removed.rows_affected() == 1 {
                    sqlx::query("DELETE FROM scan_source_fingerprints WHERE root_id=? AND normalized_relative_path=?")
                        .bind(snapshot.root.id.to_string()).bind(row.try_get::<String, _>("normalized_relative_path")?).execute(&mut *tx).await?;
                }
            }
        }
        if cancel.load(Ordering::Acquire) { return Err(RepositoryError::Provider(mediaatlas_domain::ProviderFailure::Cancelled)); }
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "watched_tests.rs"]
mod tests;
