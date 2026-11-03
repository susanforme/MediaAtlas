//! Exact LF/CRLF compatibility for the same known SQLx migration DDL.
use crate::maintenance::{cache_stamp, CacheStamp, DirectoryLease};
use sha2::{Digest, Sha384};
use sqlx::{
    sqlite::SqliteConnectOptions, ConnectOptions, Connection, Row, SqliteConnection, SqlitePool,
};
use std::{
    borrow::Cow,
    path::Path,
    sync::LazyLock,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

static RAW_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
static CANONICAL_MIGRATOR: LazyLock<sqlx::migrate::Migrator> =
    LazyLock::new(|| canonicalize(&RAW_MIGRATOR));
fn canonicalize(raw: &sqlx::migrate::Migrator) -> sqlx::migrate::Migrator {
    let migrations = raw
        .iter()
        .map(|migration| {
            sqlx::migrate::Migration::new(
                migration.version,
                migration.description.clone(),
                migration.migration_type,
                Cow::Owned(migration.sql.replace("\r\n", "\n")),
                migration.no_tx,
            )
        })
        .collect::<Vec<_>>();
    // SQLx 0.8's public constructor fields are used by its own migrate! macro.
    sqlx::migrate::Migrator {
        migrations: Cow::Owned(migrations),
        ..sqlx::migrate::Migrator::DEFAULT
    }
}
pub(crate) fn canonical_migrator() -> &'static sqlx::migrate::Migrator {
    &CANONICAL_MIGRATOR
}
const MAX_DATABASE_BYTES: i64 = 1024 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompatibilityError {
    InvalidHistory,
    FutureSchema,
    Changed,
    Database,
    Storage,
}
impl From<sqlx::Error> for CompatibilityError {
    fn from(_: sqlx::Error) -> Self {
        Self::Database
    }
}
impl From<std::io::Error> for CompatibilityError {
    fn from(_: std::io::Error) -> Self {
        Self::Storage
    }
}
type Result<T> = std::result::Result<T, CompatibilityError>;
#[derive(Debug, Clone, PartialEq, Eq)]
struct Applied {
    version: i64,
    checksum: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MigrationHistory {
    pub(crate) last_version: i64,
    pub(crate) requires_normalization: bool,
    applied: Vec<Applied>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct SchemaEntry {
    kind: String,
    name: String,
    table: String,
    sql: String,
}

/// Read-only: imported backup bytes and checksums are never normalized in place.
pub(crate) async fn validate_history(
    connection: &mut SqliteConnection,
) -> Result<MigrationHistory> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations')",
    )
    .fetch_one(&mut *connection)
    .await?;
    if !exists {
        if !schema_entries(connection).await?.is_empty() {
            return Err(CompatibilityError::InvalidHistory);
        }
        return Ok(MigrationHistory {
            last_version: 0,
            requires_normalization: false,
            applied: vec![],
        });
    }
    let rows = sqlx::query(
        "SELECT version,checksum,success FROM _sqlx_migrations ORDER BY version LIMIT 1024",
    )
    .fetch_all(&mut *connection)
    .await?;
    if rows.len() > canonical_migrator().iter().count() {
        return Err(CompatibilityError::FutureSchema);
    }
    let mut applied = Vec::new();
    let mut requires_normalization = false;
    for (row, migration) in rows.iter().zip(canonical_migrator().iter()) {
        let version: i64 = row.try_get("version")?;
        if version
            > canonical_migrator()
                .iter()
                .map(|m| m.version)
                .max()
                .unwrap_or(0)
        {
            return Err(CompatibilityError::FutureSchema);
        }
        if version != migration.version || row.try_get::<i64, _>("success")? != 1 {
            return Err(CompatibilityError::InvalidHistory);
        }
        let crlf = Sha384::digest(migration.sql.replace('\n', "\r\n").as_bytes()).to_vec();
        let checksum: Vec<u8> = row.try_get("checksum")?;
        if checksum.as_slice() == migration.checksum.as_ref() {
        } else if checksum == crlf {
            requires_normalization = true;
        } else {
            return Err(CompatibilityError::InvalidHistory);
        }
        applied.push(Applied { version, checksum });
    }
    if requires_normalization || rows.is_empty() {
        validate_integrity(connection).await?;
        validate_schema(connection, applied.len()).await?;
    }
    Ok(MigrationHistory {
        last_version: applied.last().map_or(0, |m| m.version),
        requires_normalization,
        applied,
    })
}
async fn validate_integrity(connection: &mut SqliteConnection) -> Result<()> {
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check(1)")
        .fetch_one(&mut *connection)
        .await?;
    if integrity != "ok"
        || sqlx::query("PRAGMA foreign_key_check")
            .fetch_optional(&mut *connection)
            .await?
            .is_some()
    {
        return Err(CompatibilityError::InvalidHistory);
    }
    Ok(())
}
async fn schema_entries(connection: &mut SqliteConnection) -> Result<Vec<SchemaEntry>> {
    let rows=sqlx::query("SELECT type,name,tbl_name,substr(sql,1,65537) AS sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT GLOB 'sqlite_*' ORDER BY type,name,tbl_name LIMIT 513").fetch_all(connection).await?;
    if rows.len() > 512 {
        return Err(CompatibilityError::InvalidHistory);
    }
    rows.into_iter()
        .map(|row| {
            let sql: String = row.try_get("sql")?;
            if sql.len() > 65536 {
                return Err(CompatibilityError::InvalidHistory);
            }
            Ok(SchemaEntry {
                kind: row.try_get("type")?,
                name: row.try_get("name")?,
                table: row.try_get("tbl_name")?,
                sql: sql.replace("\r\n", "\n"),
            })
        })
        .collect()
}
async fn validate_schema(connection: &mut SqliteConnection, prefix: usize) -> Result<()> {
    let options = SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(true)
        .disable_statement_logging();
    let mut expected = SqliteConnection::connect_with(&options).await?;
    install_budget(&mut expected).await?;
    let reference = sqlx::migrate::Migrator {
        migrations: Cow::Owned(canonical_migrator().iter().take(prefix).cloned().collect()),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    reference
        .run_direct(&mut expected)
        .await
        .map_err(|_| CompatibilityError::InvalidHistory)?;
    let actual = schema_entries(connection).await?;
    let expected_entries = schema_entries(&mut expected).await?;
    expected.close().await?;
    if actual != expected_entries {
        return Err(CompatibilityError::InvalidHistory);
    }
    Ok(())
}
async fn install_budget(connection: &mut SqliteConnection) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(120);
    connection
        .lock_handle()
        .await?
        .set_progress_handler(4096, move || Instant::now() < deadline);
    Ok(())
}
async fn normalize_transaction(
    connection: &mut SqliteConnection,
    before: &MigrationHistory,
    snapshot: Option<&SnapshotProof>,
) -> Result<bool> {
    if !before.requires_normalization {
        return Ok(false);
    }
    let mut transaction = connection.begin().await?;
    sqlx::query("UPDATE _sqlx_migrations SET version=version WHERE 0")
        .execute(&mut *transaction)
        .await?;
    let current = validate_history(&mut transaction).await?;
    if &current != before {
        return Err(CompatibilityError::Changed);
    }
    if let Some(snapshot) = snapshot {
        snapshot.verify()?;
    }
    for (applied, migration) in current.applied.iter().zip(canonical_migrator().iter()) {
        if applied.checksum.as_slice() == migration.checksum.as_ref() {
            continue;
        }
        let changed = sqlx::query(
            "UPDATE _sqlx_migrations SET checksum=? WHERE version=? AND checksum=? AND success=1",
        )
        .bind(migration.checksum.as_ref())
        .bind(applied.version)
        .bind(&applied.checksum)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(CompatibilityError::Changed);
        }
    }
    transaction.commit().await?;
    Ok(true)
}
struct SnapshotProof {
    directory: DirectoryLease,
    stamp: CacheStamp,
}
impl SnapshotProof {
    fn verify(&self) -> Result<()> {
        self.directory
            .verify()
            .map_err(|_| CompatibilityError::Changed)?;
        let actual = cache_stamp(
            &self.directory.dir,
            std::ffi::OsStr::new("before-upgrade.sqlite"),
            &CancellationToken::new(),
        )
        .map_err(|_| CompatibilityError::Changed)?;
        if actual != self.stamp {
            return Err(CompatibilityError::Changed);
        }
        Ok(())
    }
}
#[cfg(test)]
type CompatibilityTestHook = Option<Box<dyn FnMut(&str, &Path)>>;
#[cfg(test)]
thread_local! {static TEST_HOOK:std::cell::RefCell<CompatibilityTestHook>=const{std::cell::RefCell::new(None)};}
#[cfg(test)]
fn run_hook(stage: &str, path: &Path) {
    TEST_HOOK.with(|hook| {
        if let Some(callback) = hook.borrow_mut().as_mut() {
            callback(stage, path);
        }
    });
}

