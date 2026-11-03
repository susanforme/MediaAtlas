//! Explicit, byte-backed audio tag edits. No source is modified during Preview.
//!
//! Publication first quarantines the actual source, verifies the moved object,
//! then atomically hard-links a staged immutable payload without overwriting an
//! intervening source. Every operation retains both original SQLite bytes and a
//! named on-disk quarantine. Recovery is always another explicit Preview/Execute.
use crate::{
    library::{LibraryRepository, RepositoryError},
    music_library::{ensure_music_file, ensure_music_file_tx},
    network::CancellationToken,
};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, File, OpenOptions},
};
use chrono::Utc;
use lofty::{
    config::{ParseOptions, ParsingMode, WriteOptions},
    file::{FileType, TaggedFile, TaggedFileExt},
    prelude::{ItemKey, TagExt},
    probe::Probe,
    tag::Tag,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{
    collections::{BTreeMap, HashMap},
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

/// Full byte snapshots are intentionally bounded, including original recovery
/// bytes. Larger files require a future streaming journal implementation.
const MAX_AUDIO: usize = 256 * 1024 * 1024;
const MAX_PREVIEWS: usize = 32;
const PREVIEW_TTL: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicTagRead {
    pub file_id: Uuid,
    pub tags: BTreeMap<String, String>,
    pub format: String,
    pub writable: bool,
    pub picture_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicTagChange {
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicTagPreview {
    pub id: Uuid,
    pub file_id: Uuid,
    pub changes: Vec<MusicTagChange>,
    pub kind: String,
    pub state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicTagResult {
    pub operation_id: Uuid,
    pub file_id: Uuid,
    pub state: String,
    pub backup_relative: Option<String>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Fingerprint {
    len: u64,
    modified_nanos: u128,
    created_nanos: Option<u128>,
    identity: String,
    hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Plan {
    preview: MusicTagPreview,
    root: PathBuf,
    root_id: Uuid,
    media_item_id: Uuid,
    relative: String,
    before: Option<Fingerprint>,
    payload_hash: String,
    backup_directory: String,
}
struct Lease {
    _ancestors: Vec<Dir>,
    parent: Dir,
    // On Windows this handle denies FILE_SHARE_DELETE until execution moves
    // the source. Retained directory leases still prevent ancestor replacement.
    source: std::sync::Mutex<Option<File>>,
    created: Instant,
}
fn leases() -> &'static Mutex<HashMap<Uuid, Arc<Lease>>> {
    static LEASES: OnceLock<Mutex<HashMap<Uuid, Arc<Lease>>>> = OnceLock::new();
    LEASES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn audio_budget() -> &'static Semaphore {
    static BUDGET: Semaphore = Semaphore::const_new(2);
    &BUDGET
}
fn mutation_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
#[derive(Clone)]
pub struct MusicTagService {
    repository: LibraryRepository,
}
impl MusicTagService {
    pub fn new(repository: LibraryRepository) -> Self {
        Self { repository }
    }

    /// Startup only records interruption. It never publishes, restores or
    /// releases a backup. Old previews need a new process-local capability.
    pub async fn recover_interrupted(&self) -> Result<u64, RepositoryError> {
        Ok(sqlx::query("UPDATE music_tag_operations SET state='interrupted',error_code='interrupted',updated_at=? WHERE state='running'")
            .bind(Utc::now().to_rfc3339()).execute(self.repository.pool()).await?.rows_affected())
    }
    pub async fn read(&self, file_id: Uuid) -> Result<MusicTagRead, RepositoryError> {
        let _permit = audio_budget()
            .acquire()
            .await
            .map_err(|_| RepositoryError::Conflict)?;
        let (file, root) = ensure_music_file(&self.repository, file_id).await?;
        let lease = open_lease(Path::new(&root.path), &file.relative_path, false)?;
        let bytes = read_optional(&lease.parent, basename(&file.relative_path)?)?
            .ok_or(RepositoryError::NotFound)?
            .0;
        audio_read(file_id, &bytes)
    }
    pub async fn preview(
        &self,
        file_id: Uuid,
        changes: BTreeMap<String, String>,
    ) -> Result<MusicTagPreview, RepositoryError> {
        let _permit = audio_budget()
            .acquire()
            .await
            .map_err(|_| RepositoryError::Conflict)?;
        if changes.is_empty()
            || changes.len() > field_keys().len()
            || changes.values().any(|v| v.len() > 8192 || v.contains('\0'))
        {
            return Err(RepositoryError::InvalidInput);
        }
        let (file, root) = ensure_music_file(&self.repository, file_id).await?;
        let mut protection = self.repository.pool().begin().await?;
        ensure_tag_protection(&mut protection, file_id, &changes).await?;
        protection.commit().await?;
        let lease = open_lease(Path::new(&root.path), &file.relative_path, false)?;
        let (before, fingerprint) = read_optional(&lease.parent, basename(&file.relative_path)?)?
            .ok_or(RepositoryError::NotFound)?;
        let before_tags = audio_read(file_id, &before)?;
        let payload = apply_tags(&before, &changes)?;
        let after_tags = audio_read(file_id, &payload)?;
        let mut diff = Vec::new();
        for (field, value) in changes {
            if !field_keys().iter().any(|(key, _)| *key == field) {
                return Err(RepositoryError::InvalidInput);
            }
            if after_tags.tags.get(&field) != Some(&value) {
                return Err(RepositoryError::InvalidInput);
            }
            let old = before_tags.tags.get(&field).cloned();
            if old.as_ref() != Some(&value) {
                diff.push(MusicTagChange {
                    field,
                    before: old,
                    after: Some(value),
                });
            }
        }
        if diff.is_empty() {
            return Err(RepositoryError::InvalidInput);
        }
        let id = Uuid::new_v4();
        let preview = MusicTagPreview {
            id,
            file_id,
            changes: diff,
            kind: "write".into(),
            state: "preview".into(),
        };
        let plan = Plan {
            preview: preview.clone(),
            root: root.path.into(),
            root_id: root.id,
            media_item_id: file.media_item_id,
            relative: file.relative_path,
            before: Some(fingerprint),
            payload_hash: hash(&payload),
            backup_directory: format!(".mediaatlas-tag-{id}"),
        };
        self.save_preview(plan, Some(before), payload, None, lease)
            .await?;
        Ok(preview)
    }
    pub async fn restore_preview(
        &self,
        operation_id: Uuid,
    ) -> Result<MusicTagPreview, RepositoryError> {
        let _permit = audio_budget()
            .acquire()
            .await
            .map_err(|_| RepositoryError::Conflict)?;
        let row =
            sqlx::query("SELECT plan_json,before_bytes,state FROM music_tag_operations WHERE id=?")
                .bind(operation_id.to_string())
                .fetch_optional(self.repository.pool())
                .await?
                .ok_or(RepositoryError::NotFound)?;
        let original: Plan = serde_json::from_str(row.try_get("plan_json")?)
            .map_err(|_| RepositoryError::CorruptData)?;
        let payload: Vec<u8> = row
            .try_get::<Option<Vec<u8>>, _>("before_bytes")?
            .ok_or(RepositoryError::InvalidInput)?;
        let original_before = original
            .before
            .as_ref()
            .ok_or(RepositoryError::CorruptData)?;
        if payload.len() as u64 != original_before.len || hash(&payload) != original_before.hash {
            return Err(RepositoryError::CorruptData);
        }
        let state: String = row.try_get("state")?;
        if !matches!(
            state.as_str(),
            "succeeded" | "failed" | "conflict" | "interrupted" | "cancelled"
        ) {
            return Err(RepositoryError::Conflict);
        }
        let (file, root) = ensure_music_file(&self.repository, original.preview.file_id).await?;
        if root.id != original.root_id
            || Path::new(&root.path) != original.root
            || file.relative_path != original.relative
            || file.media_item_id != original.media_item_id
        {
            return Err(RepositoryError::Conflict);
        }
        let lease = open_lease(Path::new(&root.path), &file.relative_path, true)?;
        let current = read_optional(&lease.parent, basename(&file.relative_path)?)?;
        let current_hash = current.as_ref().map(|(_, fp)| fp.hash.as_str());
        let allowed = if state == "succeeded" {
            current_hash == Some(original.payload_hash.as_str())
        } else {
            current_hash.is_none()
                || current_hash == Some(original.payload_hash.as_str())
                || current_hash == Some(original_before.hash.as_str())
        };
        if !allowed {
            return Err(RepositoryError::Conflict);
        }
        let before = current.as_ref().map(|(_, fp)| fp.clone());
        let before_bytes = current.map(|(bytes, _)| bytes);
        let new_tags = audio_read(file.id, &payload)?;
        let old_tags = before_bytes
            .as_ref()
            .map(|bytes| audio_read(file.id, bytes))
            .transpose()?;
        // A restore is a whole-file byte restoration, including opaque tags,
        // cover art and audio; the explicit byte summary makes this visible.
        let mut diff: Vec<MusicTagChange> = field_keys()
            .iter()
            .filter_map(|(field, _)| {
                let old = old_tags
                    .as_ref()
                    .and_then(|read| read.tags.get(*field))
                    .cloned();
                let new = new_tags.tags.get(*field).cloned();
                (old != new).then(|| MusicTagChange {
                    field: (*field).into(),
                    before: old,
                    after: new,
                })
            })
            .collect();
        diff.push(MusicTagChange {
            field: "file_sha256".into(),
            before: before.as_ref().map(|fp| fp.hash.clone()),
            after: Some(hash(&payload)),
        });
        let id = Uuid::new_v4();
        let preview = MusicTagPreview {
            id,
            file_id: file.id,
            changes: diff,
            kind: "restore".into(),
            state: "preview".into(),
        };
        let plan = Plan {
            preview: preview.clone(),
            root: root.path.into(),
            root_id: root.id,
            media_item_id: file.media_item_id,
            relative: file.relative_path,
            before,
            payload_hash: hash(&payload),
            backup_directory: format!(".mediaatlas-tag-{id}"),
        };
        self.save_preview(plan, before_bytes, payload, Some(operation_id), lease)
            .await?;
        Ok(preview)
    }
    async fn save_preview(
        &self,
        plan: Plan,
        before: Option<Vec<u8>>,
        payload: Vec<u8>,
        original_id: Option<Uuid>,
        lease: Lease,
    ) -> Result<(), RepositoryError> {
        let mut active = leases().lock().await;
        active.retain(|_, lease| lease.created.elapsed() < PREVIEW_TTL);
        if active.len() >= MAX_PREVIEWS {
            if let Some(oldest) = active
                .iter()
                .min_by_key(|(_, lease)| lease.created)
                .map(|(id, _)| *id)
            {
                active.remove(&oldest);
            }
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO music_tag_operations(id,file_id,kind,original_operation_id,plan_json,before_bytes,payload,state,created_at,updated_at) VALUES(?,?,?,?,?,?,?,'preview',?,?)")
            .bind(plan.preview.id.to_string()).bind(plan.preview.file_id.to_string()).bind(&plan.preview.kind)
            .bind(original_id.map(|id| id.to_string())).bind(serde_json::to_string(&plan).map_err(|_| RepositoryError::InvalidInput)?)
            .bind(before).bind(payload).bind(&now).bind(&now).execute(self.repository.pool()).await?;
        let id = plan.preview.id;
        active.insert(id, Arc::new(lease));
        tokio::spawn(async move {
            tokio::time::sleep(PREVIEW_TTL).await;
            // UUID-specific expiry actively releases idle Windows no-delete
            // handles even if the user never performs another operation.
            leases().lock().await.remove(&id);
        });
        Ok(())
    }
    pub async fn discard(&self, preview_id: Uuid) -> Result<(), RepositoryError> {
        let _guard = mutation_lock().lock().await;
        let row = sqlx::query("SELECT file_id FROM music_tag_operations WHERE id=?")
            .bind(preview_id.to_string())
            .fetch_optional(self.repository.pool())
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let file_id = parse_id(row.try_get("file_id")?)?;
        ensure_music_file(&self.repository, file_id).await?;
        sqlx::query("UPDATE music_tag_operations SET state='cancelled',updated_at=? WHERE id=? AND state='preview'")
            .bind(Utc::now().to_rfc3339()).bind(preview_id.to_string()).execute(self.repository.pool()).await?;
        leases().lock().await.remove(&preview_id);
        Ok(())
    }
    pub async fn history(&self, file_id: Uuid) -> Result<Vec<MusicTagResult>, RepositoryError> {
        ensure_music_file(&self.repository, file_id).await?;
        let rows = sqlx::query("SELECT id,file_id,state,error_code,plan_json FROM music_tag_operations WHERE file_id=? AND state IN ('succeeded','failed','conflict','interrupted','cancelled') ORDER BY created_at DESC,id DESC LIMIT 100")
            .bind(file_id.to_string()).fetch_all(self.repository.pool()).await?;
        rows.iter().map(result_from_row).collect()
    }
    pub async fn execute(&self, preview_id: Uuid) -> Result<MusicTagResult, RepositoryError> {
        self.execute_kind(preview_id, "write", None).await
    }
    pub async fn restore_execute(
        &self,
        preview_id: Uuid,
    ) -> Result<MusicTagResult, RepositoryError> {
        self.execute_kind(preview_id, "restore", None).await
    }
    pub async fn execute_cancellable(
        &self,
        preview_id: Uuid,
        cancel: &CancellationToken,
    ) -> Result<MusicTagResult, RepositoryError> {
        self.execute_kind(preview_id, "write", Some(cancel)).await
    }
    async fn execute_kind(
        &self,
        preview_id: Uuid,
        kind: &str,
        cancel: Option<&CancellationToken>,
    ) -> Result<MusicTagResult, RepositoryError> {
        let _guard = mutation_lock().lock().await;
        let row = sqlx::query("SELECT * FROM music_tag_operations WHERE id=?")
            .bind(preview_id.to_string())
            .fetch_optional(self.repository.pool())
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let plan: Plan = serde_json::from_str(row.try_get("plan_json")?)
            .map_err(|_| RepositoryError::CorruptData)?;
        if plan.preview.id != preview_id
            || plan.backup_directory != format!(".mediaatlas-tag-{preview_id}")
        {
            return Err(RepositoryError::CorruptData);
        }
        if plan.preview.kind != kind {
            return Err(RepositoryError::InvalidInput);
        }
        let payload: Vec<u8> = row.try_get("payload")?;
        if hash(&payload) != plan.payload_hash {
            return Err(RepositoryError::CorruptData);
        }
        self.verify_association(&plan).await?;
        let state: String = row.try_get("state")?;
        if state == "succeeded" {
            let lease = open_lease(&plan.root, &plan.relative, false)?;
            let current = read_optional(&lease.parent, basename(&plan.relative)?)?
                .ok_or(RepositoryError::Conflict)?;
            if current.1.hash != plan.payload_hash {
                return Err(RepositoryError::Conflict);
            }
            return result_from_row(&row);
        }
        if state != "preview" {
            return Err(RepositoryError::Conflict);
        }
        let lease = {
            let mut active = leases().lock().await;
            active.retain(|_, lease| lease.created.elapsed() < PREVIEW_TTL);
            active
                .remove(&preview_id)
                .ok_or(RepositoryError::Conflict)?
        };
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            self.set_state(preview_id, "cancelled", Some("cancelled"))
                .await?;
            return Err(cancelled());
        }
        self.set_state(preview_id, "running", None).await?;
        // Acquire SQLite's write lock before the filesystem mutation. Root/kind/
        // isolation changes cannot commit during this confirmed operation.
        let mut transaction = self.repository.pool().begin().await?;
        sqlx::query("UPDATE music_tag_operations SET updated_at=updated_at WHERE id=?")
            .bind(preview_id.to_string())
            .execute(&mut *transaction)
            .await?;
        let isolation = ensure_music_file_tx(&mut transaction, plan.preview.file_id).await;
        match isolation {
            Ok((file, root))
                if file.media_item_id == plan.media_item_id
                    && file.root_id == plan.root_id
                    && file.relative_path == plan.relative
                    && Path::new(&root.path) == plan.root => {}
            _ => {
                transaction.rollback().await?;
                self.set_state(preview_id, "conflict", Some("privacy_conflict"))
                    .await?;
                return Err(RepositoryError::Conflict);
            }
        }
        if kind == "write" {
            let changes = plan
                .preview
                .changes
                .iter()
                .filter_map(|change| {
                    change
                        .after
                        .as_ref()
                        .map(|value| (change.field.clone(), value.clone()))
                })
                .collect();
            if let Err(error) =
                ensure_tag_protection(&mut transaction, plan.preview.file_id, &changes).await
            {
                transaction.rollback().await?;
                self.set_state(preview_id, "conflict", Some("protected_metadata"))
                    .await?;
                return Err(error);
            }
        }
        let mutation = publish(&plan, &payload, &lease, cancel);
        if mutation.is_ok() {
            let current = read_optional(&lease.parent, basename(&plan.relative)?)?
                .ok_or(RepositoryError::Conflict)?
                .1;
            if current.hash != plan.payload_hash {
                return Err(RepositoryError::Conflict);
            }
            sqlx::query("UPDATE media_files SET size_bytes=?,modified_at=? WHERE id=?")
                .bind(i64::try_from(current.len).map_err(|_| RepositoryError::InvalidInput)?)
                .bind(
                    i64::try_from(current.modified_nanos / 1_000_000)
                        .map_err(|_| RepositoryError::InvalidInput)?,
                )
                .bind(plan.preview.file_id.to_string())
                .execute(&mut *transaction)
                .await?;
        }
        let (state, code) = match mutation {
            Ok(()) => ("succeeded", None),
            Err(RepositoryError::Provider(mediaatlas_domain::ProviderFailure::Cancelled)) => {
                ("cancelled", Some("cancelled"))
            }
            Err(RepositoryError::Conflict) => ("conflict", Some("source_conflict")),
            Err(_) => ("failed", Some("write_failed")),
        };
        sqlx::query("UPDATE music_tag_operations SET state=?,error_code=?,updated_at=? WHERE id=?")
            .bind(state)
            .bind(code)
            .bind(Utc::now().to_rfc3339())
            .bind(preview_id.to_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        mutation?;
        Ok(MusicTagResult {
            operation_id: preview_id,
            file_id: plan.preview.file_id,
            state: state.into(),
            backup_relative: backup_relative(&plan),
            error_code: code.map(str::to_owned),
        })
    }
    async fn verify_association(&self, plan: &Plan) -> Result<(), RepositoryError> {
        let (file, root) = ensure_music_file(&self.repository, plan.preview.file_id).await?;
        if file.media_item_id != plan.media_item_id
            || file.root_id != plan.root_id
            || file.relative_path != plan.relative
            || Path::new(&root.path) != plan.root
        {
            return Err(RepositoryError::Conflict);
        }
        Ok(())
    }
    async fn set_state(
        &self,
        id: Uuid,
        state: &str,
        code: Option<&str>,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE music_tag_operations SET state=?,error_code=?,updated_at=? WHERE id=?")
            .bind(state)
            .bind(code)
            .bind(Utc::now().to_rfc3339())
            .bind(id.to_string())
            .execute(self.repository.pool())
            .await?;
        Ok(())
    }
}
async fn ensure_tag_protection(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    file_id: Uuid,
    changes: &BTreeMap<String, String>,
) -> Result<(), RepositoryError> {
    let rows = sqlx::query("WITH targets(id) AS (SELECT media_item_id FROM media_files WHERE id=? UNION SELECT t.media_item_id FROM music_track_files tf JOIN music_tracks t ON t.media_item_id=tf.track_id WHERE tf.file_id=? UNION SELECT t.recording_id FROM music_track_files tf JOIN music_tracks t ON t.media_item_id=tf.track_id WHERE tf.file_id=? UNION SELECT m.release_id FROM music_track_files tf JOIN music_tracks t ON t.media_item_id=tf.track_id JOIN music_media m ON m.id=t.medium_id WHERE tf.file_id=?) SELECT v.field_key,v.value_json,mi.kind FROM metadata_values v JOIN targets target ON target.id=v.media_item_id JOIN media_items mi ON mi.id=v.media_item_id WHERE v.is_locked=1 OR v.is_user_modified=1")
        .bind(file_id.to_string()).bind(file_id.to_string()).bind(file_id.to_string()).bind(file_id.to_string()).fetch_all(&mut **tx).await?;
    for row in rows {
        let field: String = row.try_get("field_key")?;
        let kind: String = row.try_get("kind")?;
        let field = if field == "title" && kind == "music_release" {
            "album"
        } else {
            field.as_str()
        };
        if let Some(proposed) = changes.get(field) {
            let value: serde_json::Value = serde_json::from_str(row.try_get("value_json")?)
                .map_err(|_| RepositoryError::CorruptData)?;
            let current = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            if &current != proposed {
                return Err(RepositoryError::ProtectedMetadata);
            }
        }
    }
    Ok(())
}
fn result_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<MusicTagResult, RepositoryError> {
    let plan: Plan = serde_json::from_str(row.try_get("plan_json")?)
        .map_err(|_| RepositoryError::CorruptData)?;
    Ok(MusicTagResult {
        operation_id: parse_id(row.try_get("id")?)?,
        file_id: parse_id(row.try_get("file_id")?)?,
        state: row.try_get("state")?,
        backup_relative: backup_relative(&plan),
        error_code: row.try_get("error_code")?,
    })
}
fn backup_relative(plan: &Plan) -> Option<String> {
    plan.before.as_ref().map(|_| {
        Path::new(&plan.relative)
            .parent()
            .unwrap_or(Path::new(""))
            .join(&plan.backup_directory)
            .join("source")
            .to_string_lossy()
            .into_owned()
    })
}
fn parse_id(value: &str) -> Result<Uuid, RepositoryError> {
    Uuid::parse_str(value).map_err(|_| RepositoryError::CorruptData)
}
fn cancelled() -> RepositoryError {
    RepositoryError::Provider(mediaatlas_domain::ProviderFailure::Cancelled)
}
fn io_error(_: std::io::Error) -> RepositoryError {
    RepositoryError::InvalidInput
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn basename(relative: &str) -> Result<&std::ffi::OsStr, RepositoryError> {
    checked_relative(relative)?
        .file_name()
        .ok_or(RepositoryError::InvalidInput)?;
    Path::new(relative)
        .file_name()
        .ok_or(RepositoryError::InvalidInput)
}
fn checked_relative(relative: &str) -> Result<&Path, RepositoryError> {
    let path = Path::new(relative);
    if relative.contains(['\\', '\0'])
        || relative.is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(path)
}
fn open_lease(root: &Path, relative: &str, allow_missing: bool) -> Result<Lease, RepositoryError> {
    checked_relative(relative)?;
    if !root.is_absolute() {
        return Err(RepositoryError::InvalidInput);
    }
    let complete = root.join(relative);
    let mut anchor = PathBuf::new();
    let mut parts = Vec::new();
    for component in complete.components() {
        match component {
            Component::Prefix(p) => anchor.push(p.as_os_str()),
            Component::RootDir => anchor.push(component.as_os_str()),
            Component::Normal(n) => parts.push(n.to_os_string()),
            _ => return Err(RepositoryError::InvalidInput),
        }
    }
    let name = parts.pop().ok_or(RepositoryError::InvalidInput)?;
    let mut current = Dir::open_ambient_dir(anchor, ambient_authority()).map_err(io_error)?;
    let mut ancestors = Vec::new();
    for part in parts {
        #[cfg(windows)]
        let next = {
            use cap_fs_ext::OpenOptionsMaybeDirExt;
            use cap_std::fs::OpenOptionsExt;
            let mut options = OpenOptions::new();
            options
                .read(true)
                .maybe_dir(true)
                .follow(FollowSymlinks::No)
                .share_mode(0x1 | 0x2);
            let file = current.open_with(part, &options).map_err(io_error)?;
            if !file.metadata().map_err(io_error)?.is_dir() {
                return Err(RepositoryError::InvalidInput);
            }
            Dir::from_std_file(file.into_std())
        };
        #[cfg(not(windows))]
        let next = current.open_dir_nofollow(part).map_err(io_error)?;
        ancestors.push(current);
        current = next;
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.share_mode(0x1 | 0x2);
    }
    let source = match current.open_with(name, &options) {
        Ok(file) if file.metadata().map_err(io_error)?.is_file() => Some(file),
        Ok(_) => return Err(RepositoryError::InvalidInput),
        Err(e) if allow_missing && e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(io_error(e)),
    };
    Ok(Lease {
        _ancestors: ancestors,
        parent: current,
        source: std::sync::Mutex::new(source),
        created: Instant::now(),
    })
}
fn read_optional(
    dir: &Dir,
    name: impl AsRef<Path>,
) -> Result<Option<(Vec<u8>, Fingerprint)>, RepositoryError> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = match dir.open_with(name, &options) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_error(e)),
    };
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() || metadata.len() > MAX_AUDIO as u64 {
        return Err(RepositoryError::InvalidInput);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_AUDIO as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > MAX_AUDIO {
        return Err(RepositoryError::InvalidInput);
    }
    // Metadata after reading catches in-place edits during the snapshot.
    let after = file.metadata().map_err(io_error)?;
    let fingerprint = fingerprint(&metadata, &bytes)?;
    if fingerprint != fingerprint_from_hash(&after, fingerprint.hash.clone())? {
        return Err(RepositoryError::Conflict);
    }
    Ok(Some((bytes, fingerprint)))
}
fn fingerprint(
    metadata: &cap_std::fs::Metadata,
    bytes: &[u8],
) -> Result<Fingerprint, RepositoryError> {
    fingerprint_from_hash(metadata, hash(bytes))
}
fn fingerprint_from_hash(
    metadata: &cap_std::fs::Metadata,
    content_hash: String,
) -> Result<Fingerprint, RepositoryError> {
    #[cfg(unix)]
    let identity = {
        use cap_std::fs::MetadataExt;
        format!("{}:{}", metadata.dev(), metadata.ino())
    };
    #[cfg(windows)]
    let identity = {
        use cap_fs_ext::MetadataExt as WindowsByHandleExt;
        use cap_std::fs::MetadataExt as WindowsMetadataExt;
        static SESSION: OnceLock<String> = OnceLock::new();
        // The legacy index is supplementary only, never claimed as a complete
        // ReFS 128-bit identifier. Full bytes/hash plus metadata and actual moved
        // quarantine verification guard the replacement operation.
        format!(
            "{}:{:?}:{:?}:{}",
            SESSION.get_or_init(|| Uuid::new_v4().to_string()),
            WindowsByHandleExt::dev(metadata),
            WindowsByHandleExt::ino(metadata),
            WindowsMetadataExt::creation_time(metadata)
        )
    };
    #[cfg(not(any(unix, windows)))]
    return Err(RepositoryError::InvalidInput);
    Ok(Fingerprint {
        len: metadata.len(),
        modified_nanos: metadata
            .modified()
            .map_err(io_error)?
            .into_std()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| RepositoryError::InvalidInput)?
            .as_nanos(),
        created_nanos: metadata
            .created()
            .ok()
            .and_then(|time| time.into_std().duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos()),
        identity,
        hash: content_hash,
    })
}
fn publish(
    plan: &Plan,
    payload: &[u8],
    lease: &Lease,
    cancel: Option<&CancellationToken>,
) -> Result<(), RepositoryError> {
    publish_with_hook(plan, payload, lease, cancel, |_| {})
}
fn publish_with_hook(
    plan: &Plan,
    payload: &[u8],
    lease: &Lease,
    cancel: Option<&CancellationToken>,
    mut hook: impl FnMut(&str),
) -> Result<(), RepositoryError> {
    // Reopen the *current* pathname through no-follow capabilities. A retained
    // directory fd may otherwise still point to a detached/renamed ancestor.
    let current = open_lease(&plan.root, &plan.relative, plan.before.is_none())?;
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        let old = lease.parent.dir_metadata().map_err(io_error)?;
        let now = current.parent.dir_metadata().map_err(io_error)?;
        if old.dev() != now.dev() || old.ino() != now.ino() {
            return Err(RepositoryError::Conflict);
        }
    }
    drop(current);
    let parent = &lease.parent;
    let name = basename(&plan.relative)?;
    if read_optional(parent, name)?.map(|(_, fp)| fp) != plan.before {
        return Err(RepositoryError::Conflict);
    }
    let temporary = format!(".mediaatlas-tag-{}.tmp", Uuid::new_v4());
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.share_mode(0x1);
    }
    let mut staged = parent.open_with(&temporary, &options).map_err(io_error)?;
    let result = (|| {
        staged.write_all(payload).map_err(io_error)?;
        if let Some(source) = lease
            .source
            .lock()
            .map_err(|_| RepositoryError::Conflict)?
            .as_ref()
        {
            staged
                .set_permissions(source.metadata().map_err(io_error)?.permissions())
                .map_err(io_error)?;
        }
        staged.sync_all().map_err(io_error)?;
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            return Err(cancelled());
        }
        let quarantine = if plan.before.is_some() {
            // Unique operation-owned directory; no rename can overwrite a
            // previous backup or the user's next source file.
            parent
                .create_dir(&plan.backup_directory)
                .map_err(io_error)?;
            Some(open_quarantine(parent, &plan.backup_directory)?)
        } else {
            None
        };
        if let Some(quarantine) = quarantine.as_ref() {
            // Windows must release OUR source no-delete handle for the move.
            // Ancestor handles stay leased. Actual moved bytes are rechecked;
            // an external intervening replacement is never treated as ours.
            lease
                .source
                .lock()
                .map_err(|_| RepositoryError::Conflict)?
                .take();
            hook("before_move");
            quarantine_source(plan, parent, name, quarantine)?;
            let moved = read_optional(quarantine, "source");
            if moved
                .as_ref()
                .ok()
                .and_then(|value| value.as_ref().map(|(_, fp)| fp))
                != plan.before.as_ref()
            {
                // Put the ACTUAL moved object back only if the original name
                // is still vacant. Never remove or overwrite an external file.
                let _ = quarantine.hard_link("source", parent, name);
                return Err(RepositoryError::Conflict);
            }
        }
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            if let Some(quarantine) = quarantine.as_ref() {
                let _ = quarantine.hard_link("source", parent, name);
            }
            return Err(cancelled());
        }
        hook("before_publish");
        match parent.hard_link(&temporary, parent, name) {
            Ok(()) => {
                // Retain the original on disk and in SQLite. An editor with
                // an old open descriptor cannot mutate the newly published file.
                if read_optional(parent, name)?.is_none_or(|(_, fp)| fp.hash != plan.payload_hash) {
                    return Err(RepositoryError::Conflict);
                }
                Ok(())
            }
            Err(e) => {
                if let Some(quarantine) = quarantine.as_ref() {
                    let _ = quarantine.hard_link("source", parent, name);
                }
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    Err(RepositoryError::Conflict)
                } else {
                    Err(io_error(e))
                }
            }
        }
    })();
    drop(staged);
    let _ = parent.remove_file(&temporary);
    result
}
fn open_quarantine(parent: &Dir, name: &str) -> Result<Dir, RepositoryError> {
    #[cfg(windows)]
    {
        use cap_fs_ext::OpenOptionsMaybeDirExt;
        use cap_std::fs::OpenOptionsExt;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .maybe_dir(true)
            .follow(FollowSymlinks::No)
            .share_mode(0x1 | 0x2);
        let file = parent.open_with(name, &options).map_err(io_error)?;
        if !file.metadata().map_err(io_error)?.is_dir() {
            return Err(RepositoryError::InvalidInput);
        }
        Ok(Dir::from_std_file(file.into_std()))
    }
    #[cfg(not(windows))]
    {
        parent.open_dir_nofollow(name).map_err(io_error)
    }
}
fn quarantine_source(
    plan: &Plan,
    parent: &Dir,
    name: &std::ffi::OsStr,
    quarantine: &Dir,
) -> Result<(), RepositoryError> {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))]
    {
        let _ = plan;
        rustix::fs::renameat_with(
            parent,
            name,
            quarantine,
            "source",
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|_| RepositoryError::Conflict)
    }
    #[cfg(windows)]
    {
        // The target is inside the unique directory created by this operation,
        // held by a no-follow capability. The source move must precede verified
        // no-clobber publication; never rename tagged bytes over a source path.
        let _ = (parent, name, quarantine);
        let source = plan.root.join(&plan.relative);
        let destination = source
            .parent()
            .ok_or(RepositoryError::InvalidInput)?
            .join(&plan.backup_directory)
            .join("source");
        // atomicwrites uses MoveFileExW without MOVEFILE_REPLACE_EXISTING.
        // Full no-delete directory leases pin every absolute path component,
        // including the quarantine dir, while OUR source handle is released.
        atomicwrites::move_atomic(&source, &destination).map_err(io_error)
    }
    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    )))]
    {
        let _ = (parent, name, quarantine);
        Err(RepositoryError::InvalidInput)
    }
}
fn field_keys() -> Vec<(&'static str, ItemKey)> {
    vec![
        ("title", ItemKey::TrackTitle),
        ("artist", ItemKey::TrackArtist),
        ("album", ItemKey::AlbumTitle),
        ("album_artist", ItemKey::AlbumArtist),
        ("date", ItemKey::RecordingDate),
        ("year", ItemKey::Year),
        ("track_number", ItemKey::TrackNumber),
        ("track_total", ItemKey::TrackTotal),
        ("disc_number", ItemKey::DiscNumber),
        ("disc_total", ItemKey::DiscTotal),
        ("genre", ItemKey::Genre),
        ("comment", ItemKey::Comment),
        ("musicbrainz_release_id", ItemKey::MusicBrainzReleaseId),
        (
            "musicbrainz_release_group_id",
            ItemKey::MusicBrainzReleaseGroupId,
        ),
        ("musicbrainz_track_id", ItemKey::MusicBrainzTrackId),
        ("musicbrainz_recording_id", ItemKey::MusicBrainzRecordingId),
        ("musicbrainz_artist_id", ItemKey::MusicBrainzArtistId),
        ("isrc", ItemKey::Isrc),
        ("barcode", ItemKey::Barcode),
        ("catalog_number", ItemKey::CatalogNumber),
    ]
}
fn parse_audio(bytes: &[u8]) -> Result<TaggedFile, RepositoryError> {
    Probe::new(Cursor::new(bytes))
        .guess_file_type()
        .map_err(|_| RepositoryError::InvalidInput)?
        .options(
            ParseOptions::new()
                .read_cover_art(true)
                .parsing_mode(ParsingMode::Strict),
        )
        .read()
        .map_err(|_| RepositoryError::InvalidInput)
}
fn audio_read(file_id: Uuid, bytes: &[u8]) -> Result<MusicTagRead, RepositoryError> {
    let parsed = parse_audio(bytes)?;
    let (format, writable) = audio_format(parsed.file_type());
    let tags = field_keys()
        .into_iter()
        .filter_map(|(field, key)| {
            parsed
                .primary_tag()
                .and_then(|tag| tag.get_string(&key))
                .or_else(|| parsed.tags().iter().find_map(|tag| tag.get_string(&key)))
                .map(|value| (field.into(), value.into()))
        })
        .collect();
    Ok(MusicTagRead {
        file_id,
        tags,
        format: format.into(),
        writable,
        picture_count: parsed.tags().iter().map(|tag| tag.pictures().len()).sum(),
    })
}
fn audio_format(format: FileType) -> (&'static str, bool) {
    match format {
        FileType::Mpeg => ("mp3", true),
        FileType::Flac => ("flac", true),
        FileType::Mp4 => ("m4a", true),
        FileType::Vorbis => ("ogg", true),
        FileType::Opus => ("opus", true),
        FileType::Wav => ("wav", true),
        _ => ("unsupported", false),
    }
}
fn apply_tags(
    bytes: &[u8],
    changes: &BTreeMap<String, String>,
) -> Result<Vec<u8>, RepositoryError> {
    let mut parsed = parse_audio(bytes)?;
    if !audio_format(parsed.file_type()).1 {
        return Err(RepositoryError::InvalidInput);
    }
    let tag_type = parsed.primary_tag_type();
    if parsed.primary_tag().is_none() {
        parsed.insert_tag(Tag::new(tag_type));
    }
    let tag = parsed
        .primary_tag_mut()
        .ok_or(RepositoryError::InvalidInput)?;
    for (field, value) in changes {
        let key = field_keys()
            .into_iter()
            .find(|(name, _)| name == field)
            .map(|(_, key)| key)
            .ok_or(RepositoryError::InvalidInput)?;
        if !tag.insert_text(key, value.clone()) {
            return Err(RepositoryError::InvalidInput);
        }
    }
    let mut target = Cursor::new(bytes.to_vec());
    // Save only the changed primary tag. Other tag containers stay byte-for-byte
    // in the original file. Lofty's default format-specific preservation retains
    // opaque frames/atoms in the primary tag and cover art was explicitly read.
    tag.save_to(&mut target, WriteOptions::default())
        .map_err(|_| RepositoryError::InvalidInput)?;
    if target.get_ref().len() > MAX_AUDIO {
        return Err(RepositoryError::InvalidInput);
    }
    let result = target.into_inner();
    let check = parse_audio(&result)?;
    if check.primary_tag().map(|tag| tag.pictures())
        != parsed.primary_tag().map(|tag| tag.pictures())
    {
        return Err(RepositoryError::Conflict);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::{
        file::AudioFile,
        picture::{MimeType, Picture, PictureType},
    };
    use mediaatlas_domain::{LibraryRoot, MediaFile, MediaItem, MediaKind, PathStyle};
    use tempfile::TempDir;

    const FIXTURES: &[(&str, &[u8])] = &[
        ("mp3", include_bytes!("../tests/fixtures/music/silence.mp3")),
        (
            "flac",
            include_bytes!("../tests/fixtures/music/silence.flac"),
        ),
        ("m4a", include_bytes!("../tests/fixtures/music/silence.m4a")),
        ("ogg", include_bytes!("../tests/fixtures/music/silence.ogg")),
        (
            "opus",
            include_bytes!("../tests/fixtures/music/silence.opus"),
        ),
        ("wav", include_bytes!("../tests/fixtures/music/silence.wav")),
    ];
    async fn library(ext: &str, bytes: &[u8]) -> (TempDir, LibraryRepository, MediaFile) {
        let directory = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&directory.path().join("library.sqlite"))
            .await
            .unwrap();
        let root = LibraryRoot {
            id: Uuid::new_v4(),
            name: "Synthetic audio only".into(),
            path: directory.path().to_string_lossy().into(),
            path_style: if cfg!(windows) {
                PathStyle::Windows
            } else {
                PathStyle::Posix
            },
            kind: Some(MediaKind::MusicTrack),
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.save_root(&root).await.unwrap();
        let item = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::MusicTrack,
            title: "Synthetic".into(),
            original_title: None,
            year: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.save_media(&item).await.unwrap();
        let file = MediaFile {
            id: Uuid::new_v4(),
            media_item_id: item.id,
            root_id: root.id,
            relative_path: format!("silence.{ext}"),
            size_bytes: bytes.len() as u64,
            modified_at: None,
            part: None,
        };
        repo.save_file(&file).await.unwrap();
        std::fs::write(directory.path().join(&file.relative_path), bytes).unwrap();
        (directory, repo, file)
    }
    fn title(value: &str) -> BTreeMap<String, String> {
        BTreeMap::from([("title".into(), value.into())])
    }
    fn enriched(bytes: &[u8]) -> Vec<u8> {
        let mut parsed = parse_audio(bytes).unwrap();
        let kind = parsed.primary_tag_type();
        if parsed.primary_tag().is_none() {
            parsed.insert_tag(Tag::new(kind));
        }
        let tag = parsed.primary_tag_mut().unwrap();
        // A valid 1x1 PNG. The application preserves its bytes without decoding.
        let image = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 0, 2, 0, 0,
            5, 0, 1, 165, 246, 69, 64, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ];
        tag.push_picture(Picture::new_unchecked(
            PictureType::CoverFront,
            Some(MimeType::Png),
            None,
            image,
        ));
        // Format-specific unknown fields are read back from the full Tag, not
        // the limited public DTO allowlist, proving they survive writeback.
        let unknown = match kind {
            lofty::tag::TagType::Mp4Ilst => "----:org.mediaatlas:UNSELECTED",
            lofty::tag::TagType::Id3v2 => "MEDIAATLAS_UNSELECTED",
            _ => "MEDIAATLAS_UNSELECTED",
        };
        tag.insert_text(
            ItemKey::Unknown(unknown.into()),
            "Opaque retained value".into(),
        );
        let mut cursor = Cursor::new(bytes.to_vec());
        parsed
            .save_to(&mut cursor, WriteOptions::default())
            .unwrap();
        if kind == lofty::tag::TagType::Mp4Ilst {
            use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst};
            let parsed = parse_audio(cursor.get_ref()).unwrap();
            let mut native: Ilst = parsed.primary_tag().unwrap().clone().into();
            native.insert(Atom::new(
                AtomIdent::Fourcc(*b"Xmat"),
                AtomData::UTF8("Opaque retained value".into()),
            ));
            cursor.set_position(0);
            native
                .save_to(&mut cursor, WriteOptions::default())
                .unwrap();
        }
        if kind == lofty::tag::TagType::Id3v2 {
            use lofty::id3::v2::Id3v2Tag;
            let parsed = parse_audio(cursor.get_ref()).unwrap();
            let mut native: Id3v2Tag = parsed.primary_tag().unwrap().clone().into();
            native.insert_user_text(
                "MEDIAATLAS_UNSELECTED".into(),
                "Opaque retained value".into(),
            );
            cursor.set_position(0);
            native
                .save_to(&mut cursor, WriteOptions::default())
                .unwrap();
        }
        cursor.into_inner()
    }
    #[tokio::test]
    async fn six_real_formats_preview_write_readback_byte_backup_restore_and_idempotency() {
        for (ext, fixture) in FIXTURES {
            let bytes = enriched(fixture);
            let (directory, repo, file) = library(ext, &bytes).await;
            let service = MusicTagService::new(repo.clone());
            let old = service.read(file.id).await.unwrap();
            assert_eq!(old.format, *ext);
            assert!(old.writable);
            assert_eq!(old.picture_count, 1, "{ext}");
            let before_parsed = parse_audio(&bytes).unwrap();
            let unknown_before: Vec<_> = before_parsed
                .primary_tag()
                .unwrap()
                .items()
                .filter(|item| matches!(item.key(), ItemKey::Unknown(_)))
                .cloned()
                .collect();
            assert!(
                !unknown_before.is_empty(),
                "fixture must contain an unknown {ext} item"
            );
            let preview = service
                .preview(file.id, title("Explicitly confirmed title"))
                .await
                .unwrap();
            let path = directory.path().join(&file.relative_path);
            assert_eq!(
                std::fs::read(&path).unwrap(),
                bytes,
                "preview writes no source {ext}"
            );
            let result = service.execute(preview.id).await.unwrap();
            assert_eq!(result.state, "succeeded");
            assert_eq!(
                std::fs::read(
                    directory
                        .path()
                        .join(result.backup_relative.as_ref().unwrap())
                )
                .unwrap(),
                bytes
            );
            assert_eq!(
                service.read(file.id).await.unwrap().tags["title"],
                "Explicitly confirmed title"
            );
            assert_eq!(service.read(file.id).await.unwrap().picture_count, 1);
            let tagged = std::fs::read(&path).unwrap();
            if let Some(evidence) = std::env::var_os("MEDIAATLAS_TAG_EVIDENCE_DIR") {
                let evidence = PathBuf::from(evidence);
                std::fs::create_dir_all(&evidence).unwrap();
                std::fs::write(evidence.join(format!("silence.{ext}")), &tagged).unwrap();
            }
            assert_eq!(
                audio_essence(ext, &bytes),
                audio_essence(ext, &tagged),
                "audio essence {ext}"
            );
            for (field, value) in &old.tags {
                if field != "title" {
                    assert_eq!(
                        service.read(file.id).await.unwrap().tags.get(field),
                        Some(value),
                        "unselected {field} {ext}"
                    );
                }
            }
            let after_parsed = parse_audio(&tagged).unwrap();
            let unknown_after: Vec<_> = after_parsed
                .primary_tag()
                .unwrap()
                .items()
                .filter(|item| matches!(item.key(), ItemKey::Unknown(_)))
                .cloned()
                .collect();
            assert_eq!(
                unknown_before, unknown_after,
                "unknown {ext} tags must survive"
            );
            assert_eq!(
                before_parsed.primary_tag().unwrap().pictures(),
                after_parsed.primary_tag().unwrap().pictures(),
                "cover bytes {ext}"
            );
            assert_eq!(
                service.execute(preview.id).await.unwrap().state,
                "succeeded"
            );
            assert_eq!(std::fs::read(&path).unwrap(), tagged);
            assert_eq!(service.history(file.id).await.unwrap().len(), 1);
            let restore = service.restore_preview(preview.id).await.unwrap();
            assert_eq!(
                std::fs::read(&path).unwrap(),
                tagged,
                "restore preview writes no source"
            );
            assert!(restore
                .changes
                .iter()
                .any(|diff| diff.field == "file_sha256"));
            let restore_result = service.restore_execute(restore.id).await.unwrap();
            assert_eq!(restore_result.state, "succeeded");
            assert_eq!(
                std::fs::read(&path).unwrap(),
                bytes,
                "exact byte restore {ext}"
            );
            assert_eq!(
                std::fs::read(
                    directory
                        .path()
                        .join(restore_result.backup_relative.unwrap())
                )
                .unwrap(),
                tagged,
                "restore retains replaced tagged bytes"
            );
        }
    }
    #[tokio::test]
    async fn stale_source_and_restore_changes_never_overwrite_external_bytes() {
        let (dir, repo, file) = library("mp3", FIXTURES[0].1).await;
        let service = MusicTagService::new(repo);
        let first = service.preview(file.id, title("New")).await.unwrap();
        let path = dir.path().join(&file.relative_path);
        let changed = apply_tags(FIXTURES[0].1, &title("External editor")).unwrap();
        std::fs::write(&path, &changed).unwrap();
        assert_eq!(
            service.execute(first.id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        assert_eq!(std::fs::read(&path).unwrap(), changed);
        let second = service.preview(file.id, title("Confirmed")).await.unwrap();
        service.execute(second.id).await.unwrap();
        let restore = service.restore_preview(second.id).await.unwrap();
        std::fs::write(&path, &changed).unwrap();
        assert_eq!(
            service.restore_execute(restore.id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        assert_eq!(std::fs::read(&path).unwrap(), changed);
    }
    #[tokio::test]
    async fn discard_and_cancellation_and_restart_require_fresh_confirmation() {
        let (dir, repo, file) = library("flac", FIXTURES[1].1).await;
        let service = MusicTagService::new(repo.clone());
        let preview = service.preview(file.id, title("Discarded")).await.unwrap();
        service.discard(preview.id).await.unwrap();
        assert_eq!(
            service.execute(preview.id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        let preview = service.preview(file.id, title("Cancelled")).await.unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            service
                .execute_cancellable(preview.id, &cancel)
                .await
                .unwrap_err(),
            cancelled()
        );
        let interrupted = service
            .preview(file.id, title("Interrupted"))
            .await
            .unwrap();
        sqlx::query("UPDATE music_tag_operations SET state='running' WHERE id=?")
            .bind(interrupted.id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        assert_eq!(service.recover_interrupted().await.unwrap(), 1);
        assert_eq!(
            service.execute(interrupted.id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        let no_session = service
            .preview(file.id, title("After restart"))
            .await
            .unwrap();
        leases().lock().await.remove(&no_session.id);
        assert_eq!(
            service.execute(no_session.id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        assert_eq!(
            std::fs::read(dir.path().join(file.relative_path)).unwrap(),
            FIXTURES[1].1
        );
    }
    #[tokio::test]
    async fn immutable_payloads_and_adult_reclassification_are_rejected() {
        let (dir, repo, file) = library("wav", FIXTURES[5].1).await;
        let service = MusicTagService::new(repo.clone());
        let preview = service
            .preview(file.id, title("Never publish"))
            .await
            .unwrap();
        assert!(
            sqlx::query("UPDATE music_tag_operations SET payload=x'00' WHERE id=?")
                .bind(preview.id.to_string())
                .execute(repo.pool())
                .await
                .is_err()
        );
        sqlx::query("UPDATE media_items SET adult_isolated=1,kind='music_track' WHERE id=?")
            .bind(file.media_item_id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        assert!(service.read(file.id).await.is_err());
        assert!(service.execute(preview.id).await.is_err());
        assert!(service.history(file.id).await.is_err());
        assert_eq!(
            std::fs::read(dir.path().join(file.relative_path)).unwrap(),
            FIXTURES[5].1
        );
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_final_and_replaced_ancestor_fail_without_touching_either_directory() {
        use std::os::unix::fs::symlink;
        let (dir, repo, mut file) = library("ogg", FIXTURES[3].1).await;
        let service = MusicTagService::new(repo.clone());
        let original = dir.path().join(&file.relative_path);
        let target = dir.path().join("unrelated.ogg");
        std::fs::rename(&original, &target).unwrap();
        symlink(&target, &original).unwrap();
        assert!(service.read(file.id).await.is_err());
        assert!(service.preview(file.id, title("No")).await.is_err());
        std::fs::remove_file(&original).unwrap();
        std::fs::rename(&target, &original).unwrap();
        std::fs::create_dir(dir.path().join("album")).unwrap();
        std::fs::rename(&original, dir.path().join("album/source.ogg")).unwrap();
        file.relative_path = "album/source.ogg".into();
        repo.save_file(&file).await.unwrap();
        let preview = service.preview(file.id, title("No")).await.unwrap();
        std::fs::rename(dir.path().join("album"), dir.path().join("detached")).unwrap();
        std::fs::create_dir(dir.path().join("album")).unwrap();
        std::fs::write(dir.path().join("album/source.ogg"), FIXTURES[3].1).unwrap();
        assert_eq!(
            service.execute(preview.id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        assert_eq!(
            std::fs::read(dir.path().join("album/source.ogg")).unwrap(),
            FIXTURES[3].1
        );
        assert_eq!(
            std::fs::read(dir.path().join("detached/source.ogg")).unwrap(),
            FIXTURES[3].1
        );
    }
    #[test]
    fn invalid_unsupported_and_unknown_selected_fields_fail_closed() {
        assert!(parse_audio(b"not audio").is_err());
        assert!(apply_tags(
            FIXTURES[0].1,
            &BTreeMap::from([("unknown_field".into(), "x".into())])
        )
        .is_err());
    }
    fn audio_essence(ext: &str, bytes: &[u8]) -> Vec<Vec<u8>> {
        match ext {
            "mp3" => {
                let mut start = 0;
                if bytes.starts_with(b"ID3") {
                    let size = bytes[6..10].iter().fold(0usize, |size, value| {
                        (size << 7) | usize::from(*value & 0x7f)
                    });
                    start = 10 + size;
                }
                let end = if bytes.len() >= 128
                    && &bytes[bytes.len() - 128..bytes.len() - 125] == b"TAG"
                {
                    bytes.len() - 128
                } else {
                    bytes.len()
                };
                vec![bytes[start..end].to_vec()]
            }
            "flac" => {
                assert!(bytes.starts_with(b"fLaC"));
                let mut index = 4;
                loop {
                    let last = bytes[index] & 0x80 != 0;
                    let len = (usize::from(bytes[index + 1]) << 16)
                        | (usize::from(bytes[index + 2]) << 8)
                        | usize::from(bytes[index + 3]);
                    index += 4 + len;
                    if last {
                        break;
                    }
                }
                vec![bytes[index..].to_vec()]
            }
            "m4a" => {
                let mut result = Vec::new();
                let mut index = 0;
                while index + 8 <= bytes.len() {
                    let size =
                        u32::from_be_bytes(bytes[index..index + 4].try_into().unwrap()) as usize;
                    let header = if size == 1 { 16 } else { 8 };
                    let size = if size == 1 {
                        u64::from_be_bytes(bytes[index + 8..index + 16].try_into().unwrap())
                            as usize
                    } else if size == 0 {
                        bytes.len() - index
                    } else {
                        size
                    };
                    assert!(size >= header && index + size <= bytes.len());
                    if &bytes[index + 4..index + 8] == b"mdat" {
                        result.push(bytes[index + header..index + size].to_vec());
                    }
                    index += size;
                }
                assert!(!result.is_empty());
                result
            }
            "wav" => {
                assert!(bytes.starts_with(b"RIFF"));
                let mut result = Vec::new();
                let mut index = 12;
                while index + 8 <= bytes.len() {
                    let len = u32::from_le_bytes(bytes[index + 4..index + 8].try_into().unwrap())
                        as usize;
                    if &bytes[index..index + 4] == b"data" {
                        result.push(bytes[index + 8..index + 8 + len].to_vec());
                    }
                    index += 8 + len + (len % 2);
                }
                assert!(!result.is_empty());
                result
            }
            "ogg" | "opus" => {
                let mut index = 0;
                let mut packet = Vec::new();
                let mut packets = Vec::new();
                while index < bytes.len() {
                    assert_eq!(&bytes[index..index + 4], b"OggS");
                    let count = bytes[index + 26] as usize;
                    let laces = &bytes[index + 27..index + 27 + count];
                    let mut offset = index + 27 + count;
                    for length in laces {
                        let length = *length as usize;
                        packet.extend_from_slice(&bytes[offset..offset + length]);
                        offset += length;
                        if length < 255 {
                            if !packet.starts_with(b"\x03vorbis")
                                && !packet.starts_with(b"OpusTags")
                            {
                                packets.push(std::mem::take(&mut packet));
                            } else {
                                packet.clear();
                            }
                        }
                    }
                    index = offset;
                }
                assert!(!packets.is_empty());
                packets
            }
            _ => panic!("fixture format"),
        }
    }
    #[test]
    fn native_id3_binary_opaque_frame_survives_selected_generic_edit() {
        use lofty::id3::v2::{BinaryFrame, Frame, FrameId, Id3v2Tag};
        let bytes = enriched(FIXTURES[0].1);
        let parsed = parse_audio(&bytes).unwrap();
        let mut native: Id3v2Tag = parsed.primary_tag().unwrap().clone().into();
        let id = FrameId::new("XMAT").unwrap();
        native.insert(Frame::Binary(BinaryFrame::new(
            id.clone(),
            vec![0, 255, 128, 0, 17, 42, 0],
        )));
        let mut cursor = Cursor::new(bytes);
        native
            .save_to(&mut cursor, WriteOptions::default())
            .unwrap();
        let original = cursor.into_inner();
        let changed = apply_tags(&original, &title("Updated while opaque stays")).unwrap();
        let parsed = parse_audio(&changed).unwrap();
        let native_after: Id3v2Tag = parsed.primary_tag().unwrap().clone().into();
        assert_eq!(native.get(&id), native_after.get(&id));
        assert!(matches!(native_after.get(&id), Some(Frame::Binary(_))));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn race_during_source_move_verifies_actual_moved_bytes_and_never_clobbers_new_source() {
        for stage in ["before_move", "before_publish"] {
            let (dir, repo, file) = library("mp3", FIXTURES[0].1).await;
            let service = MusicTagService::new(repo.clone());
            let preview = service
                .preview(file.id, title("Our payload"))
                .await
                .unwrap();
            let row = sqlx::query("SELECT plan_json,payload FROM music_tag_operations WHERE id=?")
                .bind(preview.id.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
            let plan: Plan = serde_json::from_str(row.try_get("plan_json").unwrap()).unwrap();
            let payload: Vec<u8> = row.try_get("payload").unwrap();
            let lease = leases().lock().await.remove(&preview.id).unwrap();
            let external = apply_tags(FIXTURES[0].1, &title("External replacement")).unwrap();
            let path = dir.path().join(&file.relative_path);
            let result = publish_with_hook(&plan, &payload, &lease, None, |point| {
                if point == stage {
                    if stage == "before_move" {
                        std::fs::rename(&path, dir.path().join("external-held-original.mp3"))
                            .unwrap();
                    }
                    std::fs::write(&path, &external).unwrap();
                }
            });
            assert_eq!(result.unwrap_err(), RepositoryError::Conflict);
            assert_eq!(std::fs::read(&path).unwrap(), external);
            let backup = std::fs::read(dir.path().join(backup_relative(&plan).unwrap())).unwrap();
            assert_eq!(
                backup,
                if stage == "before_move" {
                    external
                } else {
                    FIXTURES[0].1.to_vec()
                }
            );
            if stage == "before_move" {
                assert_eq!(
                    std::fs::read(dir.path().join("external-held-original.mp3")).unwrap(),
                    FIXTURES[0].1
                );
            }
        }
    }
    #[tokio::test]
    async fn restore_rejects_changes_before_preview_and_corrupt_original_snapshots() {
        let (dir, repo, file) = library("mp3", FIXTURES[0].1).await;
        let service = MusicTagService::new(repo.clone());
        let preview = service.preview(file.id, title("Our title")).await.unwrap();
        service.execute(preview.id).await.unwrap();
        let path = dir.path().join(&file.relative_path);
        let tagged = std::fs::read(&path).unwrap();
        let external = apply_tags(&tagged, &title("External later edit")).unwrap();
        std::fs::write(&path, &external).unwrap();
        assert_eq!(
            service.restore_preview(preview.id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        std::fs::write(&path, &tagged).unwrap();
        sqlx::query("DROP TRIGGER music_tag_plan_immutable")
            .execute(repo.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE music_tag_operations SET before_bytes=? WHERE id=?")
            .bind(&external)
            .bind(preview.id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        assert_eq!(
            service.restore_preview(preview.id).await.unwrap_err(),
            RepositoryError::CorruptData
        );
        assert_eq!(std::fs::read(&path).unwrap(), tagged);
    }
    #[tokio::test]
    async fn protected_tag_fields_recheck_after_preview_without_source_changes() {
        let (dir, repo, file) = library("mp3", FIXTURES[0].1).await;
        let service = MusicTagService::new(repo.clone());
        let preview = service
            .preview(file.id, title("Provider proposed"))
            .await
            .unwrap();
        let value = mediaatlas_domain::MetadataValue {
            id: Uuid::new_v4(),
            media_item_id: file.media_item_id,
            field_key: "title".into(),
            locale: Some("zh-CN".into()),
            value: serde_json::json!("Protected local value"),
            provider_id: None,
            source_url: None,
            fetched_at: None,
            confidence: None,
            is_user_modified: true,
            is_locked: true,
        };
        repo.save_metadata_value(&value, crate::library::MetadataWriteMode::User)
            .await
            .unwrap();
        assert_eq!(
            service.execute(preview.id).await.unwrap_err(),
            RepositoryError::ProtectedMetadata
        );
        assert_eq!(
            service
                .preview(file.id, title("Overwrite locked"))
                .await
                .unwrap_err(),
            RepositoryError::ProtectedMetadata
        );
        assert_eq!(
            std::fs::read(dir.path().join(file.relative_path)).unwrap(),
            FIXTURES[0].1
        );
    }
}
