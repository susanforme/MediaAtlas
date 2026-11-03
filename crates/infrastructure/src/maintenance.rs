//! Bounded local maintenance. Full backups include private metadata, never media
//! files or OS/session secret values. Restore is staged for an exclusive startup.
use crate::{library::LibraryRepository, media_output::AdultOutputAccess};
#[cfg(not(windows))]
use cap_fs_ext::DirExt;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{sqlite::SqliteConnectOptions, ConnectOptions, Connection, Row, SqliteConnection};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Mutex as AsyncMutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const MAX_DATABASE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_BACKUPS: usize = 128;
const MAX_CACHE_FILES: usize = 20_000;
const MAX_JSON_BYTES: u64 = 64 * 1024;
const PLAN_LIFETIME: Duration = Duration::from_secs(600);
use crate::migration_compat::canonical_migrator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceError {
    InvalidInput,
    Busy,
    Conflict,
    CorruptBackup,
    UnsupportedVersion,
    PrivacyLocked,
    Cancelled,
    Storage,
    Database,
    RecoveryRequired,
}
impl std::fmt::Display for MaintenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "Maintenance input is invalid.",
            Self::Busy => "Another maintenance operation or application instance is active.",
            Self::Conflict => "Maintenance files changed; create a fresh preview.",
            Self::CorruptBackup => "The backup is damaged or incompatible.",
            Self::UnsupportedVersion => "The backup requires a newer application version.",
            Self::PrivacyLocked => "Unlock the private workspace before full-library maintenance.",
            Self::Cancelled => "Maintenance was cancelled.",
            Self::Storage => "Maintenance storage is unavailable.",
            Self::Database => "Maintenance database operation failed.",
            Self::RecoveryRequired => {
                "An interrupted restore needs recovery; preserved files were not overwritten."
            }
        })
    }
}
impl std::error::Error for MaintenanceError {}
type Result<T> = std::result::Result<T, MaintenanceError>;
impl From<std::io::Error> for MaintenanceError {
    fn from(_: std::io::Error) -> Self {
        Self::Storage
    }
}
impl From<sqlx::Error> for MaintenanceError {
    fn from(_: sqlx::Error) -> Self {
        Self::Database
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BackupSummary {
    pub id: Uuid,
    pub created_at: String,
    pub app_version: String,
    pub schema_version: i64,
    pub bytes: u64,
    pub contains_private_metadata: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format_version: u32,
    summary: BackupSummary,
    sha256: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceStatus {
    pub backups: Vec<BackupSummary>,
    pub restore_pending: bool,
    pub directory: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct BackupPreview {
    pub id: Uuid,
    pub contains_private_metadata: bool,
    pub includes_secrets: bool,
    pub includes_source_media: bool,
    pub may_include_tag_recovery_bytes: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct RestorePreview {
    pub id: Uuid,
    pub backup_id: Uuid,
    pub contains_private_metadata: bool,
    pub restart_required: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct RestoreQueued {
    pub restart_required: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub id: Uuid,
    pub path: String,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct CachePreview {
    pub id: Uuid,
    pub files: usize,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct CacheResult {
    pub removed_files: usize,
}
#[derive(Clone)]
struct Plan {
    created: Instant,
    grant: Option<AdultOutputAccess>,
    kind: PlanKind,
}
#[derive(Clone)]
enum PlanKind {
    Backup,
    Restore { backup_id: Uuid, checksum: String },
    Cache(Vec<(PathBuf, CacheStamp)>),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Stamp {
    bytes: u64,
    modified_ns: u128,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CacheStamp {
    stamp: Stamp,
    identity: String,
    sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingRestore {
    format_version: u32,
    backup_id: Uuid,
    checksum: String,
    queued_at: String,
    private_authorized: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreJournal {
    format_version: u32,
    id: Uuid,
    original_sha256: String,
    candidate_sha256: String,
}

/// A cooperative OS file lock, held until every service and SQLite pool is gone.
/// All application instances must acquire this before opening this database.
pub struct DatabaseLease {
    _file: File,
    _parent: DirectoryLease,
    database: PathBuf,
}
impl DatabaseLease {
    pub fn acquire(database: &Path) -> Result<Self> {
        let parent = database.parent().ok_or(MaintenanceError::InvalidInput)?;
        ensure_directory(parent)?;
        if database.exists() {
            regular_file(database, MAX_DATABASE_BYTES)?;
        }
        let name = database
            .file_name()
            .ok_or(MaintenanceError::InvalidInput)?
            .to_string_lossy();
        let lock = parent.join(format!(".{name}.instance.lock"));
        if lock.exists() {
            regular_file(&lock, 4096)?;
        }
        let parent_lease = DirectoryLease::acquire(parent)?;
        let mut options = cap_std::fs::OpenOptions::new();
        use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
        options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .follow(FollowSymlinks::No);
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.share_mode(0x1 | 0x2);
        }
        let file = parent_lease
            .dir
            .open_with(
                lock.file_name().ok_or(MaintenanceError::InvalidInput)?,
                &options,
            )?
            .into_std();
        file.try_lock().map_err(|_| MaintenanceError::Busy)?;
        Ok(Self {
            _file: file,
            _parent: parent_lease,
            database: database.to_path_buf(),
        })
    }
    /// Run before constructing any pool. A queued restore never touches source
    /// media; original database, candidate and recovery snapshot remain retained.
    pub async fn apply_pending_restore(&self, directory: &Path) -> Result<bool> {
        self._parent.verify()?;
        ensure_directory(directory)?;
        let database_parent = DirectoryLease::acquire(
            self.database
                .parent()
                .ok_or(MaintenanceError::InvalidInput)?,
        )?;
        let maintenance_parent = DirectoryLease::acquire(directory)?;
        let journal_path = directory.join("restore-journal.json");
        if journal_path.exists() {
            self.recover_interrupted(directory, &journal_path).await?;
        }
        let pending_path = directory.join("pending-restore.json");
        if !pending_path.exists() {
            return Ok(false);
        }
        let pending: PendingRestore = read_json(&pending_path)?;
        if pending.format_version != 1 {
            return Err(MaintenanceError::UnsupportedVersion);
        }
        let manifest =
            validate_backup(directory, pending.backup_id, &CancellationToken::new()).await?;
        if manifest.sha256 != pending.checksum {
            return Err(MaintenanceError::Conflict);
        }
        let work = directory.join(format!("restore-{}", Uuid::new_v4()));
        maintenance_parent.verify()?;
        maintenance_parent
            .dir
            .create_dir(work.file_name().ok_or(MaintenanceError::InvalidInput)?)?;
        let work_parent = DirectoryLease::acquire(&work)?;
        let candidate = work.join("candidate.sqlite");
        #[cfg(test)]
        run_hook(
            "restore_before_copy",
            &backup_path(directory, pending.backup_id).join("library.sqlite"),
        );
        copy_snapshot(
            &backup_path(directory, pending.backup_id).join("library.sqlite"),
            &candidate,
            &CancellationToken::new(),
        )
        .await?;
        let copied = cache_stamp(
            &work_parent.dir,
            std::ffi::OsStr::new("candidate.sqlite"),
            &CancellationToken::new(),
        )?;
        if copied.sha256 != manifest.sha256 {
            return Err(MaintenanceError::Conflict);
        }
        prepare_restored_database(&candidate).await?;
        let candidate_stamp = cache_stamp(
            &work_parent.dir,
            std::ffi::OsStr::new("candidate.sqlite"),
            &CancellationToken::new(),
        )?;
        let candidate_sha256 = candidate_stamp.sha256.clone();
        if !self.database.exists() {
            return Err(MaintenanceError::Conflict);
        }
        regular_file(&self.database, MAX_DATABASE_BYTES)?;
        let mut current = open_connection(&self.database, false).await?;
        if private_connection(&mut current).await? && !pending.private_authorized {
            return Err(MaintenanceError::PrivacyLocked);
        }
        // Switching to DELETE requires SQLite's exclusive lock: fail closed if
        // another SQLite client still has a reader/writer, including a WAL user.
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode=DELETE")
            .fetch_one(&mut current)
            .await
            .map_err(|_| MaintenanceError::Busy)?;
        if !mode.eq_ignore_ascii_case("delete") {
            return Err(MaintenanceError::Busy);
        }
        sqlx::query("PRAGMA locking_mode=EXCLUSIVE")
            .execute(&mut current)
            .await?;
        sqlx::query("BEGIN EXCLUSIVE")
            .execute(&mut current)
            .await
            .map_err(|_| MaintenanceError::Busy)?;
        sqlx::query("COMMIT").execute(&mut current).await?;
        let safe_snapshot = work.join("before-restore.sqlite");
        vacuum_into(
            &mut current,
            &safe_snapshot,
            &CancellationToken::new(),
            None,
        )
        .await?;
        current.close().await?;
        reject_live_sidecars(&self.database)?;
        let original_sha256 = hash_file(&self.database, CancellationToken::new()).await?;
        let id = Uuid::parse_str(
            work.file_name()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("restore-"))
                .ok_or(MaintenanceError::Conflict)?,
        )
        .map_err(|_| MaintenanceError::Conflict)?;
        let journal = RestoreJournal {
            format_version: 1,
            id,
            original_sha256: original_sha256.clone(),
            candidate_sha256,
        };
        write_json_new(&journal_path, &journal)?;
        // No replace publication: the old DB is first quarantined in a unique
        // directory. The new DB is linked only into an absent destination.
        if hash_file(&self.database, CancellationToken::new()).await? != original_sha256 {
            return Err(MaintenanceError::Conflict);
        }
        reject_live_sidecars(&self.database)?;
        let original = work.join("original.sqlite");
        database_parent.verify()?;
        maintenance_parent.verify()?;
        work_parent.verify()?;
        #[cfg(test)]
        run_hook("restore_before_quarantine", &original);
        no_replace(&database_parent, &self.database, &work_parent, &original)?;
        if hash_file(&original, CancellationToken::new()).await? != original_sha256
            || reject_live_sidecars(&self.database).is_err()
        {
            return Err(MaintenanceError::RecoveryRequired);
        }
        database_parent.verify()?;
        work_parent.verify()?;
        #[cfg(test)]
        run_hook("restore_before_publish", &candidate);
        work_parent
            .dir
            .hard_link(
                "candidate.sqlite",
                &database_parent.dir,
                self.database
                    .file_name()
                    .ok_or(MaintenanceError::InvalidInput)?,
            )
            .map_err(|_| MaintenanceError::RecoveryRequired)?;
        verify_published(
            &database_parent,
            self.database
                .file_name()
                .ok_or(MaintenanceError::InvalidInput)?,
            &candidate_stamp,
        )?;
        work_parent.verify()?;
        sync_parent(&self.database)?;
        no_replace(
            &maintenance_parent,
            &pending_path,
            &work_parent,
            &work.join("consumed-plan.json"),
        )?;
        no_replace(
            &maintenance_parent,
            &journal_path,
            &work_parent,
            &work.join("completed-journal.json"),
        )?;
        sync_parent(&journal_path)?;
        Ok(true)
    }
    async fn recover_interrupted(&self, directory: &Path, journal_path: &Path) -> Result<()> {
        let journal: RestoreJournal = read_json(journal_path)?;
        if journal.format_version != 1 {
            return Err(MaintenanceError::RecoveryRequired);
        }
        let work = directory.join(format!("restore-{}", journal.id));
        ensure_existing_directory(&work)?;
        let database_parent = DirectoryLease::acquire(
            self.database
                .parent()
                .ok_or(MaintenanceError::InvalidInput)?,
        )?;
        let maintenance_parent = DirectoryLease::acquire(directory)?;
        let work_parent = DirectoryLease::acquire(&work)?;
        #[cfg(test)]
        let original = work.join("original.sqlite");
        if !self.database.exists() {
            // Roll back to the exact quarantined original, never replay restore.
            let expected = cache_stamp(
                &work_parent.dir,
                std::ffi::OsStr::new("original.sqlite"),
                &CancellationToken::new(),
            )
            .map_err(|_| MaintenanceError::RecoveryRequired)?;
            if expected.sha256 != journal.original_sha256 {
                return Err(MaintenanceError::RecoveryRequired);
            }
            reject_live_sidecars(&self.database)?;
            work_parent.verify()?;
            database_parent.verify()?;
            #[cfg(test)]
            run_hook("recovery_before_publish", &original);
            work_parent.dir.hard_link(
                "original.sqlite",
                &database_parent.dir,
                self.database
                    .file_name()
                    .ok_or(MaintenanceError::InvalidInput)?,
            )?;
            verify_published(
                &database_parent,
                self.database
                    .file_name()
                    .ok_or(MaintenanceError::InvalidInput)?,
                &expected,
            )?;
            work_parent.verify()?;
            sync_parent(&self.database)?;
        } else {
            let hash = cache_stamp(
                &database_parent.dir,
                self.database
                    .file_name()
                    .ok_or(MaintenanceError::InvalidInput)?,
                &CancellationToken::new(),
            )
            .map_err(|_| MaintenanceError::RecoveryRequired)?
            .sha256;
            if hash != journal.original_sha256 && hash != journal.candidate_sha256 {
                return Err(MaintenanceError::RecoveryRequired);
            }
        }
        // If publication completed before interruption, preserve that exact DB;
        // otherwise keep the old DB. Neither path automatically retries restore.
        let pending = directory.join("pending-restore.json");
        if pending.exists() {
            no_replace(
                &maintenance_parent,
                &pending,
                &work_parent,
                &work.join("interrupted-plan.json"),
            )?;
        }
        no_replace(
            &maintenance_parent,
            journal_path,
            &work_parent,
            &work.join("interrupted-journal.json"),
        )?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct MaintenanceService {
    repo: LibraryRepository,
    database: PathBuf,
    directory: PathBuf,
    directory_lease: Arc<DirectoryLease>,
    cache_lease: Arc<DirectoryLease>,
    plans: Arc<AsyncMutex<HashMap<Uuid, Plan>>>,
    gate: Arc<AsyncMutex<()>>,
    cancellation: Arc<Mutex<CancellationToken>>,
}
impl MaintenanceService {
    pub fn new(
        repo: LibraryRepository,
        database: PathBuf,
        directory: PathBuf,
        approved_cache_root: PathBuf,
    ) -> Result<Self> {
        ensure_directory(&directory)?;
        ensure_directory(&approved_cache_root)?;
        let directory = fs::canonicalize(directory)?;
        let cache = fs::canonicalize(approved_cache_root)?;
        if directory.starts_with(&cache) || cache.starts_with(&directory) {
            return Err(MaintenanceError::InvalidInput);
        }
        let cache_lease = Arc::new(DirectoryLease::acquire(&cache)?);
        let directory_lease = Arc::new(DirectoryLease::acquire(&directory)?);
        Ok(Self {
            repo,
            database,
            directory,
            directory_lease,
            cache_lease,
            plans: Arc::default(),
            gate: Arc::default(),
            cancellation: Arc::new(Mutex::new(CancellationToken::new())),
        })
    }
    /// Requests cancellation; the running IPC resolves only after its worker stops.
    pub fn cancel(&self) {
        if let Ok(token) = self.cancellation.lock() {
            token.cancel();
        }
    }
    fn begin_job(&self) -> CancellationToken {
        let token = CancellationToken::new();
        if let Ok(mut current) = self.cancellation.lock() {
            *current = token.clone();
        }
        token
    }
    pub async fn status(&self) -> Result<MaintenanceStatus> {
        let _gate = self.gate.try_lock().map_err(|_| MaintenanceError::Busy)?;
        let mut backups = Vec::new();
        for entry in fs::read_dir(&self.directory)?.take(4096) {
            let entry = entry?;
            let Some(id) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.strip_prefix("backup-"))
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                continue;
            };
            if backups.len() >= MAX_BACKUPS {
                return Err(MaintenanceError::Storage);
            }
            let path = entry.path();
            ensure_existing_directory(&path)?;
            if !path.join("manifest.json").exists() {
                continue;
            }
            let manifest: Manifest = read_json(&path.join("manifest.json"))?;
            if manifest.summary.id != id || manifest.format_version != 1 {
                return Err(MaintenanceError::CorruptBackup);
            }
            backups.push(manifest.summary);
        }
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(MaintenanceStatus {
            backups,
            restore_pending: self.directory.join("pending-restore.json").exists(),
            directory: self.directory.to_string_lossy().into_owned(),
        })
    }
    async fn add_plan(&self, kind: PlanKind, grant: Option<AdultOutputAccess>) -> Result<Uuid> {
        let mut plans = self.plans.lock().await;
        plans.retain(|_, p| p.created.elapsed() < PLAN_LIFETIME);
        if plans.len() >= 32 {
            return Err(MaintenanceError::Busy);
        }
        let id = Uuid::new_v4();
        plans.insert(
            id,
            Plan {
                created: Instant::now(),
                grant,
                kind,
            },
        );
        Ok(id)
    }
    async fn plan(&self, id: Uuid) -> Result<Plan> {
        let plan = self
            .plans
            .lock()
            .await
            .remove(&id)
            .ok_or(MaintenanceError::Conflict)?;
        if plan.created.elapsed() >= PLAN_LIFETIME {
            return Err(MaintenanceError::Conflict);
        }
        check_grant(plan.grant.as_ref())?;
        Ok(plan)
    }
    pub async fn backup_preview(&self, access: Option<AdultOutputAccess>) -> Result<BackupPreview> {
        let private = private_pool(&self.repo).await?;
        authorize(private, access.as_ref())?;
        let id = self.add_plan(PlanKind::Backup, access).await?;
        Ok(BackupPreview {
            id,
            contains_private_metadata: private,
            includes_source_media: false,
            may_include_tag_recovery_bytes: true,
            includes_secrets: false,
        })
    }
    pub async fn backup_confirm(
        &self,
        id: Uuid,
        access: Option<AdultOutputAccess>,
    ) -> Result<BackupSummary> {
        let _gate = self.gate.try_lock().map_err(|_| MaintenanceError::Busy)?;
        let plan = self.plan(id).await?;
        if !matches!(plan.kind, PlanKind::Backup) {
            return Err(MaintenanceError::InvalidInput);
        }
        authorize(private_pool(&self.repo).await?, access.as_ref())?;
        let token = self.begin_job();
        check_cancel(&token)?;
        let backup_id = Uuid::new_v4();
        let path = self
            .directory
            .join(format!("incomplete-backup-{backup_id}"));
        self.directory_lease.verify()?;
        self.directory_lease
            .dir
            .create_dir(path.file_name().ok_or(MaintenanceError::InvalidInput)?)?;
        let staging = DirectoryLease::acquire(&path)?;
        let database = path.join("library.sqlite");
        let mut conn = self.repo.pool().acquire().await?;
        staging.verify()?;
        self.directory_lease.verify()?;
        vacuum_into(&mut conn, &database, &token, plan.grant.clone()).await?;
        staging.verify()?;
        self.directory_lease.verify()?;
        check_cancel(&token)?;
        check_grant(plan.grant.as_ref())?;
        staging.verify()?;
        let mut snapshot = open_connection(&database, true).await?;
        set_budget(&mut snapshot, token.clone(), plan.grant.clone()).await?;
        let private = private_connection(&mut snapshot).await?;
        authorize(private, access.as_ref())?;
        authorize(private, plan.grant.as_ref())?;
        let schema_version = validate_schema(&mut snapshot).await?;
        snapshot.close().await?;
        let expected_snapshot =
            cache_stamp(&staging.dir, std::ffi::OsStr::new("library.sqlite"), &token)?;
        let bytes = expected_snapshot.stamp.bytes;
        let sha256 = expected_snapshot.sha256.clone();
        check_grant(plan.grant.as_ref())?;
        check_grant(access.as_ref())?;
        check_cancel(&token)?;
        let summary = BackupSummary {
            id: backup_id,
            created_at: Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            schema_version,
            bytes,
            contains_private_metadata: private,
        };
        let expected_manifest = Manifest {
            format_version: 1,
            summary: summary.clone(),
            sha256,
        };
        write_json_new(&path.join("manifest.json"), &expected_manifest)?;
        sync_parent(&database)?;
        let final_path = backup_path(&self.directory, backup_id);
        self.directory_lease.verify()?;
        staging.verify()?;
        self.directory_lease.dir.create_dir(
            final_path
                .file_name()
                .ok_or(MaintenanceError::InvalidInput)?,
        )?;
        let final_directory = DirectoryLease::acquire(&final_path)?;
        // create-new hard links never replace an existing backup. Manifest is
        // published last and is the only marker used by the backup catalogue.
        #[cfg(test)]
        run_hook("backup_before_publish", &database);
        staging
            .dir
            .hard_link("library.sqlite", &final_directory.dir, "library.sqlite")?;
        let actual = cache_stamp(
            &final_directory.dir,
            std::ffi::OsStr::new("library.sqlite"),
            &token,
        )?;
        if actual != expected_snapshot {
            return Err(MaintenanceError::Conflict);
        }
        #[cfg(test)]
        run_hook(
            "backup_before_manifest_publish",
            &path.join("manifest.json"),
        );
        if read_json_cap::<Manifest>(&staging.dir, "manifest.json")? != expected_manifest {
            return Err(MaintenanceError::Conflict);
        }
        staging
            .dir
            .hard_link("manifest.json", &final_directory.dir, "manifest.json")?;
        self.directory_lease.verify()?;
        staging.verify()?;
        final_directory.verify()?;
        sync_parent(&final_path.join("manifest.json"))?;
        if read_json_cap::<Manifest>(&final_directory.dir, "manifest.json")? != expected_manifest {
            return Err(MaintenanceError::Conflict);
        }
        let actual = cache_stamp(
            &final_directory.dir,
            std::ffi::OsStr::new("library.sqlite"),
            &token,
        )?;
        if actual != expected_snapshot {
            return Err(MaintenanceError::Conflict);
        }
        let validated = validate_backup(&self.directory, backup_id, &token).await?;
        if validated != expected_manifest {
            return Err(MaintenanceError::Conflict);
        }
        check_grant(plan.grant.as_ref())?;
        check_grant(access.as_ref())?;
        final_directory.verify()?;
        self.directory_lease.verify()?;
        Ok(summary)
    }
    pub async fn restore_preview(
        &self,
        backup_id: Uuid,
        access: Option<AdultOutputAccess>,
    ) -> Result<RestorePreview> {
        let _gate = self.gate.try_lock().map_err(|_| MaintenanceError::Busy)?;
        let token = self.begin_job();
        let manifest = validate_backup(&self.directory, backup_id, &token).await?;
        let private =
            manifest.summary.contains_private_metadata || private_pool(&self.repo).await?;
        authorize(private, access.as_ref())?;
        let id = self
            .add_plan(
                PlanKind::Restore {
                    backup_id,
                    checksum: manifest.sha256,
                },
                access,
            )
            .await?;
        Ok(RestorePreview {
            id,
            backup_id,
            contains_private_metadata: private,
            restart_required: true,
        })
    }
    pub async fn restore_confirm(
        &self,
        id: Uuid,
        access: Option<AdultOutputAccess>,
    ) -> Result<RestoreQueued> {
        let _gate = self.gate.try_lock().map_err(|_| MaintenanceError::Busy)?;
        let plan = self.plan(id).await?;
        let PlanKind::Restore {
            backup_id,
            checksum,
        } = plan.kind
        else {
            return Err(MaintenanceError::InvalidInput);
        };
        let token = self.begin_job();
        let manifest = validate_backup(&self.directory, backup_id, &token).await?;
        if manifest.sha256 != checksum {
            return Err(MaintenanceError::Conflict);
        }
        let private =
            manifest.summary.contains_private_metadata || private_pool(&self.repo).await?;
        authorize(private, access.as_ref())?;
        authorize(private, plan.grant.as_ref())?;
        check_cancel(&token)?;
        write_json_new(
            &self.directory.join("pending-restore.json"),
            &PendingRestore {
                format_version: 1,
                backup_id,
                checksum,
                queued_at: Utc::now().to_rfc3339(),
                private_authorized: plan.grant.is_some(),
            },
        )?;
        Ok(RestoreQueued {
            restart_required: true,
        })
    }
    /// Called after adult lock; waits for active maintenance to acknowledge
    /// revocation, then permanently retires any confirmed private restore plan.
    pub async fn revoke_private_restore(&self) -> Result<bool> {
        self.cancel();
        let _gate = self.gate.lock().await;
        let pending_path = self.directory.join("pending-restore.json");
        if !pending_path.exists() {
            return Ok(false);
        }
        let pending: PendingRestore = read_json(&pending_path)?;
        if !pending.private_authorized {
            return Ok(false);
        }
        let retired = self
            .directory
            .join(format!("revoked-restore-{}.json", Uuid::new_v4()));
        no_replace(
            &self.directory_lease,
            &pending_path,
            &self.directory_lease,
            &retired,
        )?;
        Ok(true)
    }
    pub async fn diagnostics_export(&self) -> Result<ExportResult> {
        let _gate = self.gate.try_lock().map_err(|_| MaintenanceError::Busy)?;
        let token = self.begin_job();
        check_cancel(&token)?;
        // Fixed fields and fixed enum states only. No log files, raw errors,
        // media metadata, provider names, user input, paths or network rows.
        let mut connection = open_connection(&self.database, true).await?;
        set_budget(&mut connection, token.clone(), None).await?;
        let sqlite_version: String = sqlx::query_scalar("SELECT sqlite_version()")
            .fetch_one(&mut connection)
            .await?;
        let page_count: i64 = sqlx::query_scalar("PRAGMA page_count")
            .fetch_one(&mut connection)
            .await?;
        let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
            .fetch_one(&mut connection)
            .await?;
        let migrations: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&mut connection)
            .await?;
        connection.close().await?;
        check_cancel(&token)?;
        let id = Uuid::new_v4();
        let path = self.directory.join(format!("diagnostics-{id}.json"));
        let report = serde_json::json!({"format_version":1,"app_version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"schema_version":canonical_migrator().iter().map(|m|m.version).max().unwrap_or(0),"sqlite_version":sqlite_version,"database_pages":page_count,"database_page_bytes":page_size,"applied_migrations":migrations,"supported_error_codes":["maintenance_storage","maintenance_conflict","maintenance_database"],"includes_logs":false,"includes_credentials":false,"includes_media_metadata":false});
        write_json_new(&path, &report)?;
        Ok(ExportResult {
            id,
            path: path.to_string_lossy().into_owned(),
            bytes: fs::metadata(path)?.len(),
        })
    }
    pub async fn cache_preview(&self) -> Result<CachePreview> {
        let _gate = self.gate.try_lock().map_err(|_| MaintenanceError::Busy)?;
        let token = self.begin_job();
        let files = cache_files(&self.cache_lease, &token)?;
        let bytes = files.iter().map(|(_, s)| s.stamp.bytes).sum();
        let count = files.len();
        let id = self.add_plan(PlanKind::Cache(files), None).await?;
        Ok(CachePreview {
            id,
            files: count,
            bytes,
        })
    }
    pub async fn cache_confirm(&self, id: Uuid) -> Result<CacheResult> {
        let _gate = self.gate.try_lock().map_err(|_| MaintenanceError::Busy)?;
        let plan = self.plan(id).await?;
        let PlanKind::Cache(files) = plan.kind else {
            return Err(MaintenanceError::InvalidInput);
        };
        let token = self.begin_job();
        let current = cache_files(&self.cache_lease, &token)?;
        if current != files {
            return Err(MaintenanceError::Conflict);
        }
        let maintenance_parent = DirectoryLease::acquire(&self.directory)?;
        let quarantine = self
            .directory
            .join(format!("cache-quarantine-{}", Uuid::new_v4()));
        maintenance_parent.dir.create_dir(
            quarantine
                .file_name()
                .ok_or(MaintenanceError::InvalidInput)?,
        )?;
        let quarantine_parent = DirectoryLease::acquire(&quarantine)?;
        let mut removed_files = 0;
        for (path, expected) in files {
            check_cancel(&token)?;
            self.cache_lease.verify()?;
            let name = path.file_name().ok_or(MaintenanceError::Conflict)?;
            if cache_stamp(&self.cache_lease.dir, name, &token)? != expected {
                return Err(MaintenanceError::Conflict);
            }
            #[cfg(test)]
            run_hook("cache_before_quarantine", &path);
            let target = quarantine.join(name);
            no_replace(&self.cache_lease, &path, &quarantine_parent, &target)?;
            let actual = cache_stamp(&quarantine_parent.dir, name, &token);
            if !actual.is_ok_and(|stamp| stamp == expected) {
                // Preserve the actual moved object. Restore only into an absent
                // name; a concurrent replacement keeps both byte sequences.
                let _ = no_replace(&quarantine_parent, &target, &self.cache_lease, &path);
                return Err(MaintenanceError::Conflict);
            }
            removed_files += 1;
        }
        // Quarantined cache bytes are deliberately retained. Resetting the
        // active cache never conditionally unlinks a concurrently changed file.
        Ok(CacheResult { removed_files })
    }
    pub fn database_path(&self) -> &Path {
        &self.database
    }
}

impl MaintenanceError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "maintenance_invalid_input",
            Self::Busy => "maintenance_busy",
            Self::Conflict => "maintenance_conflict",
            Self::CorruptBackup => "maintenance_corrupt_backup",
            Self::UnsupportedVersion => "maintenance_unsupported_version",
            Self::PrivacyLocked => "maintenance_privacy_locked",
            Self::Cancelled => "maintenance_cancelled",
            Self::Storage => "maintenance_storage",
            Self::Database => "maintenance_database",
            Self::RecoveryRequired => "maintenance_recovery_required",
        }
    }
}
pub(crate) struct DirectoryLease {
    path: PathBuf,
    pub(crate) dir: cap_std::fs::Dir,
    identity: String,
    _ancestors: Vec<cap_std::fs::Dir>,
}
impl DirectoryLease {
    pub(crate) fn acquire(path: &Path) -> Result<Self> {
        let (dir, ancestors) = open_directory_nofollow(path)?;
        let identity = directory_identity(&dir.dir_metadata()?)?;
        Ok(Self {
            path: path.to_path_buf(),
            dir,
            identity,
            _ancestors: ancestors,
        })
    }
    pub(crate) fn verify(&self) -> Result<()> {
        let (current, _ancestors) = open_directory_nofollow(&self.path)?;
        if directory_identity(&current.dir_metadata()?)? != self.identity {
            return Err(MaintenanceError::Conflict);
        }
        Ok(())
    }
}
fn open_directory_nofollow(path: &Path) -> Result<(cap_std::fs::Dir, Vec<cap_std::fs::Dir>)> {
    use std::path::Component;
    if !path.is_absolute() {
        return Err(MaintenanceError::InvalidInput);
    }
    let mut anchor = PathBuf::new();
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => anchor.push(prefix.as_os_str()),
            Component::RootDir => anchor.push(component.as_os_str()),
            Component::Normal(name) => names.push(name.to_os_string()),
            _ => return Err(MaintenanceError::InvalidInput),
        }
    }
    let mut dir = cap_std::fs::Dir::open_ambient_dir(anchor, cap_std::ambient_authority())?;
    let mut ancestors = Vec::new();
    for name in names {
        let next = open_child_directory(&dir, &name)?;
        ancestors.push(dir);
        dir = next;
    }
    Ok((dir, ancestors))
}
fn open_child_directory(
    dir: &cap_std::fs::Dir,
    name: &std::ffi::OsStr,
) -> Result<cap_std::fs::Dir> {
    #[cfg(windows)]
    {
        use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
        use cap_std::fs::OpenOptionsExt;
        let mut options = cap_std::fs::OpenOptions::new();
        options
            .read(true)
            .maybe_dir(true)
            .follow(FollowSymlinks::No)
            .share_mode(0x1 | 0x2);
        let file = dir.open_with(name, &options)?;
        if !file.metadata()?.is_dir() {
            return Err(MaintenanceError::Conflict);
        }
        Ok(cap_std::fs::Dir::from_std_file(file.into_std()))
    }
    #[cfg(not(windows))]
    {
        dir.open_dir_nofollow(name)
            .map_err(|_| MaintenanceError::Conflict)
    }
}
fn directory_identity(metadata: &cap_std::fs::Metadata) -> Result<String> {
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        Ok(format!("{}:{}", metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        use cap_fs_ext::MetadataExt as Handle;
        use cap_std::fs::MetadataExt as Win;
        Ok(format!(
            "{:?}:{:?}:{}",
            Handle::dev(metadata),
            Handle::ino(metadata),
            Win::creation_time(metadata)
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        Err(MaintenanceError::InvalidInput)
    }
}
fn no_replace(
    from_parent: &DirectoryLease,
    from: &Path,
    to_parent: &DirectoryLease,
    to: &Path,
) -> Result<()> {
    from_parent.verify()?;
    to_parent.verify()?;
    let from_name = from.file_name().ok_or(MaintenanceError::InvalidInput)?;
    let to_name = to.file_name().ok_or(MaintenanceError::InvalidInput)?;
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))]
    {
        rustix::fs::renameat_with(
            &from_parent.dir,
            from_name,
            &to_parent.dir,
            to_name,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|_| MaintenanceError::Conflict)?;
    }
    #[cfg(windows)]
    {
        let _ = (from_name, to_name);
        atomicwrites::move_atomic(from, to).map_err(|_| MaintenanceError::Conflict)?;
    }
    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    )))]
    {
        return Err(MaintenanceError::InvalidInput);
    }
    from_parent.verify()?;
    to_parent.verify()?;
    Ok(())
}
fn check_cancel(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() {
        Err(MaintenanceError::Cancelled)
    } else {
        Ok(())
    }
}
fn check_grant(access: Option<&AdultOutputAccess>) -> Result<()> {
    if let Some(access) = access {
        access
            .ensure()
            .map_err(|_| MaintenanceError::PrivacyLocked)?;
    }
    Ok(())
}
fn authorize(private: bool, access: Option<&AdultOutputAccess>) -> Result<()> {
    check_grant(access)?;
    if private && access.is_none() {
        Err(MaintenanceError::PrivacyLocked)
    } else {
        Ok(())
    }
}
fn ensure_existing_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(MaintenanceError::InvalidInput);
    }
    // Reject symlink ancestors too: approved directories must be actual local
    // application paths, not aliases into arbitrary user media directories.
    for ancestor in path.ancestors().skip(1) {
        if !ancestor.as_os_str().is_empty()
            && fs::symlink_metadata(ancestor)?.file_type().is_symlink()
        {
            return Err(MaintenanceError::InvalidInput);
        }
    }
    Ok(())
}
fn ensure_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    ensure_existing_directory(path)
}
fn regular_file(path: &Path, max: u64) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > max {
        return Err(MaintenanceError::CorruptBackup);
    }
    Ok(metadata)
}
fn file_stamp(path: &Path) -> Result<Stamp> {
    let m = regular_file(path, MAX_DATABASE_BYTES)?;
    Ok(Stamp {
        bytes: m.len(),
        modified_ns: m
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| MaintenanceError::Storage)?
            .as_nanos(),
    })
}
fn write_json_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let data = serde_json::to_vec_pretty(value).map_err(|_| MaintenanceError::Storage)?;
    if data.len() > MAX_JSON_BYTES as usize {
        return Err(MaintenanceError::Storage);
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                MaintenanceError::Conflict
            } else {
                MaintenanceError::Storage
            }
        })?;
    file.write_all(&data)?;
    file.sync_all()?;
    sync_parent(path)
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    regular_file(path, MAX_JSON_BYTES)?;
    let data = fs::read(path)?;
    serde_json::from_slice(&data).map_err(|_| MaintenanceError::CorruptBackup)
}
fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path.parent().ok_or(MaintenanceError::InvalidInput)?)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
fn backup_path(directory: &Path, id: Uuid) -> PathBuf {
    directory.join(format!("backup-{id}"))
}
async fn hash_file(path: &Path, token: CancellationToken) -> Result<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let deadline = Instant::now() + Duration::from_secs(120);
        let before = file_stamp(&path)?;
        let mut file = File::open(&path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut total = 0;
        loop {
            check_cancel(&token)?;
            if Instant::now() >= deadline {
                return Err(MaintenanceError::Cancelled);
            }
            let size = file.read(&mut buffer)?;
            if size == 0 {
                break;
            }
            total += size as u64;
            if total > MAX_DATABASE_BYTES {
                return Err(MaintenanceError::Storage);
            }
            hasher.update(&buffer[..size]);
        }
        if file_stamp(&path)? != before {
            return Err(MaintenanceError::Conflict);
        }
        Ok(format!("{:x}", hasher.finalize()))
    })
    .await
    .map_err(|_| MaintenanceError::Storage)?
}
async fn copy_snapshot(source: &Path, target: &Path, token: &CancellationToken) -> Result<()> {
    let before = hash_file(source, token.clone()).await?;
    let source = source.to_path_buf();
    let target = target.to_path_buf();
    let task_target = target.clone();
    let token = token.clone();
    let task_token = token.clone();
    tokio::task::spawn_blocking(move || {
        regular_file(&source, MAX_DATABASE_BYTES)?;
        let mut input = File::open(source)?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(task_target)?;
        let mut buffer = [0; 64 * 1024];
        let mut total = 0;
        loop {
            check_cancel(&task_token)?;
            let size = input.read(&mut buffer)?;
            if size == 0 {
                break;
            }
            total += size as u64;
            if total > MAX_DATABASE_BYTES {
                return Err(MaintenanceError::Storage);
            }
            output.write_all(&buffer[..size])?;
        }
        output.sync_all()?;
        Ok(())
    })
    .await
    .map_err(|_| MaintenanceError::Storage)??;
    if hash_file(&target, token).await? != before {
        return Err(MaintenanceError::Conflict);
    }
    Ok(())
}
async fn open_connection(path: &Path, readonly: bool) -> Result<SqliteConnection> {
    regular_file(path, MAX_DATABASE_BYTES)?;
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(readonly)
        .create_if_missing(false)
        .busy_timeout(Duration::from_secs(2))
        .disable_statement_logging();
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(|_| MaintenanceError::CorruptBackup)?;
    set_budget(&mut connection, CancellationToken::new(), None).await?;
    Ok(connection)
}
async fn set_budget(
    connection: &mut SqliteConnection,
    token: CancellationToken,
    grant: Option<AdultOutputAccess>,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(120);
    connection
        .lock_handle()
        .await?
        .set_progress_handler(4096, move || {
            !token.is_cancelled()
                && Instant::now() < deadline
                && grant.as_ref().is_none_or(|g| g.ensure().is_ok())
        });
    Ok(())
}