/// Only the live-database path creates a consistent, private pre-upgrade snapshot.
pub(crate) async fn normalize_current_database(pool: &SqlitePool, database: &Path) -> Result<bool> {
    let mut connection = pool.acquire().await?;
    install_budget(&mut connection).await?;
    let result = normalize_current_inner(&mut connection, database).await;
    connection.lock_handle().await?.remove_progress_handler();
    connection.return_to_pool().await;
    result
}
async fn normalize_current_inner(
    connection: &mut SqliteConnection,
    database: &Path,
) -> Result<bool> {
    let before = validate_history(connection).await?;
    if !before.requires_normalization {
        return Ok(false);
    }
    let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(&mut *connection)
        .await?;
    let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(&mut *connection)
        .await?;
    if pages.saturating_mul(page_size) > MAX_DATABASE_BYTES {
        return Err(CompatibilityError::Storage);
    }
    let parent = std::fs::canonicalize(database.parent().ok_or(CompatibilityError::Storage)?)?;
    let parent_lease = DirectoryLease::acquire(&parent).map_err(|_| CompatibilityError::Storage)?;
    let root = parent.join(".mediaatlas-migration-backups");
    match parent_lease.dir.create_dir(".mediaatlas-migration-backups") {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(CompatibilityError::Storage),
    }
    let root_lease = DirectoryLease::acquire(&root).map_err(|_| CompatibilityError::Storage)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        root_lease.dir.set_permissions(
            ".",
            cap_std::fs::Permissions::from_std(std::fs::Permissions::from_mode(0o700)),
        )?;
    }
    let id = Uuid::new_v4().to_string();
    root_lease.dir.create_dir(&id)?;
    let snapshot_directory = root.join(&id);
    let snapshot_lease =
        DirectoryLease::acquire(&snapshot_directory).map_err(|_| CompatibilityError::Storage)?;
    let snapshot = snapshot_directory.join("before-upgrade.sqlite");
    parent_lease
        .verify()
        .map_err(|_| CompatibilityError::Changed)?;
    root_lease
        .verify()
        .map_err(|_| CompatibilityError::Changed)?;
    snapshot_lease
        .verify()
        .map_err(|_| CompatibilityError::Changed)?;
    sqlx::query("VACUUM INTO ?")
        .bind(snapshot.to_str().ok_or(CompatibilityError::Storage)?)
        .execute(&mut *connection)
        .await?;
    snapshot_lease
        .verify()
        .map_err(|_| CompatibilityError::Changed)?;
    root_lease
        .verify()
        .map_err(|_| CompatibilityError::Changed)?;
    parent_lease
        .verify()
        .map_err(|_| CompatibilityError::Changed)?;
    let file = snapshot_lease.dir.open_with(
        "before-upgrade.sqlite",
        cap_std::fs::OpenOptions::new().write(true),
    )?;
    file.sync_all()?;
    // Persist the file entry, its newly created UUID directory, and the upgrade
    // namespace before committing any live migration metadata changes.
    #[cfg(unix)]
    for directory in [&snapshot_lease, &root_lease, &parent_lease] {
        use cap_fs_ext::OpenOptionsMaybeDirExt;
        let mut options = cap_std::fs::OpenOptions::new();
        options.read(true).maybe_dir(true);
        // cap-std directory handles can be O_PATH on Linux. Open the held
        // directory itself for reading so fsync receives a usable descriptor.
        directory.dir.open_with(".", &options)?.sync_all()?;
    }
    let stamp = cache_stamp(
        &snapshot_lease.dir,
        std::ffi::OsStr::new("before-upgrade.sqlite"),
        &CancellationToken::new(),
    )
    .map_err(|_| CompatibilityError::Storage)?;
    let mut saved = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&snapshot)
            .read_only(true)
            .disable_statement_logging(),
    )
    .await?;
    install_budget(&mut saved).await?;
    validate_integrity(&mut saved).await?;
    if validate_history(&mut saved).await? != before {
        return Err(CompatibilityError::Changed);
    }
    saved.close().await?;
    let proof = SnapshotProof {
        directory: snapshot_lease,
        stamp,
    };
    proof.verify()?;
    #[cfg(test)]
    run_hook("snapshot_before_normalize", &snapshot);
    normalize_transaction(connection, &before, Some(&proof)).await
}
/// Operates only on an already copied, hash-validated restore candidate.
pub(crate) async fn normalize_candidate(connection: &mut SqliteConnection) -> Result<bool> {
    install_budget(connection).await?;
    let result = async {
        let before = validate_history(connection).await?;
        normalize_transaction(connection, &before, None).await
    }
    .await;
    connection.lock_handle().await?.remove_progress_handler();
    result
}

#[cfg(test)]
mod tests;
