//! Local browsing and root settings. SQL is parameterized and visibility is enforced here.

mod thumbnails;

use chrono::{DateTime, Utc};
use globset::Glob;
use mediaatlas_application::library::*;
use mediaatlas_domain::{normalize_root_path, LibraryRoot, MediaItem, MediaKind, PathStyle};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteRow, QueryBuilder, Row, Sqlite};
use std::{path::PathBuf, sync::OnceLock};

fn root_mutation_gate() -> &'static tokio::sync::Mutex<()> {
    static GATE: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    GATE.get_or_init(|| tokio::sync::Mutex::new(()))
}
use uuid::Uuid;

use crate::library::{LibraryRepository, RepositoryError};

type Result<T> = std::result::Result<T, RepositoryError>;

const POSTER: &str = "EXISTS(SELECT 1 FROM artworks a WHERE a.media_item_id=m.id AND a.kind IN ('poster','cover','album_cover'))";
const BACKDROP: &str =
    "EXISTS(SELECT 1 FROM artworks a WHERE a.media_item_id=m.id AND a.kind='backdrop')";
const NFO: &str = "EXISTS(SELECT 1 FROM media_files f WHERE f.media_item_id=m.id AND lower(f.relative_path) LIKE '%.nfo')";
// Identity collision is a review signal, never an instruction to merge works.
const DUPLICATE: &str = "EXISTS(SELECT 1 FROM external_ids x JOIN external_ids y ON x.provider_id=y.provider_id AND x.namespace=y.namespace AND x.external_id=y.external_id WHERE x.media_item_id=m.id AND y.media_item_id<>m.id)";

#[derive(Clone)]
pub struct BrowseService {
    repo: LibraryRepository,
    cache_dir: PathBuf,
    approved_artwork_cache: Option<PathBuf>,
    adult_access: Option<crate::media_output::AdultOutputAccess>,
    render_posters: bool,
    tools: bool,
}

#[derive(Serialize, Deserialize)]
struct Cursor {
    scope: String,
    id: Uuid,
    text: String,
    number: i64,
}

impl BrowseService {
    pub fn new(repo: LibraryRepository, cache_dir: PathBuf) -> Self {
        Self {
            repo,
            cache_dir,
            approved_artwork_cache: None,
            adult_access: None,
            render_posters: true,
            tools: false,
        }
    }

    /// Explicit tools read scope includes unfinished scan/import candidates.
    /// It never grants adult access; typed callers must still supply that grant.
    pub fn for_tools(mut self) -> Self {
        self.tools = true;
        self
    }

    pub async fn tools_list_media(&self, input: &BrowseQuery) -> Result<MediaPageView> {
        self.clone().for_tools().list_media(input).await
    }

    pub async fn tools_media_detail(
        &self,
        id: Uuid,
        include_adult: bool,
    ) -> Result<MediaDetailView> {
        self.clone()
            .for_tools()
            .get_media(&MediaDetailInput {
                media_id: id,
                include_adult,
            })
            .await
    }

    fn completion_filter(&self, sql: &mut QueryBuilder<'_, Sqlite>) {
        if !self.tools {
            sql.push(" AND (")
                .push(crate::library_ready::ready_predicate())
                .push(")");
        }
    }

    /// Only trusted desktop composition may approve the network artwork cache.
    pub fn with_artwork_cache(mut self, path: PathBuf) -> Self {
        self.approved_artwork_cache = Some(path);
        self
    }

    pub fn with_adult_access(
        mut self,
        access: crate::media_output::AdultOutputAccess,
        covers: bool,
    ) -> Self {
        self.adult_access = Some(access);
        self.render_posters = covers;
        self
    }

    pub async fn list_roots(&self, include_adult: bool) -> Result<Vec<RootView>> {
        let mut roots = Vec::new();
        for root in self.repo.list_roots().await? {
            if !include_adult && root.kind == Some(MediaKind::AdultVideo) {
                continue;
            }
            roots.push(self.root_view(root).await?);
        }
        Ok(roots)
    }

