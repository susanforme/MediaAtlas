use super::*;
use crate::library::LibraryRepository;
async fn create_history(path: &Path, prefix: usize, crlf: bool) -> SqliteConnection {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .disable_statement_logging();
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    sqlx::migrate::Migrator::DEFAULT
        .run(&mut connection)
        .await
        .unwrap();
    for migration in canonical_migrator().iter().take(prefix) {
        let sql = if crlf {
            migration.sql.replace('\n', "\r\n")
        } else {
            migration.sql.to_string()
        };
        sqlx::raw_sql(&sql).execute(&mut connection).await.unwrap();
        sqlx::query("INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES (?,?,1,?,0)").bind(migration.version).bind(migration.description.as_ref()).bind(Sha384::digest(sql.as_bytes()).to_vec()).execute(&mut connection).await.unwrap();
    }
    connection
}
async fn checksum_rows(connection: &mut SqliteConnection) -> Vec<(i64, Vec<u8>)> {
    sqlx::query("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
        .fetch_all(connection)
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.get("version"), row.get("checksum")))
        .collect()
}
fn temporary() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = std::fs::canonicalize(directory.path())
        .unwrap()
        .join("legacy.sqlite");
    (directory, path)
}
#[test]
fn embedded_crlf_build_is_canonicalized_to_the_same_lf_sql_and_sha384() {
    let mut raw = sqlx::migrate::Migrator::DEFAULT;
    raw.migrations = Cow::Owned(
        canonical_migrator()
            .iter()
            .map(|m| {
                sqlx::migrate::Migration::new(
                    m.version,
                    m.description.clone(),
                    m.migration_type,
                    Cow::Owned(m.sql.replace('\n', "\r\n")),
                    m.no_tx,
                )
            })
            .collect(),
    );
    let canonical = canonicalize(&raw);
    for (actual, expected) in canonical.iter().zip(canonical_migrator().iter()) {
        assert_eq!(actual.sql, expected.sql);
        assert_eq!(actual.checksum, expected.checksum);
        assert!(!actual.sql.contains("\r\n"));
    }
}
#[tokio::test]
async fn historical_crlf_database_opens_with_data_and_preupgrade_snapshot_preserved() {
    let (_dir, path) = temporary();
    let mut connection = create_history(&path, 17, true).await;
    sqlx::query("INSERT INTO media_items(id,kind,title,created_at,updated_at) VALUES ('preserved','movie','synthetic user metadata',1,1)").execute(&mut connection).await.unwrap();
    let before = checksum_rows(&mut connection).await;
    connection.close().await.unwrap();
    let repository = LibraryRepository::open(&path).await.unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM media_items WHERE id='preserved'")
        .fetch_one(repository.pool())
        .await
        .unwrap();
    assert_eq!(title, "synthetic user metadata");
    let mut current = repository.pool().acquire().await.unwrap();
    let rows = checksum_rows(&mut current).await;
    for ((version, checksum), migration) in rows.iter().zip(canonical_migrator().iter()) {
        assert_eq!(*version, migration.version);
        assert_eq!(checksum.as_slice(), migration.checksum.as_ref());
    }
    let root = path.parent().unwrap().join(".mediaatlas-migration-backups");
    let snapshot = std::fs::read_dir(root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
        .join("before-upgrade.sqlite");
    let mut saved = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&snapshot)
            .read_only(true)
            .disable_statement_logging(),
    )
    .await
    .unwrap();
    assert_eq!(checksum_rows(&mut saved).await, before);
    let saved_title: String =
        sqlx::query_scalar("SELECT title FROM media_items WHERE id='preserved'")
            .fetch_one(&mut saved)
            .await
            .unwrap();
    assert_eq!(saved_title, title);
}
#[tokio::test]
async fn old_crlf_prefix_is_normalized_then_standard_migrations_reach_current_schema() {
    let (_dir, path) = temporary();
    create_history(&path, 4, true).await.close().await.unwrap();
    let repository = LibraryRepository::open(&path).await.unwrap();
    let version: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(repository.pool())
        .await
        .unwrap();
    assert_eq!(version, 21);
}
#[tokio::test]
async fn unknown_checksum_after_known_alias_causes_zero_rewrites_and_no_snapshot() {
    let (_dir, path) = temporary();
    let mut connection = create_history(&path, 4, true).await;
    sqlx::query("UPDATE _sqlx_migrations SET checksum=x'01' WHERE version=4")
        .execute(&mut connection)
        .await
        .unwrap();
    let before = checksum_rows(&mut connection).await;
    connection.close().await.unwrap();
    assert!(LibraryRepository::open(&path).await.is_err());
    let mut connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    assert_eq!(checksum_rows(&mut connection).await, before);
    assert!(!path
        .parent()
        .unwrap()
        .join(".mediaatlas-migration-backups")
        .exists());
}
#[tokio::test]
async fn schema_alteration_with_whitelisted_checksums_is_rejected_without_rewrite() {
    let (_dir, path) = temporary();
    let mut connection = create_history(&path, 4, true).await;
    sqlx::query("ALTER TABLE media_items ADD COLUMN external_schema_change TEXT")
        .execute(&mut connection)
        .await
        .unwrap();
    let before = checksum_rows(&mut connection).await;
    connection.close().await.unwrap();
    assert!(LibraryRepository::open(&path).await.is_err());
    let mut connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    assert_eq!(checksum_rows(&mut connection).await, before);
    assert!(!path
        .parent()
        .unwrap()
        .join(".mediaatlas-migration-backups")
        .exists());
}
#[tokio::test]
async fn extra_trigger_and_index_are_not_treated_as_the_official_schema() {
    for ddl in [
        "CREATE INDEX unexpected_index ON media_items(title)",
        "CREATE TRIGGER unexpected_trigger AFTER INSERT ON media_items BEGIN SELECT 1; END",
    ] {
        let (_dir, path) = temporary();
        let mut connection = create_history(&path, 4, true).await;
        sqlx::query(ddl).execute(&mut connection).await.unwrap();
        let before = checksum_rows(&mut connection).await;
        assert!(matches!(
            normalize_candidate(&mut connection).await,
            Err(CompatibilityError::InvalidHistory)
        ));
        assert_eq!(checksum_rows(&mut connection).await, before);
    }
}
#[tokio::test]
async fn future_failed_and_nonprefix_histories_are_rejected_without_rewrite() {
    for mutation in ["INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES (99,'unknown',1,x'01',0)","UPDATE _sqlx_migrations SET success=0 WHERE version=4","DELETE FROM _sqlx_migrations WHERE version=2"] {
        let (_dir,path)=temporary();let mut connection=create_history(&path,4,true).await;sqlx::query(mutation).execute(&mut connection).await.unwrap();let before=checksum_rows(&mut connection).await;assert!(normalize_candidate(&mut connection).await.is_err());assert_eq!(checksum_rows(&mut connection).await,before);
    }
}
#[tokio::test]
async fn canonical_database_and_fresh_database_do_not_create_upgrade_snapshots() {
    let (_dir, path) = temporary();
    let repository = LibraryRepository::open(&path).await.unwrap();
    repository.pool().close().await;
    let repository = LibraryRepository::open(&path).await.unwrap();
    assert!(!path
        .parent()
        .unwrap()
        .join(".mediaatlas-migration-backups")
        .exists());
    assert!(repository.pool().size() > 0);
}
#[tokio::test]
async fn changed_checksum_between_inspection_and_transaction_is_not_rewritten() {
    let (_dir, path) = temporary();
    let mut connection = create_history(&path, 4, true).await;
    let before = validate_history(&mut connection).await.unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum=x'01' WHERE version=4")
        .execute(&mut connection)
        .await
        .unwrap();
    let changed = checksum_rows(&mut connection).await;
    assert!(normalize_transaction(&mut connection, &before, None)
        .await
        .is_err());
    assert_eq!(checksum_rows(&mut connection).await, changed);
}
#[tokio::test]
async fn history_table_schema_tamper_and_empty_history_unknown_schema_are_rejected() {
    for prefix in [0, 4] {
        let (_dir, path) = temporary();
        let mut connection = create_history(&path, prefix, true).await;
        sqlx::query("ALTER TABLE _sqlx_migrations ADD COLUMN unexpected INTEGER")
            .execute(&mut connection)
            .await
            .unwrap();
        let before = checksum_rows(&mut connection).await;
        assert!(normalize_candidate(&mut connection).await.is_err());
        assert_eq!(checksum_rows(&mut connection).await, before);
    }
    let (_dir, path) = temporary();
    let mut connection = create_history(&path, 0, false).await;
    sqlx::query("CREATE TABLE unexpected(bytes BLOB)")
        .execute(&mut connection)
        .await
        .unwrap();
    assert!(normalize_candidate(&mut connection).await.is_err());
    sqlx::query("DROP TABLE _sqlx_migrations")
        .execute(&mut connection)
        .await
        .unwrap();
    assert!(normalize_candidate(&mut connection).await.is_err());
}
#[tokio::test]
async fn replaced_preupgrade_snapshot_causes_zero_checksum_rewrites() {
    let (_dir, path) = temporary();
    let mut connection = create_history(&path, 4, true).await;
    let before = checksum_rows(&mut connection).await;
    connection.close().await.unwrap();
    TEST_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(|stage, path| {
            if stage == "snapshot_before_normalize" {
                std::fs::rename(path, path.with_extension("preserved.sqlite")).unwrap();
                std::fs::write(path, b"external replacement snapshot").unwrap();
            }
        }))
    });
    let result = LibraryRepository::open(&path).await;
    TEST_HOOK.with(|hook| hook.borrow_mut().take());
    assert!(result.is_err());
    let mut connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    assert_eq!(checksum_rows(&mut connection).await, before);
}
#[tokio::test]
async fn empty_official_history_table_is_a_supported_fresh_prefix() {
    let (_dir, path) = temporary();
    let mut connection = create_history(&path, 0, false).await;
    assert!(!normalize_candidate(&mut connection).await.unwrap());
    connection.close().await.unwrap();
    assert!(LibraryRepository::open(&path).await.is_ok());
    assert!(!path
        .parent()
        .unwrap()
        .join(".mediaatlas-migration-backups")
        .exists());
}
