//! Manual review ledger. Reads may reconcile a proven result, never replay a merge.
use crate::library::{LibraryRepository, RepositoryError};
use chrono::{DateTime, Utc};
use mediaatlas_application::{library::BrowseQuery, review_queue::*};
use mediaatlas_domain::MediaKind;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqliteConnection};
use std::collections::HashSet;
use uuid::Uuid;

type Result<T> = std::result::Result<T, RepositoryError>;
const ITEM_TTL: i64 = 7 * 24 * 60 * 60 * 1000;
const CHECKPOINT_TTL: i64 = 30 * 60 * 1000;

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct ReviewQueueService {
    repo: LibraryRepository,
}
impl ReviewQueueService {
    pub fn new(repo: LibraryRepository) -> Self {
        Self { repo }
    }

    pub async fn create(&self, input: ReviewQueueCreate) -> Result<ReviewQueueView> {
        if input.media_ids.is_empty() || input.media_ids.len() > 32 {
            return Err(RepositoryError::InvalidInput);
        }
        validate_context(&input.context)?;
        let mut seen = HashSet::new();
        let ids: Vec<_> = input
            .media_ids
            .into_iter()
            .filter(|id| seen.insert(*id))
            .collect();
        let mut tx = self.repo.pool().begin().await?;
        // Acquire the SQLite writer before qualification, so privacy/root changes cannot race it.
        sqlx::query("UPDATE review_queues SET revision=revision WHERE 0")
            .execute(&mut *tx)
            .await?;
        if let Some(root) = input.context.root {
            let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM library_roots WHERE id=? AND enabled=1 AND (kind IS NULL OR kind<>'adult_video'))").bind(root.to_string()).fetch_one(&mut *tx).await?;
            if !allowed {
                return Err(RepositoryError::InvalidInput);
            }
        }
        for id in &ids {
            if qualification(&mut tx, *id, input.context.root)
                .await?
                .is_some()
            {
                return Err(RepositoryError::InvalidInput);
            }
        }
        let id = Uuid::new_v4();
        let now = Utc::now().timestamp_millis();
        sqlx::query("INSERT INTO review_queues(id,current_media_id,context_json,created_at,updated_at,preview_watermark) VALUES(?,?,?,?,?,(SELECT COALESCE(max(rowid),0) FROM video_merge_previews))")
            .bind(id.to_string()).bind(ids[0].to_string()).bind(json(&input.context)?).bind(now).bind(now).execute(&mut *tx).await?;
        for (position, media) in ids.iter().enumerate() {
            let scope = scope_hash(&mut tx, *media).await?;
            sqlx::query("INSERT INTO review_queue_items(queue_id,media_id,position,state,checked_at,scope_hash) VALUES(?,?,?,'pending',?,?)")
                .bind(id.to_string()).bind(media.to_string()).bind(position as i64).bind(now).bind(scope).execute(&mut *tx).await?;
        }
        let view = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(view)
    }
    pub async fn get(&self, id: Uuid) -> Result<ReviewQueueView> {
        let mut tx = self.repo.pool().begin().await?;
        let view = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(view)
    }
    pub async fn list(&self) -> Result<Vec<ReviewQueueView>> {
        let mut tx = self.repo.pool().begin().await?;
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM review_queues ORDER BY updated_at DESC,rowid DESC LIMIT 20",
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut out = vec![];
        for id in ids {
            out.push(load(&mut tx, uuid(&id)?).await?);
        }
        tx.commit().await?;
        Ok(out)
    }
    pub async fn checkpoint(&self, input: ReviewQueueCheckpoint) -> Result<ReviewQueueView> {
        let mut tx = self.repo.pool().begin().await?;
        lock_revision(&mut tx, input.queue_id, input.expected_revision).await?;
        let view = load(&mut tx, input.queue_id).await?;
        let item = view
            .items
            .iter()
            .find(|i| i.media_id == input.media_id)
            .ok_or(RepositoryError::NotFound)?;
        if view.current_media_id != Some(input.media_id)
            || item.state != ReviewState::Pending
            || item.needs_recheck
        {
            return Err(RepositoryError::Conflict);
        }
        let now = Utc::now().timestamp_millis();
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM video_merge_previews p JOIN review_queues q ON q.id=? WHERE p.id=? AND p.media_item_id=? AND p.state='planned' AND p.created_at>=q.created_at AND (p.created_at>q.created_at OR p.rowid>q.preview_watermark) AND p.created_at>=?)")
            .bind(input.queue_id.to_string()).bind(input.preview_id.to_string()).bind(input.media_id.to_string()).bind(now-CHECKPOINT_TTL).fetch_one(&mut *tx).await?;
        if !valid {
            return Err(RepositoryError::Conflict);
        }
        let checkpoint = Uuid::new_v4();
        let scope = scope_hash(&mut tx, input.media_id).await?;
        sqlx::query("INSERT INTO review_queue_checkpoints(preview_id,queue_id,media_id,checkpoint,revision,scope_hash,created_at) VALUES(?,?,?,?,?,?,?)")
            .bind(input.preview_id.to_string()).bind(input.queue_id.to_string()).bind(input.media_id.to_string()).bind(checkpoint.to_string()).bind((input.expected_revision+1) as i64).bind(&scope).bind(now).execute(&mut *tx).await?;
        sqlx::query("UPDATE review_queue_items SET checkpoint=?,preview_id=?,checkpoint_at=?,checked_at=?,scope_hash=? WHERE queue_id=? AND media_id=?")
            .bind(checkpoint.to_string()).bind(input.preview_id.to_string()).bind(now).bind(now).bind(scope).bind(input.queue_id.to_string()).bind(input.media_id.to_string()).execute(&mut *tx).await?;
        // A prior successful merge may have been reconciled read-only after a crash.
        // Persist this explicitly selected next item before its final merge guard runs.
        sqlx::query("UPDATE review_queues SET current_media_id=? WHERE id=?")
            .bind(input.media_id.to_string())
            .bind(input.queue_id.to_string())
            .execute(&mut *tx)
            .await?;
        bump(&mut tx, input.queue_id).await?;
        let view = load(&mut tx, input.queue_id).await?;
        tx.commit().await?;
        Ok(view)
    }
    pub async fn update(&self, input: ReviewQueueUpdate) -> Result<ReviewQueueView> {
        let mut tx = self.repo.pool().begin().await?;
        lock_revision(&mut tx, input.queue_id, input.expected_revision).await?;
        let view = load(&mut tx, input.queue_id).await?;
        let item = view
            .items
            .iter()
            .find(|i| i.media_id == input.media_id)
            .ok_or(RepositoryError::NotFound)?;
        let now = Utc::now().timestamp_millis();
        match input.action {
            ReviewAction::Select | ReviewAction::Retry => {
                if item.state == ReviewState::Confirmed
                    || (input.action == ReviewAction::Retry
                        && !matches!(item.state, ReviewState::Deferred | ReviewState::Failed))
                    || qualification(&mut tx, input.media_id, view.context.root)
                        .await?
                        .is_some()
                {
                    return Err(RepositoryError::Conflict);
                }
                let scope = scope_hash(&mut tx, input.media_id).await?;
                sqlx::query("UPDATE review_queue_items SET state='pending',checkpoint=NULL,preview_id=NULL,checkpoint_at=NULL,checked_at=?,scope_hash=? WHERE queue_id=? AND media_id=?").bind(now).bind(scope).bind(input.queue_id.to_string()).bind(input.media_id.to_string()).execute(&mut *tx).await?;
                sqlx::query("UPDATE review_queues SET current_media_id=? WHERE id=?")
                    .bind(input.media_id.to_string())
                    .bind(input.queue_id.to_string())
                    .execute(&mut *tx)
                    .await?;
            }
            ReviewAction::Confirmed => {
                // A succeeded bound preview can be recovered after a crash, but never replayed here.
                if item.needs_recheck
                    || input.checkpoint.is_none()
                    || input.checkpoint != item.checkpoint
                    || !proven(&mut tx, input.queue_id, input.media_id).await?
                {
                    return Err(RepositoryError::Conflict);
                }
                if view.current_media_id != Some(input.media_id)
                    && item.state != ReviewState::Confirmed
                {
                    return Err(RepositoryError::Conflict);
                }
                sqlx::query("UPDATE review_queue_items SET state='confirmed' WHERE queue_id=? AND media_id=?").bind(input.queue_id.to_string()).bind(input.media_id.to_string()).execute(&mut *tx).await?;
                advance(&mut tx, &view, input.media_id).await?;
            }
            ReviewAction::Defer | ReviewAction::Failed => {
                if view.current_media_id != Some(input.media_id)
                    || item.state != ReviewState::Pending
                {
                    return Err(RepositoryError::Conflict);
                }
                let state = if input.action == ReviewAction::Defer {
                    "deferred"
                } else {
                    "failed"
                };
                sqlx::query("UPDATE review_queue_items SET state=?,checkpoint=NULL,preview_id=NULL,checkpoint_at=NULL WHERE queue_id=? AND media_id=?").bind(state).bind(input.queue_id.to_string()).bind(input.media_id.to_string()).execute(&mut *tx).await?;
                advance(&mut tx, &view, input.media_id).await?;
            }
        }
        bump(&mut tx, input.queue_id).await?;
        let view = load(&mut tx, input.queue_id).await?;
        tx.commit().await?;
        Ok(view)
    }
}

