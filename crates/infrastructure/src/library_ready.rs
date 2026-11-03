//! Local availability is independent from metadata completion.
use crate::library::{LibraryRepository, RepositoryError};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use sqlx::{Row, Sqlite, Transaction};
use std::{
    io::Read,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "m4v", "wmv", "webm", "ts", "m2ts", "mpg", "mpeg", "vob", "ogv",
];
/// m is the surrounding media_items alias. Readiness is filtered before SQL pagination.
/// A local file association is mandatory; remote-only metadata graphs are tools.
pub fn ready_predicate() -> String {
    let direct = direct_ready("m");
    let descendant = direct_ready("completed");
    let typed = typed_descendant("m", "completed");
    format!("(({direct}) OR (m.kind IN ('tv_series','tv_season') AND EXISTS(SELECT 1 FROM media_items completed WHERE ({descendant}) AND ({typed}))))")
}
/// Only the P2 typed Series → Season → Episode/Anime hierarchy participates.
/// Arbitrary media relations never grant completion or access to local files.
pub(crate) fn typed_descendant(parent: &str, child: &str) -> String {
    let parent_safe = ordinary_node(parent);
    let child_safe = ordinary_node(child);
    let season_safe = ordinary_node("sm");
    format!("(({parent_safe}) AND ({child_safe}) AND (({parent}.kind IN ('tv_series','tv_season') AND {child}.kind IN ('tv_episode','anime') AND EXISTS(SELECT 1 FROM media_relations rel WHERE rel.parent_id={parent}.id AND rel.child_id={child}.id AND rel.relation_type='episode')) OR ({parent}.kind='tv_series' AND {child}.kind IN ('tv_episode','anime') AND EXISTS(SELECT 1 FROM media_relations sr JOIN media_items sm ON sm.id=sr.child_id AND sm.kind='tv_season' JOIN media_relations er ON er.parent_id=sm.id AND er.relation_type='episode' WHERE sr.parent_id={parent}.id AND sr.relation_type='season' AND er.child_id={child}.id AND ({season_safe})))))")
}
/// Resolve only the two supported hierarchy levels before inspecting metadata.
/// This keeps per-card metadata checks bounded by relations rather than the library.
pub(crate) fn ancestor_ids(alias: &str) -> String {
    format!("SELECT ar.parent_id FROM media_relations ar WHERE ar.child_id={alias}.id UNION SELECT pr.parent_id FROM media_relations cr JOIN media_relations pr ON pr.child_id=cr.parent_id WHERE cr.child_id={alias}.id")
}
pub(crate) fn ancestor_isolation(alias: &str) -> String {
    format!("NOT EXISTS(SELECT 1 FROM media_relations ar JOIN media_items am ON am.id=ar.parent_id WHERE ar.child_id={alias}.id AND (am.kind='adult_video' OR am.adult_isolated=1 OR EXISTS(SELECT 1 FROM media_files af JOIN library_roots rr ON rr.id=af.root_id WHERE af.media_item_id=am.id AND rr.kind='adult_video'))) AND NOT EXISTS(SELECT 1 FROM media_relations cr JOIN media_relations pr ON pr.child_id=cr.parent_id JOIN media_items am ON am.id=pr.parent_id WHERE cr.child_id={alias}.id AND (am.kind='adult_video' OR am.adult_isolated=1 OR EXISTS(SELECT 1 FROM media_files af JOIN library_roots rr ON rr.id=af.root_id WHERE af.media_item_id=am.id AND rr.kind='adult_video')))")
}
fn ordinary_node(alias: &str) -> String {
    let ancestors = ancestor_isolation(alias);
    format!("{alias}.kind<>'adult_video' AND {alias}.adult_isolated=0 AND NOT EXISTS(SELECT 1 FROM media_files af JOIN library_roots ar ON ar.id=af.root_id WHERE af.media_item_id={alias}.id AND (ar.kind='adult_video' OR ar.enabled=0)) AND ({ancestors})")
}
fn metadata_evidence(alias: &str) -> String {
    format!("(EXISTS(SELECT 1 FROM local_metadata_provenance p WHERE p.media_item_id={alias}.id) OR ({alias}.kind<>'adult_video' AND EXISTS(SELECT 1 FROM video_metadata_sources v WHERE v.media_item_id={alias}.id)) OR ({alias}.kind='adult_video' AND EXISTS(SELECT 1 FROM adult_metadata_sources a WHERE a.media_item_id={alias}.id)))")
}
/// Filename-derived local fields do not establish metadata completion. The
/// health indicator is independent from whether a local item can be browsed.
pub(crate) fn metadata_complete_predicate(alias: &str) -> String {
    let own = metadata_evidence(alias);
    let inherited = metadata_evidence("metadata_parent");
    let typed = typed_descendant("metadata_parent", alias);
    let parents = ancestor_ids(alias);
    format!("(({own}) OR EXISTS(SELECT 1 FROM metadata_values mv WHERE mv.media_item_id={alias}.id AND (mv.is_user_modified=1 OR (mv.provider_id IS NOT NULL AND mv.provider_id<>'local'))) OR ({alias}.kind IN ('tv_episode','anime') AND EXISTS(SELECT 1 FROM media_items metadata_parent WHERE metadata_parent.id IN ({parents}) AND ({typed}) AND ({inherited}))))")
}
fn direct_ready(alias: &str) -> String {
    let extensions = VIDEO_EXTENSIONS
        .iter()
        .map(|e| format!("lower(f.relative_path) LIKE '%.{e}'"))
        .collect::<Vec<_>>()
        .join(" OR ");
    // Both libraries follow actual local files. Adult visibility is separately
    // restricted by the explicit grant and isolated browse/open scope.
    format!("{alias}.kind IN ('movie','tv_episode','anime','music_video','concert','adult_video') AND EXISTS(SELECT 1 FROM media_files f JOIN library_roots lr ON lr.id=f.root_id WHERE f.media_item_id={alias}.id AND lr.enabled=1 AND (lr.kind IS NULL OR lr.kind={alias}.kind OR (lr.kind IN ('tv_series','tv_season') AND {alias}.kind IN ('tv_episode','anime'))) AND ({extensions}))")
}
pub(crate) async fn record_local(
    tx: &mut Transaction<'_, Sqlite>,
    media_id: Uuid,
    source_kind: &str,
    reference: &str,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO local_metadata_provenance(media_item_id,source_kind,source_reference,confirmed_at) VALUES(?,?,?,?) ON CONFLICT(media_item_id) DO UPDATE SET source_kind=excluded.source_kind,source_reference=excluded.source_reference,confirmed_at=excluded.confirmed_at")
        .bind(media_id.to_string()).bind(source_kind).bind(reference).bind(chrono::Utc::now().timestamp_millis()).execute(&mut **tx).await?;
    Ok(())
}