    async fn root_view(&self, root: LibraryRoot) -> Result<RootView> {
        let settings = sqlx::query(
            "SELECT exclude_patterns_json,last_scan_at FROM root_settings WHERE root_id=?",
        )
        .bind(root.id.to_string())
        .fetch_optional(self.repo.pool())
        .await?;
        let (exclude_patterns, last_scan_at) = match settings {
            Some(row) => (
                serde_json::from_str(&get::<String>(&row, "exclude_patterns_json")?)
                    .map_err(|_| RepositoryError::CorruptData)?,
                get::<Option<i64>>(&row, "last_scan_at")?
                    .map(time)
                    .transpose()?,
            ),
            None => (Vec::new(), None),
        };
        Ok(RootView {
            root,
            exclude_patterns,
            last_scan_at,
        })
    }

    pub async fn save_root(&self, input: &RootInput) -> Result<RootView> {
        let _root_mutation = root_mutation_gate().lock().await;
        if input.name.trim().is_empty()
            || input.name.len() > 256
            || input.name.contains('\0')
            || input.path.contains('\0')
            || input.exclude_patterns.len() > 100
        {
            return Err(RepositoryError::InvalidInput);
        }
        for pattern in &input.exclude_patterns {
            if pattern.is_empty() || pattern.len() > 512 || Glob::new(pattern).is_err() {
                return Err(RepositoryError::InvalidInput);
            }
        }
        let path = std::fs::canonicalize(&input.path).map_err(|_| RepositoryError::InvalidInput)?;
        if !path.is_dir() {
            return Err(RepositoryError::InvalidInput);
        }
        let path = path
            .to_str()
            .ok_or(RepositoryError::InvalidInput)?
            .to_owned();
        let path_style = if cfg!(windows) {
            PathStyle::Windows
        } else {
            PathStyle::Posix
        };
        let path = strip_extended_windows_prefix(path);
        let normalized =
            normalize_root_path(&path, path_style).map_err(|_| RepositoryError::InvalidInput)?;
        for root in self.repo.list_roots().await? {
            if input.id == Some(root.id) {
                continue;
            }
            let other_adult = root.kind == Some(MediaKind::AdultVideo);
            let this_adult = input.kind == Some(MediaKind::AdultVideo);
            if other_adult != this_adult {
                let other = PathBuf::from(root.path);
                let current = PathBuf::from(&path);
                if current.starts_with(&other) || other.starts_with(&current) {
                    return Err(RepositoryError::ProtectedMetadata);
                }
            }
        }
        let id = input.id.unwrap_or_else(Uuid::new_v4);
        let now = Utc::now();
        let mut tx = self.repo.pool().begin().await?;
        let existing =
            sqlx::query("SELECT created_at,path,kind,path_style FROM library_roots WHERE id=?")
                .bind(id.to_string())
                .fetch_optional(&mut *tx)
                .await?;
        if input.id.is_some() && existing.is_none() {
            return Err(RepositoryError::NotFound);
        }
        if existing.is_some() {
            let active: i64 = sqlx::query_scalar("SELECT count(*) FROM scan_sessions s JOIN scrape_tasks t ON t.id=s.task_id WHERE s.root_id=? AND t.state IN ('queued','running')")
                .bind(id.to_string()).fetch_one(&mut *tx).await?;
            if active > 0 {
                return Err(RepositoryError::Conflict);
            }
        }
        let created = existing
            .as_ref()
            .map(|r| get::<i64>(r, "created_at"))
            .transpose()?
            .unwrap_or(now.timestamp_millis());
        // Preserve root identity semantics once files exist; root relocation keeps relative paths.
        if let Some(previous) = &existing {
            if get::<String>(previous, "path_style")? != path_style.as_str() {
                let files: i64 =
                    sqlx::query_scalar("SELECT count(*) FROM media_files WHERE root_id=?")
                        .bind(id.to_string())
                        .fetch_one(&mut *tx)
                        .await?;
                if files > 0 {
                    return Err(RepositoryError::Conflict);
                }
            }
        }
        if let Some(previous) = &existing {
            let was_adult =
                get::<Option<String>>(previous, "kind")?.as_deref() == Some("adult_video");
            if was_adult != (input.kind == Some(MediaKind::AdultVideo)) {
                let files: i64 =
                    sqlx::query_scalar("SELECT count(*) FROM media_files WHERE root_id=?")
                        .bind(id.to_string())
                        .fetch_one(&mut *tx)
                        .await?;
                if files > 0 {
                    return Err(RepositoryError::ProtectedMetadata);
                }
            }
        }
        let kind = input.kind.map(|v| v.as_str().to_owned());
        sqlx::query("INSERT INTO library_roots(id,name,path,normalized_path,path_style,kind,enabled,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,path=excluded.path,normalized_path=excluded.normalized_path,path_style=excluded.path_style,kind=excluded.kind,enabled=excluded.enabled,updated_at=excluded.updated_at")
            .bind(id.to_string()).bind(input.name.trim()).bind(&path).bind(normalized).bind(path_style.as_str()).bind(kind).bind(input.enabled).bind(created).bind(now.timestamp_millis())
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO root_settings(root_id,exclude_patterns_json) VALUES (?,?) ON CONFLICT(root_id) DO UPDATE SET exclude_patterns_json=excluded.exclude_patterns_json")
            .bind(id.to_string()).bind(serde_json::to_string(&input.exclude_patterns).map_err(|_|RepositoryError::InvalidInput)?)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        self.root_view(
            self.repo
                .get_root(id)
                .await?
                .ok_or(RepositoryError::NotFound)?,
        )
        .await
    }