async fn qualification(
    conn: &mut SqliteConnection,
    id: Uuid,
    root: Option<Uuid>,
) -> Result<Option<&'static str>> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM media_items WHERE id=?)")
        .bind(id.to_string())
        .fetch_one(&mut *conn)
        .await?;
    if !exists {
        return Ok(Some("missing_media"));
    }
    let unsafe_sql=format!("{} SELECT EXISTS(SELECT 1 FROM media_items m LEFT JOIN media_files f ON f.media_item_id=m.id LEFT JOIN library_roots r ON r.id=f.root_id WHERE m.id IN(SELECT id FROM related) AND (m.kind='adult_video' OR m.adult_isolated=1 OR r.kind='adult_video' OR r.enabled=0))",related_cte());
    let unsafe_related: bool = sqlx::query_scalar(&unsafe_sql)
        .bind(id.to_string())
        .fetch_one(&mut *conn)
        .await?;
    if unsafe_related {
        return Ok(Some("unavailable_media"));
    }
    let ancestors = crate::library_ready::ancestor_isolation("m");
    let typed = crate::library_ready::typed_descendant("m", "child");
    let ready = crate::library_ready::ready_predicate();
    let sql=format!("SELECT EXISTS(SELECT 1 FROM media_items m WHERE m.id=? AND m.kind IN ('movie','tv_series','anime') AND m.adult_isolated=0 AND ({ancestors}) AND NOT EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=m.id AND (r.kind='adult_video' OR r.enabled=0)) AND ({ready}) AND (? IS NULL OR EXISTS(SELECT 1 FROM media_files f WHERE f.root_id=? AND (f.media_item_id=m.id OR EXISTS(SELECT 1 FROM media_items child WHERE child.id=f.media_item_id AND ({typed}))))))");
    let root = root.map(|id| id.to_string());
    let allowed: bool = sqlx::query_scalar(&sql)
        .bind(id.to_string())
        .bind(&root)
        .bind(&root)
        .fetch_one(conn)
        .await?;
    Ok(if allowed {
        None
    } else {
        Some("unavailable_media")
    })
}
async fn proven(conn: &mut SqliteConnection, queue: Uuid, media: Uuid) -> Result<bool> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM review_queue_items i JOIN review_queues q ON q.id=i.queue_id JOIN review_queue_checkpoints c ON c.preview_id=i.preview_id AND c.queue_id=i.queue_id AND c.media_id=i.media_id AND c.checkpoint=i.checkpoint JOIN video_merge_previews p ON p.id=i.preview_id WHERE i.queue_id=? AND i.media_id=? AND p.media_item_id=i.media_id AND p.state='succeeded' AND p.created_at>=q.created_at)").bind(queue.to_string()).bind(media.to_string()).fetch_one(conn).await?)
}
async fn load(conn: &mut SqliteConnection, id: Uuid) -> Result<ReviewQueueView> {
    let row = sqlx::query("SELECT * FROM review_queues WHERE id=?")
        .bind(id.to_string())
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(RepositoryError::NotFound)?;
    let mut context: ReviewContext =
        serde_json::from_str(&row.try_get::<String, _>("context_json")?)
            .map_err(|_| RepositoryError::CorruptData)?;
    let rows = sqlx::query("SELECT * FROM review_queue_items WHERE queue_id=? ORDER BY position")
        .bind(id.to_string())
        .fetch_all(&mut *conn)
        .await?;
    let now = Utc::now().timestamp_millis();
    let mut items = vec![];
    for row in rows {
        let media = uuid(&row.try_get::<String, _>("media_id")?)?;
        let mut state: ReviewState =
            serde_json::from_value(serde_json::Value::String(row.try_get("state")?))
                .map_err(|_| RepositoryError::CorruptData)?;
        let checkpoint_at: Option<i64> = row.try_get("checkpoint_at")?;
        let qualified = qualification(conn, media, context.root).await?;
        // Derived from the trusted confirmation record, not client state. No writes on resume.
        if qualified.is_none() && proven(conn, id, media).await? {
            state = ReviewState::Confirmed;
        }
        let changed = row.try_get::<String, _>("scope_hash")? != scope_hash(conn, media).await?;
        let reason = qualified.or_else(|| {
            if changed {
                return Some("expired_item");
            }
            if state == ReviewState::Confirmed {
                None
            } else if checkpoint_at.is_some_and(|time| time < now - CHECKPOINT_TTL) {
                Some("stale_checkpoint")
            } else if row.try_get::<i64, _>("checked_at").unwrap_or(0) < now - ITEM_TTL {
                Some("expired_item")
            } else {
                None
            }
        });
        items.push(ReviewQueueItem {
            media_id: media,
            position: row.try_get::<i64, _>("position")? as u32,
            state,
            checkpoint: optional_uuid(row.try_get("checkpoint")?)?,
            preview_id: optional_uuid(row.try_get("preview_id")?)?,
            needs_recheck: reason.is_some(),
            reason: reason.map(str::to_owned),
        });
    }
    let current = optional_uuid(row.try_get("current_media_id")?)?;
    // Search text and opaque cursors may contain titles. Reclassified or deleted
    // media must not expose that historical text through an ordinary queue read.
    if items.iter().any(|item| {
        matches!(
            item.reason.as_deref(),
            Some("missing_media" | "unavailable_media")
        )
    }) {
        context.q = None;
        context.cursor = None;
        context.page = None;
    }
    let current = current
        .filter(|id| {
            items
                .iter()
                .any(|i| i.media_id == *id && i.state == ReviewState::Pending)
        })
        .or_else(|| {
            items
                .iter()
                .find(|i| i.state == ReviewState::Pending)
                .map(|i| i.media_id)
        });
    Ok(ReviewQueueView {
        id,
        revision: row.try_get::<i64, _>("revision")? as u64,
        current_media_id: current,
        context,
        items,
        created_at: time(row.try_get("created_at")?)?,
        updated_at: time(row.try_get("updated_at")?)?,
    })
}
async fn lock_revision(conn: &mut SqliteConnection, id: Uuid, expected: u64) -> Result<()> {
    let expected = i64::try_from(expected).map_err(|_| RepositoryError::InvalidInput)?;
    if sqlx::query("UPDATE review_queues SET revision=revision WHERE id=? AND revision=?")
        .bind(id.to_string())
        .bind(expected)
        .execute(conn)
        .await?
        .rows_affected()
        != 1
    {
        return Err(RepositoryError::Conflict);
    }
    Ok(())
}
async fn bump(conn: &mut SqliteConnection, id: Uuid) -> Result<()> {
    sqlx::query("UPDATE review_queues SET revision=revision+1,updated_at=? WHERE id=?")
        .bind(Utc::now().timestamp_millis())
        .bind(id.to_string())
        .execute(conn)
        .await?;
    Ok(())
}
async fn advance(conn: &mut SqliteConnection, view: &ReviewQueueView, except: Uuid) -> Result<()> {
    let next = view
        .items
        .iter()
        .find(|i| i.media_id != except && i.state == ReviewState::Pending)
        .map(|i| i.media_id.to_string());
    sqlx::query("UPDATE review_queues SET current_media_id=? WHERE id=?")
        .bind(next)
        .bind(view.id.to_string())
        .execute(conn)
        .await?;
    Ok(())
}
fn uuid(text: &str) -> Result<Uuid> {
    Uuid::parse_str(text).map_err(|_| RepositoryError::CorruptData)
}
fn optional_uuid(value: Option<String>) -> Result<Option<Uuid>> {
    value.map(|v| uuid(&v)).transpose()
}
fn time(value: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp_millis(value).ok_or(RepositoryError::CorruptData)
}
fn json(value: &impl serde::Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(|_| RepositoryError::InvalidInput)
}