/// Backfill only actual safely-read, validated legacy NFO metadata that was already
/// indexed by an explicit scan confirmation. Filename-derived `provider_id=local`
/// rows alone never establish metadata completion. No network or source writes are performed.
pub async fn backfill_local_nfo(repo: &LibraryRepository) -> Result<u64, RepositoryError> {
    backfill_nfo_lane(repo, None).await
}
/// Adult legacy metadata is inspected only while the explicit typed grant lives.
pub async fn backfill_adult_local_nfo(
    repo: &LibraryRepository,
    access: crate::media_output::AdultOutputAccess,
) -> Result<u64, RepositoryError> {
    access.ensure()?;
    backfill_nfo_lane(repo, Some(access)).await
}
fn backfill_scope(adult: bool) -> String {
    if adult {
        "m.kind='adult_video' AND m.adult_isolated=1 AND r.kind='adult_video' AND NOT EXISTS(SELECT 1 FROM media_files af JOIN library_roots ar ON ar.id=af.root_id WHERE af.media_item_id=m.id AND (ar.kind IS NULL OR ar.kind<>'adult_video' OR ar.enabled=0))".to_owned()
    } else {
        format!("m.kind IN ('movie','tv_episode','anime','music_video','concert') AND m.adult_isolated=0 AND (r.kind IS NULL OR r.kind<>'adult_video') AND NOT EXISTS(SELECT 1 FROM media_files af JOIN library_roots ar ON ar.id=af.root_id WHERE af.media_item_id=m.id AND ar.kind='adult_video') AND ({})", ancestor_isolation("m"))
    }
}
async fn backfill_nfo_lane(
    repo: &LibraryRepository,
    access: Option<crate::media_output::AdultOutputAccess>,
) -> Result<u64, RepositoryError> {
    let scope = backfill_scope(access.is_some());
    let sql=format!("SELECT f.id,f.media_item_id,f.relative_path,r.path FROM media_files f JOIN library_roots r ON r.id=f.root_id JOIN media_items m ON m.id=f.media_item_id WHERE lower(f.relative_path) LIKE '%.nfo' AND r.enabled=1 AND ({scope}) AND NOT EXISTS(SELECT 1 FROM local_metadata_provenance p WHERE p.media_item_id=f.media_item_id)");
    let rows = sqlx::query(&sql).fetch_all(repo.pool()).await?;
    let mut count = 0;
    for row in rows {
        if let Some(access) = &access {
            access.ensure()?;
        }
        let file_id: String = row.try_get("id")?;
        let media_id: String = row.try_get("media_item_id")?;
        let relative: String = row.try_get("relative_path")?;
        let root: String = row.try_get("path")?;
        let check_root = root.clone();
        let check_relative = relative.clone();
        let read_access = access.clone();
        let valid = tokio::task::spawn_blocking(move || {
            if let Some(access) = &read_access {
                access.ensure()?;
            }
            Ok::<_, RepositoryError>(valid_nfo(&check_root, &check_relative))
        })
        .await
        .map_err(|_| RepositoryError::Database)??;
        let Some(title) = valid else {
            continue;
        };
        let mut tx = repo.pool().begin().await?;
        sqlx::query("UPDATE media_files SET size_bytes=size_bytes WHERE id=?")
            .bind(&file_id)
            .execute(&mut *tx)
            .await?;
        let current_sql=format!("SELECT EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id JOIN media_items m ON m.id=f.media_item_id WHERE f.id=? AND f.media_item_id=? AND f.relative_path=? AND r.path=? AND r.enabled=1 AND ({scope}) AND EXISTS(SELECT 1 FROM metadata_values v WHERE v.media_item_id=f.media_item_id AND v.field_key='title' AND v.provider_id='local' AND v.value_json=json_quote(?)))");
        let current: bool = sqlx::query_scalar(&current_sql)
            .bind(&file_id)
            .bind(&media_id)
            .bind(&relative)
            .bind(&root)
            .bind(&title)
            .fetch_one(&mut *tx)
            .await?;
        if current {
            if let Some(access) = &access {
                access.ensure()?;
            }
            record_local(
                &mut tx,
                Uuid::parse_str(&media_id).map_err(|_| RepositoryError::CorruptData)?,
                "nfo",
                &format!("legacy-file:{file_id}"),
            )
            .await?;
            count += 1;
        }
        if let Some(access) = &access {
            access.ensure()?;
        }
        tx.commit().await?;
    }
    if let Some(access) = &access {
        access.ensure()?;
    }
    Ok(count)
}
fn valid_nfo(root: &str, relative: &str) -> Option<String> {
    let read = || -> Option<String> {
        let relative = Path::new(relative);
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return None;
        }
        let path = Path::new(root).join(relative);
        if !path.is_absolute() {
            return None;
        }
        let mut anchor = PathBuf::new();
        let mut parts = vec![];
        for component in path.components() {
            match component {
                Component::Prefix(p) => anchor.push(p.as_os_str()),
                Component::RootDir => anchor.push(component.as_os_str()),
                Component::Normal(n) => parts.push(n.to_os_string()),
                _ => return None,
            }
        }
        let name = parts.pop()?;
        let mut dir = Dir::open_ambient_dir(anchor, ambient_authority()).ok()?;
        for part in parts {
            let mut options = OpenOptions::new();
            options
                .read(true)
                .maybe_dir(true)
                .follow(FollowSymlinks::No);
            let opened = dir.open_with(part, &options).ok()?;
            if !opened.metadata().ok()?.is_dir() {
                return None;
            }
            dir = Dir::from_std_file(opened.into_std());
        }
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let file = dir.open_with(name, &options).ok()?;
        let info = file.metadata().ok()?;
        if !info.is_file() || info.len() > 2 * 1024 * 1024 {
            return None;
        }
        let mut content = String::new();
        file.take(2 * 1024 * 1024 + 1)
            .read_to_string(&mut content)
            .ok()?;
        if content.len() > 2 * 1024 * 1024 {
            return None;
        }
        Some(content)
    };
    let content = read()?;
    let upper = content.to_ascii_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
        return None;
    }
    let doc = roxmltree::Document::parse(&content).ok()?;
    if !matches!(
        doc.root_element().tag_name().name(),
        "movie" | "tvshow" | "episodedetails" | "musicvideo"
    ) {
        return None;
    }
    doc.root_element()
        .children()
        .find(|n| n.has_tag_name("title"))
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}