    pub async fn remove_root(&self, id: Uuid) -> Result<bool> {
        let _root_mutation = root_mutation_gate().lock().await;
        let mut tx = self.repo.pool().begin().await?;
        let active: i64 = sqlx::query_scalar("SELECT count(*) FROM scan_sessions s JOIN scrape_tasks t ON t.id=s.task_id WHERE s.root_id=? AND t.state IN ('queued','running')")
            .bind(id.to_string()).fetch_one(&mut *tx).await?;
        if active > 0 {
            return Err(RepositoryError::Conflict);
        }
        let removed = sqlx::query("DELETE FROM library_roots WHERE id=?")
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?
            .rows_affected()
            > 0;
        tx.commit().await?;
        Ok(removed)
    }

    pub async fn list_media(&self, input: &BrowseQuery) -> Result<MediaPageView> {
        validate_query(input)?;
        let scope = format!(
            "{}:{}",
            if self.tools { "tools" } else { "library" },
            scope(input)?
        );
        let cursor = input
            .cursor
            .as_ref()
            .map(|text| {
                if text.len() > 8192 {
                    return Err(RepositoryError::InvalidInput);
                }
                let cursor: Cursor =
                    serde_json::from_str(text).map_err(|_| RepositoryError::InvalidInput)?;
                if cursor.scope != scope {
                    return Err(RepositoryError::InvalidInput);
                }
                Ok(cursor)
            })
            .transpose()?;
        let mut sql = QueryBuilder::new(select_media());
        filter(&mut sql, input)?;
        self.completion_filter(&mut sql);
        if let Some(cursor) = &cursor {
            match input.sort.as_str() {
                "title_asc" => {
                    sql.push(" AND (m.title COLLATE NOCASE > ")
                        .push_bind(cursor.text.clone())
                        .push(" COLLATE NOCASE OR (m.title COLLATE NOCASE = ")
                        .push_bind(cursor.text.clone())
                        .push(" COLLATE NOCASE AND m.id > ")
                        .push_bind(cursor.id.to_string())
                        .push("))");
                }
                _ => {
                    let field = sort_field(&input.sort);
                    sql.push(format!(" AND ({field} < "))
                        .push_bind(cursor.number)
                        .push(format!(" OR ({field} = "))
                        .push_bind(cursor.number)
                        .push(" AND m.id > ")
                        .push_bind(cursor.id.to_string())
                        .push("))");
                }
            }
        }
        sql.push(format!(
            " ORDER BY {} {} ,m.id ASC LIMIT ",
            sort_field(&input.sort),
            if input.sort == "title_asc" {
                "ASC"
            } else {
                "DESC"
            }
        ))
        .push_bind(i64::from(input.limit) + 1);
        let rows = sql.build().fetch_all(self.repo.pool()).await?;
        let more = rows.len() > input.limit as usize;
        let mut items = Vec::new();
        for row in rows.iter().take(input.limit as usize) {
            let media = media(row)?;
            let health = health(row)?;
            let poster_uri = self.poster(&media).await?;
            items.push(MediaCardView {
                media,
                health,
                poster_uri,
            });
        }
        let next_cursor = if more {
            items
                .last()
                .map(|last| {
                    serde_json::to_string(&Cursor {
                        scope,
                        id: last.media.id,
                        text: last.media.title.clone(),
                        number: match input.sort.as_str() {
                            "year_desc" => i64::from(last.media.year.unwrap_or(i32::MIN)),
                            "added_desc" => last.media.created_at.timestamp_millis(),
                            _ => last.media.updated_at.timestamp_millis(),
                        },
                    })
                    .map_err(|_| RepositoryError::CorruptData)
                })
                .transpose()?
        } else {
            None
        };
        Ok(MediaPageView { items, next_cursor })
    }