async fn vacuum_into(
    connection: &mut SqliteConnection,
    path: &Path,
    token: &CancellationToken,
    grant: Option<AdultOutputAccess>,
) -> Result<()> {
    if path.exists() {
        return Err(MaintenanceError::Conflict);
    }
    let target = path.to_str().ok_or(MaintenanceError::InvalidInput)?;
    let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(&mut *connection)
        .await?;
    let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(&mut *connection)
        .await?;
    if pages.saturating_mul(page_size) > MAX_DATABASE_BYTES as i64 {
        return Err(MaintenanceError::Storage);
    }
    let deadline = Instant::now() + Duration::from_secs(120);
    let progress_token = token.clone();
    let progress_grant = grant.clone();
    connection
        .lock_handle()
        .await?
        .set_progress_handler(4096, move || {
            !progress_token.is_cancelled()
                && Instant::now() < deadline
                && progress_grant.as_ref().is_none_or(|g| g.ensure().is_ok())
        });
    let result = sqlx::query("VACUUM INTO ?")
        .bind(target)
        .execute(&mut *connection)
        .await;
    connection.lock_handle().await?.remove_progress_handler();
    check_cancel(token)?;
    check_grant(grant.as_ref())?;
    if Instant::now() >= deadline {
        return Err(MaintenanceError::Cancelled);
    }
    result?;
    let file = OpenOptions::new().write(true).open(path)?;
    file.sync_all()?;
    Ok(())
}
async fn private_pool(repo: &LibraryRepository) -> Result<bool> {
    let mut conn = repo.pool().acquire().await?;
    set_budget(&mut conn, CancellationToken::new(), None).await?;
    let result = private_connection(&mut conn).await;
    conn.lock_handle().await?.remove_progress_handler();
    result
}
async fn private_connection(conn: &mut SqliteConnection) -> Result<bool> {
    let tables=sqlx::query_scalar::<_,String>("SELECT name FROM sqlite_master WHERE type='table' AND (name LIKE 'adult_%' OR name IN ('media_items','library_roots','scrape_tasks','provider_auth_settings','provider_sessions','scan_candidates','provider_requests','organization_operations','output_previews'))").fetch_all(&mut *conn).await?;
    for table in tables {
        let predicate = match table.as_str() {
            "adult_privacy_settings" => "entry_visible=1 OR covers_visible=1 OR remember_recent=1",
            "media_items" => {
                let columns = sqlx::query("PRAGMA table_info(media_items)")
                    .fetch_all(&mut *conn)
                    .await?;
                if columns.iter().any(|c| {
                    c.try_get::<String, _>("name")
                        .is_ok_and(|n| n == "adult_isolated")
                }) {
                    "kind='adult_video' OR adult_isolated=1"
                } else {
                    "kind='adult_video'"
                }
            }
            "library_roots" => "kind='adult_video'",
            "scrape_tasks" => "COALESCE(json_extract(input_snapshot,'$.adult'),0)=1",
            "provider_auth_settings" => "provider_id IN ('dmm','wikidata')",
            "provider_sessions" => {
                "provider_id IN ('dmm','wikidata','adult_scraper')"
            }
            "provider_requests"=>"task_id IN (SELECT task_id FROM provider_sessions WHERE provider_id IN ('dmm','wikidata','adult_scraper'))",
            "scan_candidates" => "kind='adult_video'",
            "organization_operations" => "lane='adult'",
            "output_previews" => "json_extract(snapshot,'$.channel')='adult'",
            other if other.starts_with("adult_") => "1=1",
            _ => continue,
        };
        // Table identifiers came from a fixed allowlist or the adult namespace;
        // quote any embedded double quote from an imported database identifier.
        let table = table.replace('"', "\"\"");
        let query = format!("SELECT EXISTS(SELECT 1 FROM \"{table}\" WHERE {predicate})");
        if sqlx::query_scalar::<_, bool>(&query)
            .fetch_one(&mut *conn)
            .await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}
async fn validate_schema(conn: &mut SqliteConnection) -> Result<i64> {
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check(1)")
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| MaintenanceError::CorruptBackup)?;
    if integrity != "ok" {
        return Err(MaintenanceError::CorruptBackup);
    }
    if sqlx::query("PRAGMA foreign_key_check")
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| MaintenanceError::CorruptBackup)?
        .is_some()
    {
        return Err(MaintenanceError::CorruptBackup);
    }
    let history = crate::migration_compat::validate_history(conn)
        .await
        .map_err(|error| match error {
            crate::migration_compat::CompatibilityError::FutureSchema => {
                MaintenanceError::UnsupportedVersion
            }
            _ => MaintenanceError::CorruptBackup,
        })?;
    if history.last_version == 0 {
        return Err(MaintenanceError::CorruptBackup);
    }
    Ok(history.last_version)
}
async fn validate_backup(
    directory: &Path,
    id: Uuid,
    token: &CancellationToken,
) -> Result<Manifest> {
    let result = validate_backup_inner(directory, id, token).await;
    check_cancel(token)?;
    result
}
async fn validate_backup_inner(
    directory: &Path,
    id: Uuid,
    token: &CancellationToken,
) -> Result<Manifest> {
    check_cancel(token)?;
    let root = backup_path(directory, id);
    ensure_existing_directory(&root)?;
    let manifest: Manifest = read_json(&root.join("manifest.json"))?;
    if manifest.format_version != 1 {
        return Err(MaintenanceError::UnsupportedVersion);
    }
    if manifest.summary.id != id
        || manifest.sha256.len() != 64
        || !manifest.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(MaintenanceError::CorruptBackup);
    }
    let version = semver::Version::parse(&manifest.summary.app_version)
        .map_err(|_| MaintenanceError::CorruptBackup)?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|_| MaintenanceError::UnsupportedVersion)?;
    if version > current {
        return Err(MaintenanceError::UnsupportedVersion);
    }
    let database = root.join("library.sqlite");
    if regular_file(&database, MAX_DATABASE_BYTES)?.len() != manifest.summary.bytes
        || hash_file(&database, token.clone()).await? != manifest.sha256
    {
        return Err(MaintenanceError::CorruptBackup);
    }
    // VACUUM snapshots must be standalone. Never fold externally supplied WAL
    // into the database after validating only its main-file checksum.
    reject_any_sidecars(&database)?;
    let mut conn = open_connection(&database, true).await?;
    set_budget(&mut conn, token.clone(), None).await?;
    let schema = validate_schema(&mut conn).await?;
    let private = private_connection(&mut conn).await?;
    conn.close().await?;
    if schema != manifest.summary.schema_version
        || private != manifest.summary.contains_private_metadata
    {
        return Err(MaintenanceError::CorruptBackup);
    }
    check_cancel(token)?;
    Ok(manifest)
}
fn sidecar_path(database: &Path, suffix: &str) -> PathBuf {
    let mut value = database.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}