fn validate_context(c: &ReviewContext) -> Result<()> {
    if c.kind
        .is_some_and(|k| !matches!(k, MediaKind::Movie | MediaKind::TvSeries | MediaKind::Anime))
        || c.health.as_deref().is_some_and(|v| {
            !matches!(
                v,
                "missing_poster"
                    | "missing_nfo"
                    | "missing_backdrop"
                    | "incomplete_metadata"
                    | "metadata_pending"
                    | "metadata_needs_review"
                    | "metadata_failed"
                    | "metadata_unconfigured"
                    | "duplicate"
            )
        })
        || c.sort.as_deref().is_some_and(|v| {
            !matches!(v, "title_asc" | "year_desc" | "added_desc" | "updated_desc")
        })
        || c.page.is_some_and(|v| !(1..=1_000_000).contains(&v))
    {
        return Err(RepositoryError::InvalidInput);
    }
    for text in [&c.q].into_iter().flatten() {
        if text.is_empty() {
            return Err(RepositoryError::InvalidInput);
        }
        safe_text(text)?;
    }
    if let Some(cursor) = &c.cursor {
        if cursor.len() > 2048 {
            return Err(RepositoryError::InvalidInput);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Cursor {
            scope: String,
            id: Uuid,
            text: String,
            number: i64,
        }
        let parsed: Cursor =
            serde_json::from_str(cursor).map_err(|_| RepositoryError::InvalidInput)?;
        safe_text(&parsed.text)?;
        let query = BrowseQuery {
            root_id: c.root,
            kind: c.kind,
            query: c.q.clone(),
            health: c.health.clone(),
            genre: None,
            person: None,
            studio: None,
            year: None,
            sort: c.sort.clone().unwrap_or_else(|| "updated_desc".into()),
            cursor: None,
            limit: 0,
            include_adult: false,
        };
        if parsed.scope != format!("tools:{}", json(&query)?) {
            return Err(RepositoryError::InvalidInput);
        }
        let _ = (parsed.id, parsed.number);
    }
    Ok(())
}
fn safe_text(text: &str) -> Result<()> {
    let lower = text.to_ascii_lowercase();
    if text.len() > 256
        || text.chars().any(char::is_control)
        || text.contains("://")
        || lower.starts_with("bearer ")
        || lower.contains("token=")
        || lower.contains("api_key=")
    {
        Err(RepositoryError::InvalidInput)
    } else {
        Ok(())
    }
}

// Bounded typed descendants plus all two-level ancestors. Never broad arbitrary descendants.
fn related_cte() -> &'static str {
    "WITH target(id) AS(SELECT ?),related(id) AS(SELECT id FROM target UNION SELECT child_id FROM media_relations WHERE parent_id IN target AND relation_type IN('episode','season') UNION SELECT er.child_id FROM media_relations sr JOIN media_relations er ON er.parent_id=sr.child_id AND er.relation_type='episode' WHERE sr.parent_id IN target AND sr.relation_type='season' UNION SELECT parent_id FROM media_relations WHERE child_id IN target UNION SELECT pr.parent_id FROM media_relations cr JOIN media_relations pr ON pr.child_id=cr.parent_id WHERE cr.child_id IN target)"
}
async fn scope_hash(conn: &mut SqliteConnection, id: Uuid) -> Result<String> {
    let sql=format!("{} SELECT json_array(m.id,m.kind,m.adult_isolated,f.id,f.root_id,f.relative_path,f.size_bytes,f.modified_at,r.path,r.path_style,r.kind,r.enabled,r.updated_at,COALESCE(s.exclude_patterns_json,'[]')) FROM media_items m LEFT JOIN media_files f ON f.media_item_id=m.id LEFT JOIN library_roots r ON r.id=f.root_id LEFT JOIN root_settings s ON s.root_id=r.id WHERE m.id IN(SELECT id FROM related) ORDER BY m.id,f.id",related_cte());
    let rows: Vec<String> = sqlx::query_scalar(&sql)
        .bind(id.to_string())
        .fetch_all(conn)
        .await?;
    Ok(format!("{:x}", Sha256::digest(json(&rows)?.as_bytes())))
}