    pub async fn get_media(&self, input: &MediaDetailInput) -> Result<MediaDetailView> {
        let mut sql = QueryBuilder::new(select_media());
        filter(
            &mut sql,
            &BrowseQuery {
                include_adult: input.include_adult,
                ..Default::default()
            },
        )?;
        self.completion_filter(&mut sql);
        sql.push(" AND m.id=").push_bind(input.media_id.to_string());
        let row = sql
            .build()
            .fetch_optional(self.repo.pool())
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let media = media(&row)?;
        let mut relation_sql = QueryBuilder::new("SELECT m.id,m.kind,m.title FROM media_items m WHERE EXISTS(SELECT 1 FROM media_relations r WHERE (r.parent_id=");
        relation_sql
            .push_bind(input.media_id.to_string())
            .push(" AND r.child_id=m.id) OR (r.child_id=")
            .push_bind(input.media_id.to_string())
            .push(" AND r.parent_id=m.id))");
        visibility(&mut relation_sql, input.include_adult);
        self.completion_filter(&mut relation_sql);
        relation_sql.push(" ORDER BY m.title,m.id");
        let relation_rows = relation_sql.build().fetch_all(self.repo.pool()).await?;
        let relations = relation_rows
            .iter()
            .map(|row| {
                Ok(MediaRelationView {
                    id: uuid(row, "id")?,
                    kind: kind(row, "kind")?,
                    title: get(row, "title")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let files = self.detail_files(&media, input.include_adult).await?;
        Ok(MediaDetailView {
            poster_uri: self.poster(&media).await?,
            health: health(&row)?,
            media,
            files,
            artworks: self.repo.list_artworks(input.media_id).await?,
            external_ids: self.repo.list_external_ids(input.media_id).await?,
            metadata_values: self.repo.list_metadata_values(input.media_id).await?,
            relations,
        })
    }

    async fn detail_files(
        &self,
        item: &MediaItem,
        include_adult: bool,
    ) -> Result<Vec<mediaatlas_domain::MediaFile>> {
        let mut files = self.repo.list_files(item.id).await?;
        if matches!(item.kind, MediaKind::TvSeries | MediaKind::TvSeason) {
            let typed = crate::library_ready::typed_descendant("parent", "m");
            let mut sql = QueryBuilder::new(
                "SELECT m.id FROM media_items m JOIN media_items parent ON parent.id=",
            );
            sql.push_bind(item.id.to_string())
                .push(format!(" WHERE ({typed})"));
            visibility(&mut sql, include_adult);
            self.completion_filter(&mut sql);
            sql.push(" ORDER BY m.title,m.id");
            for row in sql.build().fetch_all(self.repo.pool()).await? {
                files.extend(self.repo.list_files(uuid(&row, "id")?).await?);
            }
        }
        files.sort_by(|a, b| {
            (&a.part, &a.relative_path, &a.id).cmp(&(&b.part, &b.relative_path, &b.id))
        });
        files.dedup_by_key(|f| f.id);
        Ok(files)
    }

    pub async fn summary(&self, input: &BrowseQuery) -> Result<LibrarySummaryView> {
        validate_query(input)?;
        let metadata = crate::library_ready::metadata_complete_predicate("m");
        let mut sql = QueryBuilder::new(format!("SELECT count(*) total,sum(NOT ({POSTER})) missing_poster,sum(NOT ({NFO})) missing_nfo,sum(NOT ({metadata})) incomplete_metadata FROM media_items m WHERE 1=1"));
        filter(&mut sql, input)?;
        self.completion_filter(&mut sql);
        let row = sql.build().fetch_one(self.repo.pool()).await?;
        let roots = self.list_roots(input.include_adult).await?;
        let total_roots = roots
            .iter()
            .filter(|r| input.root_id.is_none_or(|id| id == r.root.id))
            .count() as u64;
        let mut tasks = QueryBuilder::new("SELECT sum(t.state IN ('queued','running','waiting_for_user')) pending,sum(t.state IN ('failed','interrupted')) failed FROM scrape_tasks t JOIN scan_sessions s ON s.task_id=t.id JOIN library_roots r ON r.id=s.root_id WHERE 1=1");
        if !input.include_adult {
            tasks.push(" AND (r.kind IS NULL OR r.kind<>'adult_video') AND NOT EXISTS(SELECT 1 FROM scan_candidates ac WHERE ac.task_id=s.task_id AND ac.kind='adult_video')");
        }
        if let Some(root_id) = input.root_id {
            tasks.push(" AND r.id=").push_bind(root_id.to_string());
        }
        let taskrow = tasks.build().fetch_one(self.repo.pool()).await?;
        Ok(LibrarySummaryView {
            total_media: count(&row, "total")?,
            total_roots,
            pending_tasks: count(&taskrow, "pending")?,
            failed_tasks: count(&taskrow, "failed")?,
            missing_poster: count(&row, "missing_poster")?,
            missing_nfo: count(&row, "missing_nfo")?,
            incomplete_metadata: count(&row, "incomplete_metadata")?,
        })
    }

    pub async fn facets(&self, input: &BrowseQuery) -> Result<LibraryFacetsView> {
        validate_query(input)?;
        let mut input = input.clone();
        input.cursor = None;
        let genres = self.facet(&input, &["genre", "genres"]).await?;
        let people = self
            .facet(
                &input,
                &[
                    "actor",
                    "actors",
                    "performer",
                    "performers",
                    "artist",
                    "artists",
                    "people",
                    "director",
                    "directors",
                ],
            )
            .await?;
        let studios = self
            .facet(&input, &["studio", "studios", "label", "labels"])
            .await?;
        let mut years=QueryBuilder::new("SELECT CAST(m.year AS TEXT) value,count(DISTINCT m.id) count FROM media_items m WHERE m.year IS NOT NULL");
        filter(&mut years, &input)?;
        self.completion_filter(&mut years);
        years.push(" GROUP BY m.year ORDER BY m.year DESC");
        let yearrows = years.build().fetch_all(self.repo.pool()).await?;
        Ok(LibraryFacetsView {
            genres,
            people,
            studios,
            years: facet_rows(&yearrows)?,
        })
    }

    async fn facet(&self, input: &BrowseQuery, keys: &[&str]) -> Result<Vec<FacetOption>> {
        let mut sql=QueryBuilder::new("SELECT CAST(j.value AS TEXT) value,count(DISTINCT m.id) count FROM media_items m JOIN metadata_values v ON v.media_item_id=m.id JOIN json_each(v.value_json) j WHERE typeof(j.value)='text' AND v.field_key IN (");
        let mut separated = sql.separated(",");
        for key in keys {
            separated.push_bind(key.to_string());
        }
        separated.push_unseparated(")");
        filter(&mut sql, input)?;
        self.completion_filter(&mut sql);
        sql.push(" GROUP BY CAST(j.value AS TEXT) ORDER BY value COLLATE NOCASE LIMIT 500");
        facet_rows(&sql.build().fetch_all(self.repo.pool()).await?)
    }
}

fn strip_extended_windows_prefix(path: String) -> String {
    if cfg!(windows) {
        if let Some(path) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{path}");
        }
        if let Some(path) = path.strip_prefix(r"\\?\") {
            return path.to_owned();
        }
    }
    path
}

fn select_media() -> String {
    let metadata = crate::library_ready::metadata_complete_predicate("m");
    let refresh = refresh_status_predicate();
    format!("SELECT m.*,{POSTER} has_poster,{BACKDROP} has_backdrop,{NFO} has_nfo,{metadata} has_metadata,{DUPLICATE} duplicate,{refresh} refresh_status FROM media_items m WHERE 1=1")
}

fn refresh_status_predicate() -> String {
    let ancestor = crate::library_ready::typed_descendant("refresh_parent", "m");
    let parents = crate::library_ready::ancestor_ids("m");
    // A late cancellation/failure from an older worker must not replace the
    // outcome of the newer refresh. Row insertion order breaks same-ms ties.
    format!("(SELECT CASE refresh.state WHEN 'pending' THEN 'metadata_pending' WHEN 'failed' THEN CASE WHEN refresh.reason='artwork_failed' THEN 'metadata_artwork_failed' ELSE 'metadata_failed' END WHEN 'needs_review' THEN CASE WHEN refresh.reason='provider_unconfigured' THEN 'metadata_unconfigured' ELSE 'metadata_needs_review' END ELSE NULL END FROM library_refresh_items refresh JOIN scrape_tasks refresh_task ON refresh_task.id=refresh.task_id WHERE refresh.media_item_id IN (SELECT m.id UNION SELECT refresh_parent.id FROM media_items refresh_parent WHERE refresh_parent.id IN ({parents}) AND ({ancestor})) ORDER BY refresh_task.created_at DESC,refresh_task.rowid DESC LIMIT 1)")
}

fn visibility(sql: &mut QueryBuilder<'_, Sqlite>, include_adult: bool) {
    if !include_adult {
        sql.push(" AND (")
            .push(crate::library_ready::ancestor_isolation("m"))
            .push(")");
        sql.push(" AND m.kind<>'adult_video' AND m.adult_isolated=0 AND NOT EXISTS(SELECT 1 FROM media_files vf JOIN library_roots vr ON vr.id=vf.root_id WHERE vf.media_item_id=m.id AND vr.kind='adult_video') AND NOT EXISTS(SELECT 1 FROM media_relations vr JOIN media_items vm ON vm.id=vr.child_id WHERE vr.parent_id=m.id AND (vm.kind='adult_video' OR vm.adult_isolated=1)) AND NOT EXISTS(SELECT 1 FROM media_relations pr JOIN media_files f ON f.media_item_id=pr.child_id JOIN library_roots r ON r.id=f.root_id WHERE pr.parent_id=m.id AND r.kind='adult_video') AND NOT EXISTS(SELECT 1 FROM media_relations pr JOIN media_relations cr ON cr.parent_id=pr.child_id JOIN media_items vm ON vm.id=cr.child_id WHERE pr.parent_id=m.id AND (vm.kind='adult_video' OR vm.adult_isolated=1)) AND NOT EXISTS(SELECT 1 FROM media_relations pr JOIN media_relations cr ON cr.parent_id=pr.child_id JOIN media_files f ON f.media_item_id=cr.child_id JOIN library_roots r ON r.id=f.root_id WHERE pr.parent_id=m.id AND r.kind='adult_video')");
    }
}

fn filter(sql: &mut QueryBuilder<'_, Sqlite>, input: &BrowseQuery) -> Result<()> {
    visibility(sql, input.include_adult);
    if let Some(kind) = input.kind {
        sql.push(" AND m.kind=").push_bind(kind.as_str().to_owned());
    }
    if let Some(root) = input.root_id {
        let typed = crate::library_ready::typed_descendant("m", "descendant");
        sql.push(" AND (EXISTS(SELECT 1 FROM media_files f WHERE f.media_item_id=m.id AND f.root_id=").push_bind(root.to_string()).push(format!(") OR EXISTS(SELECT 1 FROM media_items descendant JOIN media_files f ON f.media_item_id=descendant.id WHERE ({typed}) AND f.root_id=")).push_bind(root.to_string()).push("))");
    }
    if let Some(text) = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let pattern = format!(
            "%{}%",
            text.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        sql.push(" AND (m.title LIKE ")
            .push_bind(pattern.clone())
            .push(" ESCAPE '\\' OR m.original_title LIKE ")
            .push_bind(pattern)
            .push(" ESCAPE '\\')");
    }
    if let Some(year) = input.year {
        sql.push(" AND m.year=").push_bind(year);
    }
    for (value, keys) in [
        (&input.genre, &["genre", "genres"][..]),
        (
            &input.person,
            &[
                "actor",
                "actors",
                "performer",
                "performers",
                "artist",
                "artists",
                "people",
                "director",
                "directors",
            ][..],
        ),
        (&input.studio, &["studio", "studios", "label", "labels"][..]),
    ] {
        if let Some(value) = value {
            sql.push(" AND EXISTS(SELECT 1 FROM metadata_values v JOIN json_each(v.value_json) j WHERE v.media_item_id=m.id AND v.field_key IN (");
            let mut separated = sql.separated(",");
            for key in keys {
                separated.push_bind((*key).to_owned());
            }
            separated.push_unseparated(") AND CAST(j.value AS TEXT)=");
            sql.push_bind(value.clone()).push(")");
        }
    }
    if let Some(health) = input.health.as_deref() {
        let predicate = match health {
            "missing_poster" => format!("NOT ({POSTER})"),
            "missing_backdrop" => format!("NOT ({BACKDROP})"),
            "missing_nfo" => format!("NOT ({NFO})"),
            "incomplete_metadata" => format!(
                "NOT ({})",
                crate::library_ready::metadata_complete_predicate("m")
            ),
            "metadata_pending" => format!("{}='metadata_pending'", refresh_status_predicate()),
            "metadata_failed" => format!(
                "({0}='metadata_artwork_failed' OR (NOT ({1}) AND {0}='metadata_failed'))",
                refresh_status_predicate(),
                crate::library_ready::metadata_complete_predicate("m")
            ),
            "metadata_needs_review" | "metadata_unconfigured" => format!(
                "NOT ({}) AND {}='{}'",
                crate::library_ready::metadata_complete_predicate("m"),
                refresh_status_predicate(),
                health
            ),
            "duplicate" => DUPLICATE.to_owned(),
            _ => return Err(RepositoryError::InvalidInput),
        };
        sql.push(" AND (").push(predicate).push(")");
    }
    Ok(())
}

fn validate_query(input: &BrowseQuery) -> Result<()> {
    if !(1..=200).contains(&input.limit)
        || !matches!(
            input.sort.as_str(),
            "title_asc" | "year_desc" | "added_desc" | "updated_desc"
        )
    {
        return Err(RepositoryError::InvalidInput);
    }
    for value in [&input.query, &input.genre, &input.person, &input.studio] {
        if value
            .as_ref()
            .is_some_and(|s| s.len() > 1024 || s.contains('\0'))
        {
            return Err(RepositoryError::InvalidInput);
        }
    }
    Ok(())
}

fn scope(input: &BrowseQuery) -> Result<String> {
    let mut copy = input.clone();
    copy.cursor = None;
    copy.limit = 0;
    serde_json::to_string(&copy).map_err(|_| RepositoryError::InvalidInput)
}
fn sort_field(sort: &str) -> &str {
    match sort {
        "title_asc" => "m.title COLLATE NOCASE",
        "year_desc" => "COALESCE(m.year,-2147483648)",
        "added_desc" => "m.created_at",
        _ => "m.updated_at",
    }
}
fn get<'r, T>(row: &'r SqliteRow, key: &str) -> Result<T>
where
    T: sqlx::Decode<'r, Sqlite> + sqlx::Type<Sqlite>,
{
    row.try_get(key).map_err(|_| RepositoryError::CorruptData)
}
fn uuid(row: &SqliteRow, key: &str) -> Result<Uuid> {
    Uuid::parse_str(&get::<String>(row, key)?).map_err(|_| RepositoryError::CorruptData)
}
fn kind(row: &SqliteRow, key: &str) -> Result<MediaKind> {
    serde_json::from_value(serde_json::Value::String(get(row, key)?))
        .map_err(|_| RepositoryError::CorruptData)
}
fn time(millis: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp_millis(millis).ok_or(RepositoryError::CorruptData)
}
fn media(row: &SqliteRow) -> Result<MediaItem> {
    Ok(MediaItem {
        id: uuid(row, "id")?,
        kind: kind(row, "kind")?,
        title: get(row, "title")?,
        original_title: get(row, "original_title")?,
        year: get(row, "year")?,
        created_at: time(get(row, "created_at")?)?,
        updated_at: time(get(row, "updated_at")?)?,
    })
}
fn health(row: &SqliteRow) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (key, name) in [
        ("has_poster", "missing_poster"),
        ("has_backdrop", "missing_backdrop"),
        ("has_nfo", "missing_nfo"),
        ("has_metadata", "incomplete_metadata"),
    ] {
        if !get::<bool>(row, key)? {
            out.push(name.into());
        }
    }
    if get::<bool>(row, "duplicate")? {
        out.push("duplicate".into());
    }
    if let Some(state) = get::<Option<String>>(row, "refresh_status")? {
        // An earlier failed automatic attempt must not label a subsequently
        // corrected item as unresolved. A currently running refresh is real work.
        if state == "metadata_artwork_failed" {
            out.push("metadata_failed".into());
        } else if state == "metadata_pending" || !get::<bool>(row, "has_metadata")? {
            out.push(state);
        }
    }
    Ok(out)
}
fn count(row: &SqliteRow, key: &str) -> Result<u64> {
    Ok(get::<Option<i64>>(row, key)?.unwrap_or(0).max(0) as u64)
}
fn facet_rows(rows: &[SqliteRow]) -> Result<Vec<FacetOption>> {
    rows.iter()
        .map(|row| {
            Ok(FacetOption {
                value: get(row, "value")?,
                count: count(row, "count")?,
            })
        })
        .collect()
}