fn reject_any_sidecars(database: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        if fs::symlink_metadata(sidecar_path(database, suffix)).is_ok() {
            return Err(MaintenanceError::Conflict);
        }
    }
    Ok(())
}
fn reject_live_sidecars(database: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let path = sidecar_path(database, suffix);
        if path.exists() {
            let m = regular_file(&path, MAX_DATABASE_BYTES)?;
            if m.len() > 0 {
                return Err(MaintenanceError::Busy);
            }
        }
    }
    Ok(())
}
async fn prepare_restored_database(path: &Path) -> Result<()> {
    let mut conn = open_connection(path, false).await?;
    validate_schema(&mut conn).await?;
    crate::migration_compat::normalize_candidate(&mut conn)
        .await
        .map_err(|_| MaintenanceError::CorruptBackup)?;
    canonical_migrator()
        .run_direct(&mut conn)
        .await
        .map_err(|_| MaintenanceError::CorruptBackup)?;
    let mut tx = conn.begin().await?;
    // Restored previews can refer to files that moved since backup. All pending
    // filesystem writes require fresh previews; none resume automatically.
    for query in [
        "UPDATE scrape_tasks SET state='interrupted' WHERE state IN ('queued','running','waiting_for_user')",
        "UPDATE scrape_runs SET state='interrupted' WHERE state IN ('queued','running','waiting_for_user')",
        "UPDATE file_operations SET state='rolled_back' WHERE state IN ('planned','approved','running','failed','interrupted')",
        "UPDATE music_tag_operations SET state='interrupted',error_code='restored_snapshot' WHERE state IN ('preview','running')",
        "UPDATE organization_operations SET state='interrupted',error_code='restored_snapshot' WHERE state IN ('preview','running')",
        "UPDATE adult_output_batches SET state='interrupted' WHERE state IN ('planned','running')",
        "UPDATE adult_output_batch_items SET state='interrupted' WHERE state IN ('planned','running')",
        "INSERT OR IGNORE INTO adult_revoked_output_previews(preview_id) SELECT id FROM output_previews WHERE json_extract(snapshot,'$.channel')='adult'",
        "UPDATE watcher_roots SET opted_in=0",
    ] {sqlx::query(query).execute(&mut *tx).await?;}
    tx.commit().await?;
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&mut conn)
        .await?;
    sqlx::query("PRAGMA journal_mode=DELETE")
        .execute(&mut conn)
        .await?;
    conn.close().await?;
    OpenOptions::new().write(true).open(path)?.sync_all()?;
    Ok(())
}
fn verify_published(
    parent: &DirectoryLease,
    name: &std::ffi::OsStr,
    expected: &CacheStamp,
) -> Result<()> {
    parent
        .verify()
        .map_err(|_| MaintenanceError::RecoveryRequired)?;
    let actual = cache_stamp(&parent.dir, name, &CancellationToken::new())
        .map_err(|_| MaintenanceError::RecoveryRequired)?;
    if &actual != expected {
        return Err(MaintenanceError::RecoveryRequired);
    }
    parent
        .verify()
        .map_err(|_| MaintenanceError::RecoveryRequired)?;
    Ok(())
}
fn read_json_cap<T: serde::de::DeserializeOwned>(
    parent: &cap_std::fs::Dir,
    name: &str,
) -> Result<T> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent.open_with(name, &options)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_JSON_BYTES {
        return Err(MaintenanceError::CorruptBackup);
    }
    let mut bytes = Vec::new();
    file.take(MAX_JSON_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_JSON_BYTES as usize {
        return Err(MaintenanceError::CorruptBackup);
    }
    serde_json::from_slice(&bytes).map_err(|_| MaintenanceError::CorruptBackup)
}
fn cache_files(
    cache: &DirectoryLease,
    token: &CancellationToken,
) -> Result<Vec<(PathBuf, CacheStamp)>> {
    cache.verify()?;
    let mut files = Vec::new();
    let mut total = 0u64;
    for (index, entry) in cache.dir.entries()?.enumerate() {
        check_cancel(token)?;
        if index >= MAX_CACHE_FILES {
            return Err(MaintenanceError::Storage);
        }
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(text) = name.to_str() else { continue };
        if !text.ends_with(".png") && !text.ends_with(".webp") && !text.ends_with(".jpg") {
            continue;
        }
        let stamp = cache_stamp(&cache.dir, &name, token)?;
        total = total.saturating_add(stamp.stamp.bytes);
        if total > 512 * 1024 * 1024 {
            return Err(MaintenanceError::Storage);
        }
        files.push((cache.path.join(name), stamp));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    cache.verify()?;
    Ok(files)
}
pub(crate) fn cache_stamp(
    parent: &cap_std::fs::Dir,
    name: &std::ffi::OsStr,
    token: &CancellationToken,
) -> Result<CacheStamp> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.share_mode(0x1 | 0x2);
    }
    let mut file = parent.open_with(name, &options)?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() > MAX_DATABASE_BYTES {
        return Err(MaintenanceError::Conflict);
    }
    let identity = directory_identity(&before)?;
    let stamp = Stamp {
        bytes: before.len(),
        modified_ns: before
            .modified()?
            .into_std()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| MaintenanceError::Storage)?
            .as_nanos(),
    };
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0;
    loop {
        check_cancel(token)?;
        if Instant::now() >= deadline {
            return Err(MaintenanceError::Cancelled);
        }
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        total += size as u64;
        if total > MAX_DATABASE_BYTES {
            return Err(MaintenanceError::Storage);
        }
        hasher.update(&buffer[..size]);
    }
    let after = file.metadata()?;
    if after.len() != before.len()
        || after.modified()? != before.modified()?
        || directory_identity(&after)? != identity
    {
        return Err(MaintenanceError::Conflict);
    }
    Ok(CacheStamp {
        stamp,
        identity,
        sha256: format!("{:x}", hasher.finalize()),
    })
}
#[cfg(test)]
type MaintenanceTestHook = Option<Box<dyn FnMut(&str, &Path)>>;
#[cfg(test)]
thread_local! {static TEST_HOOK:std::cell::RefCell<MaintenanceTestHook>=const{std::cell::RefCell::new(None)};}
#[cfg(test)]
fn run_hook(stage: &str, path: &Path) {
    TEST_HOOK.with(|hook| {
        if let Some(callback) = hook.borrow_mut().as_mut() {
            callback(stage, path);
        }
    });
}

#[cfg(test)]
mod tests;
