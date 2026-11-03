//! Read-only, bounded maintenance review. Repairs stay explicit tool workflows.
use crate::library::{LibraryRepository, RepositoryError};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row, Sqlite};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthQuery {
    pub cursor: Option<String>,
    pub limit: u16,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthItem {
    pub media_id: Uuid,
    pub title: String,
    pub issues: Vec<String>,
    /// Navigation only. The destination still requires its own preview/confirm.
    pub repair_route: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthPage {
    pub items: Vec<HealthItem>,
    pub next_cursor: Option<String>,
}
#[derive(Clone)]
pub struct HealthService {
    repo: LibraryRepository,
    artwork_cache: Option<PathBuf>,
}
impl HealthService {
    pub fn new(repo: LibraryRepository) -> Self {
        Self {
            repo,
            artwork_cache: None,
        }
    }
    /// Only application composition supplies this trusted ordinary cache root;
    /// IPC callers cannot choose paths. Arbitrary database cache paths are never read.
    pub fn with_artwork_cache(mut self, cache: PathBuf) -> Self {
        self.artwork_cache = Some(cache);
        self
    }
    /// Pagination is over reviewed records, including healthy ones, so bounded
    /// work and a stable continuation do not depend on the number of failures.
    pub async fn check(&self, query: &HealthQuery) -> Result<HealthPage, RepositoryError> {
        if !(1..=200).contains(&query.limit) {
            return Err(RepositoryError::InvalidInput);
        }
        let after = match &query.cursor {
            Some(id) => Uuid::parse_str(id)
                .map_err(|_| RepositoryError::InvalidInput)?
                .to_string(),
            None => String::new(),
        };
        let rows=sqlx::query("SELECT m.id,m.title, EXISTS(SELECT 1 FROM artworks a WHERE a.media_item_id=m.id AND a.kind IN('poster','cover','album_cover')) AS has_poster, EXISTS(SELECT 1 FROM media_files f WHERE f.media_item_id=m.id AND lower(f.relative_path) LIKE '%.nfo') AS has_nfo, (EXISTS(SELECT 1 FROM local_metadata_provenance p WHERE p.media_item_id=m.id) OR EXISTS(SELECT 1 FROM video_metadata_sources v WHERE v.media_item_id=m.id) OR EXISTS(SELECT 1 FROM metadata_values v WHERE v.media_item_id=m.id AND (v.is_user_modified=1 OR (v.provider_id IS NOT NULL AND v.provider_id!='local')))) AS has_metadata, EXISTS(SELECT 1 FROM external_ids x JOIN external_ids y ON y.provider_id=x.provider_id AND y.namespace=x.namespace AND y.external_id=x.external_id JOIN media_items other ON other.id=y.media_item_id WHERE x.media_item_id=m.id AND other.id<>m.id AND other.kind!='adult_video' AND other.adult_isolated=0 AND NOT EXISTS(SELECT 1 FROM media_files ff JOIN library_roots rr ON rr.id=ff.root_id WHERE ff.media_item_id=other.id AND rr.kind='adult_video')) AS duplicate_identity FROM media_items m WHERE m.id>? AND m.kind!='adult_video' AND m.adult_isolated=0 AND NOT EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=m.id AND r.kind='adult_video') ORDER BY m.id LIMIT ?")
            .bind(after).bind(i64::from(query.limit)+1).fetch_all(self.repo.pool()).await?;
        let has_more = rows.len() > usize::from(query.limit);
        let mut items = Vec::new();
        for row in rows.into_iter().take(usize::from(query.limit)) {
            let id: String = row.try_get("id")?;
            let media_id = Uuid::parse_str(&id).map_err(|_| RepositoryError::CorruptData)?;
            let mut issues = Vec::new();
            if !row.try_get::<bool, _>("has_poster")? {
                issues.push("missing_poster".into());
            }
            if !row.try_get::<bool, _>("has_nfo")? {
                issues.push("missing_nfo".into());
            }
            if !row.try_get::<bool, _>("has_metadata")? {
                issues.push("incomplete_metadata".into());
            }
            if row.try_get::<bool, _>("duplicate_identity")? {
                issues.push("duplicate_identity".into());
            }
            // Cap unusually large work graphs; surface the cap instead of silently
            // asserting all files were checked. No media bytes are read.
            let files=sqlx::query("SELECT f.relative_path,r.path,r.enabled FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=? ORDER BY f.id LIMIT 1025").bind(&id).fetch_all(self.repo.pool()).await?;
            if files.is_empty() {
                issues.push("no_local_file".into());
            }
            if files.len() > 1024 {
                issues.push("file_check_limit".into());
            }
            let checks = files
                .into_iter()
                .take(1024)
                .map(|f| {
                    Ok((
                        f.try_get::<String, _>("path")?,
                        f.try_get::<String, _>("relative_path")?,
                        f.try_get::<bool, _>("enabled")?,
                    ))
                })
                .collect::<Result<Vec<_>, RepositoryError>>()?;
            let unavailable = tokio::task::spawn_blocking(move || {
                checks.iter().any(|(root, relative, enabled)| {
                    !*enabled || !regular_file_available(Path::new(root), Path::new(relative))
                })
            })
            .await
            .map_err(|_| RepositoryError::Database)?;
            if unavailable {
                issues.push("offline_file".into());
            }
            let artworks=sqlx::query("SELECT cache_path,source_url FROM artworks WHERE media_item_id=? ORDER BY id LIMIT 257").bind(&id).fetch_all(self.repo.pool()).await?;
            if artworks.len() > 256 {
                issues.push("asset_check_limit".into());
            }
            let mut asset_checks = Vec::new();
            let mut unverified = false;
            let mut missing = false;
            for art in artworks.into_iter().take(256) {
                let cache_path: Option<String> = art.try_get("cache_path")?;
                let source_url: Option<String> = art.try_get("source_url")?;
                if let Some(path) = cache_path {
                    if let Some(cache) = self
                        .artwork_cache
                        .as_ref()
                        .filter(|cache| Path::new(&path).parent() == Some(cache.as_path()))
                    {
                        if let Some(name) = Path::new(&path).file_name() {
                            asset_checks.push((cache.clone(), PathBuf::from(name)));
                            continue;
                        }
                    }
                    unverified = true;
                    continue;
                }
                if let Some(source) = source_url
                    .as_deref()
                    .and_then(|source| source.strip_prefix("library://"))
                {
                    if let Some((root_id, relative)) = source.split_once('/') {
                        if let Ok(root_id) = Uuid::parse_str(root_id) {
                            if let Some(root) = self.repo.get_root(root_id).await? {
                                let associated:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM media_files WHERE media_item_id=? AND root_id=?)").bind(&id).bind(root_id.to_string()).fetch_one(self.repo.pool()).await?;
                                if associated
                                    && root.enabled
                                    && root.kind != Some(mediaatlas_domain::MediaKind::AdultVideo)
                                {
                                    asset_checks
                                        .push((PathBuf::from(root.path), PathBuf::from(relative)));
                                    continue;
                                }
                            }
                        }
                    }
                    unverified = true;
                } else {
                    missing = true;
                }
            }
            let missing = missing
                || tokio::task::spawn_blocking(move || {
                    asset_checks
                        .iter()
                        .any(|(root, relative)| !regular_file_available(root, relative))
                })
                .await
                .map_err(|_| RepositoryError::Database)?;
            if missing {
                issues.push("missing_artwork_asset".into());
            }
            if unverified {
                issues.push("unverified_artwork_asset".into());
            }
            items.push(HealthItem {
                media_id,
                title: row.try_get("title")?,
                issues,
                repair_route: format!("/tools?media={media_id}"),
            });
        }
        let next_cursor = has_more
            .then(|| items.last().map(|i| i.media_id.to_string()))
            .flatten();
        // Revalidate after filesystem checks; a concurrent adult reclassification
        // must not return its title. Preserve the original continuation boundary.
        let mut query=QueryBuilder::<Sqlite>::new("SELECT m.id FROM media_items m WHERE m.kind!='adult_video' AND m.adult_isolated=0 AND NOT EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=m.id AND r.kind='adult_video') AND m.id IN (");
        let mut separated = query.separated(",");
        for item in &items {
            separated.push_bind(item.media_id.to_string());
        }
        separated.push_unseparated(")");
        let allowed: HashSet<String> = query
            .build_query_scalar()
            .fetch_all(self.repo.pool())
            .await?
            .into_iter()
            .collect();
        let visible = items
            .into_iter()
            .filter(|item| allowed.contains(&item.media_id.to_string()))
            .collect();
        Ok(HealthPage {
            items: visible,
            next_cursor,
        })
    }
}
fn regular_file_available(root: &Path, relative: &Path) -> bool {
    fn check(root: &Path, relative: &Path) -> Option<()> {
        if !root.is_absolute()
            || !relative
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        {
            return None;
        }
        let joined = root.join(relative);
        let mut anchor = PathBuf::new();
        let mut parts = Vec::new();
        for component in joined.components() {
            match component {
                Component::Prefix(prefix) => anchor.push(prefix.as_os_str()),
                Component::RootDir => anchor.push(component.as_os_str()),
                Component::Normal(name) => parts.push(name.to_os_string()),
                _ => return None,
            }
        }
        let final_name = parts.pop()?;
        let mut directories = vec![Dir::open_ambient_dir(anchor, ambient_authority()).ok()?];
        for name in parts {
            let mut options = OpenOptions::new();
            options
                .read(true)
                .maybe_dir(true)
                .follow(FollowSymlinks::No);
            let opened = directories.last()?.open_with(name, &options).ok()?;
            if !opened.metadata().ok()?.is_dir() {
                return None;
            }
            directories.push(Dir::from_std_file(opened.into_std()));
        }
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        directories
            .last()?
            .open_with(final_name, &options)
            .ok()?
            .metadata()
            .ok()?
            .is_file()
            .then_some(())
    }
    check(root, relative).is_some()
}

#[cfg(test)]
mod tests;