/// Called in ScraperService's final writer transaction for queue-bound previews only.
/// Old/superseded bindings remain recorded, so clearing an item cannot bypass this guard.
pub(crate) async fn validate_bound_preview(
    conn: &mut SqliteConnection,
    preview: Uuid,
) -> Result<()> {
    // Acquire the writer before any snapshot read. Root/privacy changes then
    // serialize with the actual metadata merge, rather than failing a read upgrade.
    sqlx::query("UPDATE review_queues SET revision=revision WHERE id IN(SELECT queue_id FROM review_queue_checkpoints WHERE preview_id=?)")
        .bind(preview.to_string()).execute(&mut *conn).await?;
    let row=sqlx::query("SELECT c.*,q.revision AS current_revision,q.current_media_id,q.context_json,i.preview_id AS current_preview,i.checkpoint AS current_checkpoint,i.state AS item_state FROM review_queue_checkpoints c JOIN review_queues q ON q.id=c.queue_id JOIN review_queue_items i ON i.queue_id=c.queue_id AND i.media_id=c.media_id WHERE c.preview_id=?").bind(preview.to_string()).fetch_optional(&mut *conn).await?;
    let Some(row) = row else {
        return Ok(());
    };
    let media = uuid(&row.try_get::<String, _>("media_id")?)?;
    let context: ReviewContext = serde_json::from_str(&row.try_get::<String, _>("context_json")?)
        .map_err(|_| RepositoryError::CorruptData)?;
    if row.try_get::<i64, _>("revision")? != row.try_get::<i64, _>("current_revision")?
        || row.try_get::<Option<String>, _>("current_media_id")? != Some(media.to_string())
        || row.try_get::<Option<String>, _>("current_preview")? != Some(preview.to_string())
        || row.try_get::<Option<String>, _>("current_checkpoint")?
            != Some(row.try_get("checkpoint")?)
        || row.try_get::<String, _>("item_state")? != "pending"
        || row.try_get::<i64, _>("created_at")? < Utc::now().timestamp_millis() - CHECKPOINT_TTL
        || qualification(conn, media, context.root).await?.is_some()
        || row.try_get::<String, _>("scope_hash")? != scope_hash(conn, media).await?
    {
        return Err(RepositoryError::Conflict);
    }
    Ok(())
}
