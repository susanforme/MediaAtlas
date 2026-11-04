use super::*;
async fn fixture() -> (
    tempfile::TempDir,
    LibraryRepository,
    MaintenanceService,
    DatabaseLease,
) {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    let db = base.join("library.sqlite");
    let lease = DatabaseLease::acquire(&db).unwrap();
    let repo = LibraryRepository::open(&db).await.unwrap();
    let service = MaintenanceService::new(
        repo.clone(),
        db,
        base.join("maintenance"),
        base.join("cache"),
    )
    .unwrap();
    (dir, repo, service, lease)
}
async fn backup(service: &MaintenanceService) -> BackupSummary {
    let preview = service.backup_preview(None).await.unwrap();
    service.backup_confirm(preview.id, None).await.unwrap()
}
#[tokio::test]
async fn vacuum_snapshot_includes_committed_wal_and_immutable_manifest() {
    let (_dir, repo, service, _lease) = fixture().await;
    sqlx::query("INSERT INTO media_items(id,kind,title,created_at,updated_at) VALUES ('synthetic','movie','committed WAL',1,1)").execute(repo.pool()).await.unwrap();
    let summary = backup(&service).await;
    let manifest = validate_backup(&service.directory, summary.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(manifest.summary.schema_version, 20);
    let mut snapshot = open_connection(
        &backup_path(&service.directory, summary.id).join("library.sqlite"),
        true,
    )
    .await
    .unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM media_items WHERE id='synthetic'")
        .fetch_one(&mut snapshot)
        .await
        .unwrap();
    assert_eq!(title, "committed WAL");
    assert!(service.backup_confirm(summary.id, None).await.is_err());
}
#[tokio::test]
async fn private_backup_requires_grant_and_old_epoch_stays_revoked() {
    let (_dir, repo, service, _lease) = fixture().await;
    sqlx::query("INSERT INTO media_items(id,kind,title,adult_isolated,created_at,updated_at) VALUES ('private','adult_video','PRIVATE-MARKER',1,1,1)").execute(repo.pool()).await.unwrap();
    assert!(matches!(
        service.backup_preview(None).await,
        Err(MaintenanceError::PrivacyLocked)
    ));
    let grant = AdultOutputAccess::new();
    let p = service.backup_preview(Some(grant.clone())).await.unwrap();
    grant.revoke();
    assert!(matches!(
        service
            .backup_confirm(p.id, Some(AdultOutputAccess::new()))
            .await,
        Err(MaintenanceError::PrivacyLocked)
    ));
    let p = service
        .backup_preview(Some(AdultOutputAccess::new()))
        .await
        .unwrap();
    let summary = service
        .backup_confirm(p.id, Some(AdultOutputAccess::new()))
        .await
        .unwrap();
    assert!(summary.contains_private_metadata);
    assert!(matches!(
        service.restore_preview(summary.id, None).await,
        Err(MaintenanceError::PrivacyLocked)
    ));
}
#[tokio::test]
async fn privacy_created_after_preview_is_rechecked_and_failed_snapshot_not_exported() {
    let (_dir, repo, service, _lease) = fixture().await;
    let preview = service.backup_preview(None).await.unwrap();
    sqlx::query("UPDATE adult_privacy_settings SET entry_visible=1")
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(matches!(
        service.backup_confirm(preview.id, None).await,
        Err(MaintenanceError::PrivacyLocked)
    ));
    assert!(service.status().await.unwrap().backups.is_empty());
}
#[tokio::test]
async fn checksum_future_version_manifest_lie_and_migration_tamper_rejected() {
    let (_dir, _repo, service, _lease) = fixture().await;
    let summary = backup(&service).await;
    let path = backup_path(&service.directory, summary.id);
    let manifest_path = path.join("manifest.json");
    let original = fs::read(&manifest_path).unwrap();
    let mut manifest: Manifest = read_json(&manifest_path).unwrap();
    manifest.summary.app_version = "999.0.0".into();
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(matches!(
        service.restore_preview(summary.id, None).await,
        Err(MaintenanceError::UnsupportedVersion)
    ));
    fs::write(&manifest_path, &original).unwrap();
    let mut manifest: Manifest = read_json(&manifest_path).unwrap();
    manifest.summary.contains_private_metadata = true;
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(matches!(
        service.restore_preview(summary.id, None).await,
        Err(MaintenanceError::CorruptBackup)
    ));
    fs::write(&manifest_path, &original).unwrap();
    let database = path.join("library.sqlite");
    let mut conn = open_connection(&database, false).await.unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum=x'01' WHERE version=1")
        .execute(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();
    let mut manifest: Manifest = read_json(&manifest_path).unwrap();
    manifest.sha256 = hash_file(&database, CancellationToken::new())
        .await
        .unwrap();
    manifest.summary.bytes = fs::metadata(&database).unwrap().len();
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(matches!(
        service.restore_preview(summary.id, None).await,
        Err(MaintenanceError::CorruptBackup)
    ));
}
#[tokio::test]
async fn queued_restore_waits_for_restart_preserves_current_snapshot_and_source_media() {
    let (dir, repo, service, lease) = fixture().await;
    let summary = backup(&service).await;
    let source = dir.path().join("source.synthetic");
    fs::write(&source, b"unchanged source media").unwrap();
    sqlx::query("INSERT INTO media_items(id,kind,title,created_at,updated_at) VALUES ('new','movie','after backup',1,1)").execute(repo.pool()).await.unwrap();
    let p = service.restore_preview(summary.id, None).await.unwrap();
    service.restore_confirm(p.id, None).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM media_items")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    repo.pool().close().await;
    assert!(lease
        .apply_pending_restore(&service.directory)
        .await
        .unwrap());
    assert!(!lease
        .apply_pending_restore(&service.directory)
        .await
        .unwrap());
    let restored = LibraryRepository::open(service.database_path())
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM media_items")
        .fetch_one(restored.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    let work = fs::read_dir(&service.directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("restore-")
        })
        .unwrap();
    let mut before = open_connection(&work.join("before-restore.sqlite"), true)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM media_items")
        .fetch_one(&mut before)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(fs::read(source).unwrap(), b"unchanged source media");
}
#[tokio::test]
async fn single_instance_lease_cannot_be_reacquired_until_drop() {
    let (_dir, repo, service, lease) = fixture().await;
    assert!(matches!(
        DatabaseLease::acquire(service.database_path()),
        Err(MaintenanceError::Busy)
    ));
    repo.pool().close().await;
    drop(lease);
    assert!(DatabaseLease::acquire(service.database_path()).is_ok());
}
#[tokio::test]
async fn diagnostics_allowlist_excludes_raw_private_user_network_and_auth_values() {
    let (_dir, repo, service, _lease) = fixture().await;
    sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES ('private','search_providers','failed','{\"adult\":true,\"query\":\"SECRET-PRIVATE-MARKER https://host/token\"}',1,1)").execute(repo.pool()).await.unwrap();
    let result = service.diagnostics_export().await.unwrap();
    let bytes = fs::read(result.path).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("SECRET"));
    assert!(!text.contains("token"));
    assert!(!text.contains("host"));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(value["database_pages"].as_i64().unwrap() > 0);
    assert_eq!(value["applied_migrations"], 20);
    assert_eq!(value["includes_credentials"], false);
}
#[tokio::test]
async fn cache_preview_is_stale_safe_and_never_traverses_subdirectories() {
    let (_dir, _repo, service, _lease) = fixture().await;
    fs::write(service.cache_lease.path.join("abc.png"), b"cache").unwrap();
    fs::create_dir(service.cache_lease.path.join("private")).unwrap();
    fs::write(service.cache_lease.path.join("private/def.png"), b"private").unwrap();
    let p = service.cache_preview().await.unwrap();
    assert_eq!(p.files, 1);
    fs::write(service.cache_lease.path.join("abc.png"), b"external-change").unwrap();
    assert!(matches!(
        service.cache_confirm(p.id).await,
        Err(MaintenanceError::Conflict)
    ));
    let p = service.cache_preview().await.unwrap();
    assert_eq!(service.cache_confirm(p.id).await.unwrap().removed_files, 1);
    assert!(service.cache_lease.path.join("private/def.png").exists());
}
#[tokio::test]
async fn restored_pending_plans_never_replay_and_watchers_require_fresh_opt_in() {
    let (_dir, repo, service, lease) = fixture().await;
    sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES ('pending','write_audio_tags','running','{}',1,1)").execute(repo.pool()).await.unwrap();
    sqlx::query("INSERT INTO file_operations(id,plan,state,created_at,updated_at) VALUES ('pending','{}','planned',1,1)").execute(repo.pool()).await.unwrap();
    let summary = backup(&service).await;
    let p = service.restore_preview(summary.id, None).await.unwrap();
    service.restore_confirm(p.id, None).await.unwrap();
    repo.pool().close().await;
    lease
        .apply_pending_restore(&service.directory)
        .await
        .unwrap();
    let restored = LibraryRepository::open(service.database_path())
        .await
        .unwrap();
    let task: String = sqlx::query_scalar("SELECT state FROM scrape_tasks")
        .fetch_one(restored.pool())
        .await
        .unwrap();
    let op: String = sqlx::query_scalar("SELECT state FROM file_operations")
        .fetch_one(restored.pool())
        .await
        .unwrap();
    assert_eq!(task, "interrupted");
    assert_eq!(op, "rolled_back");
}
#[tokio::test]
async fn cancelled_hash_and_standalone_backup_sidecars_are_rejected() {
    let (_dir, _repo, service, _lease) = fixture().await;
    let summary = backup(&service).await;
    let path = backup_path(&service.directory, summary.id).join("library.sqlite");
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        hash_file(&path, token).await,
        Err(MaintenanceError::Cancelled)
    ));
    fs::write(sidecar_path(&path, "-wal"), b"external WAL").unwrap();
    assert!(matches!(
        service.restore_preview(summary.id, None).await,
        Err(MaintenanceError::Conflict)
    ));
}

#[tokio::test]
async fn cache_same_size_same_mtime_replacement_is_not_approved() {
    let (_dir, _repo, service, _lease) = fixture().await;
    let path = service.cache_lease.path.join("abc.png");
    fs::write(&path, b"old").unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let preview = service.cache_preview().await.unwrap();
    fs::write(&path, b"new").unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert!(matches!(
        service.cache_confirm(preview.id).await,
        Err(MaintenanceError::Conflict)
    ));
    assert_eq!(fs::read(path).unwrap(), b"new");
}
#[tokio::test]
async fn cache_changed_at_quarantine_boundary_preserves_both_objects() {
    let (_dir, _repo, service, _lease) = fixture().await;
    let path = service.cache_lease.path.join("abc.png");
    fs::write(&path, b"previewed").unwrap();
    let preview = service.cache_preview().await.unwrap();
    TEST_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(|stage, path| {
            if stage == "cache_before_quarantine" {
                fs::rename(path, path.with_extension("preserved")).unwrap();
                fs::write(path, b"external").unwrap();
            }
        }))
    });
    let result = service.cache_confirm(preview.id).await;
    TEST_HOOK.with(|hook| hook.borrow_mut().take());
    assert!(matches!(result, Err(MaintenanceError::Conflict)));
    assert_eq!(fs::read(&path).unwrap(), b"external");
    assert_eq!(
        fs::read(path.with_extension("preserved")).unwrap(),
        b"previewed"
    );
}
#[tokio::test]
async fn restore_quarantine_collision_never_overwrites_foreign_bytes() {
    let (_dir, repo, service, lease) = fixture().await;
    let summary = backup(&service).await;
    let p = service.restore_preview(summary.id, None).await.unwrap();
    service.restore_confirm(p.id, None).await.unwrap();
    repo.pool().close().await;
    TEST_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(|stage, path| {
            if stage == "restore_before_quarantine" {
                fs::write(path, b"foreign collision").unwrap();
            }
        }))
    });
    let result = lease.apply_pending_restore(&service.directory).await;
    TEST_HOOK.with(|hook| hook.borrow_mut().take());
    assert!(matches!(result, Err(MaintenanceError::Conflict)));
    assert_eq!(
        hash_file(service.database_path(), CancellationToken::new())
            .await
            .unwrap(),
        read_json::<RestoreJournal>(&service.directory.join("restore-journal.json"))
            .unwrap()
            .original_sha256
    );
    let work = fs::read_dir(&service.directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.is_dir()
                && p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("restore-")
        })
        .unwrap();
    assert_eq!(
        fs::read(work.join("original.sqlite")).unwrap(),
        b"foreign collision"
    );
    assert!(!lease
        .apply_pending_restore(&service.directory)
        .await
        .unwrap());
    assert!(!service.directory.join("pending-restore.json").exists());
}
#[tokio::test]
async fn interrupted_restore_rolls_back_exact_original_without_replaying_plan() {
    let (_dir, repo, service, lease) = fixture().await;
    repo.pool().close().await;
    let original_sha256 = hash_file(service.database_path(), CancellationToken::new())
        .await
        .unwrap();
    let id = Uuid::new_v4();
    let work = service.directory.join(format!("restore-{id}"));
    fs::create_dir(&work).unwrap();
    fs::rename(service.database_path(), work.join("original.sqlite")).unwrap();
    write_json_new(
        &service.directory.join("restore-journal.json"),
        &RestoreJournal {
            format_version: 1,
            id,
            original_sha256: original_sha256.clone(),
            candidate_sha256: "unpublished".into(),
        },
    )
    .unwrap();
    fs::write(
        service.directory.join("pending-restore.json"),
        b"interrupted user confirmation retained",
    )
    .unwrap();
    assert!(!lease
        .apply_pending_restore(&service.directory)
        .await
        .unwrap());
    assert_eq!(
        hash_file(service.database_path(), CancellationToken::new())
            .await
            .unwrap(),
        original_sha256
    );
    assert!(work.join("interrupted-plan.json").exists());
    assert!(!lease
        .apply_pending_restore(&service.directory)
        .await
        .unwrap());
}
#[tokio::test]
async fn restored_cross_machine_auth_has_only_opaque_refs_and_truthfully_missing_secret() {
    use crate::{
        credentials::CredentialStore,
        provider_auth::{AuthSaveInput, ProviderAuthService},
    };
    let (_dir, repo, service, lease) = fixture().await;
    let auth = ProviderAuthService::new(repo.clone(), Arc::new(CredentialStore::new()));
    let secret = "SYNTHETIC-SESSION-SECRET-NEVER-IN-SNAPSHOT";
    assert!(
        auth.save(AuthSaveInput {
            provider_id: "tmdb".into(),
            method: "bearer".into(),
            storage: "session_only".into(),
            secret: secret.into()
        })
        .await
        .unwrap()
        .configured
    );
    let summary = backup(&service).await;
    let bytes =
        fs::read(backup_path(&service.directory, summary.id).join("library.sqlite")).unwrap();
    assert!(!bytes.windows(secret.len()).any(|s| s == secret.as_bytes()));
    let p = service.restore_preview(summary.id, None).await.unwrap();
    service.restore_confirm(p.id, None).await.unwrap();
    repo.pool().close().await;
    lease
        .apply_pending_restore(&service.directory)
        .await
        .unwrap();
    let restored = LibraryRepository::open(service.database_path())
        .await
        .unwrap();
    let reference: String = sqlx::query_scalar(
        "SELECT credential_ref FROM provider_auth_settings WHERE provider_id='tmdb'",
    )
    .fetch_one(restored.pool())
    .await
    .unwrap();
    assert!(reference.starts_with("tmdb.auth.bearer.session."));
    let status = ProviderAuthService::new(restored, Arc::new(CredentialStore::new()))
        .status("tmdb")
        .await
        .unwrap();
    assert!(!status.configured);
    assert_eq!(status.status, "missing_credentials");
}
#[tokio::test]
async fn relational_corruption_is_rejected_even_with_matching_checksum() {
    let (_dir, _repo, service, _lease) = fixture().await;
    let summary = backup(&service).await;
    let path = backup_path(&service.directory, summary.id);
    let database = path.join("library.sqlite");
    let mut conn = open_connection(&database, false).await.unwrap();
    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&mut conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO external_ids(id,media_item_id,provider_id,namespace,external_id,is_primary,created_at) VALUES ('invalid','missing','tmdb','movie','1',1,1)").execute(&mut conn).await.unwrap();
    conn.close().await.unwrap();
    let manifest_path = path.join("manifest.json");
    let mut manifest: Manifest = read_json(&manifest_path).unwrap();
    manifest.sha256 = hash_file(&database, CancellationToken::new())
        .await
        .unwrap();
    manifest.summary.bytes = fs::metadata(&database).unwrap().len();
    fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(matches!(
        service.restore_preview(summary.id, None).await,
        Err(MaintenanceError::CorruptBackup)
    ));
}
#[tokio::test]
async fn lock_retires_confirmed_private_restore_without_restoring_grant() {
    let (_dir, repo, service, lease) = fixture().await;
    sqlx::query("UPDATE adult_privacy_settings SET entry_visible=1")
        .execute(repo.pool())
        .await
        .unwrap();
    let grant = AdultOutputAccess::new();
    let p = service.backup_preview(Some(grant.clone())).await.unwrap();
    let summary = service
        .backup_confirm(p.id, Some(grant.clone()))
        .await
        .unwrap();
    let p = service
        .restore_preview(summary.id, Some(grant.clone()))
        .await
        .unwrap();
    service
        .restore_confirm(p.id, Some(grant.clone()))
        .await
        .unwrap();
    grant.revoke();
    assert!(service.revoke_private_restore().await.unwrap());
    assert!(!service.status().await.unwrap().restore_pending);
    repo.pool().close().await;
    assert!(!lease
        .apply_pending_restore(&service.directory)
        .await
        .unwrap());
}
#[tokio::test]
async fn ordinary_queued_restore_cannot_snapshot_private_data_added_after_confirmation() {
    let (_dir, repo, service, lease) = fixture().await;
    let summary = backup(&service).await;
    let p = service.restore_preview(summary.id, None).await.unwrap();
    service.restore_confirm(p.id, None).await.unwrap();
    sqlx::query("UPDATE adult_privacy_settings SET entry_visible=1")
        .execute(repo.pool())
        .await
        .unwrap();
    repo.pool().close().await;
    assert!(matches!(
        lease.apply_pending_restore(&service.directory).await,
        Err(MaintenanceError::PrivacyLocked)
    ));
    assert!(service.database_path().exists());
    assert!(!service.directory.join("restore-journal.json").exists());
}
#[tokio::test]
async fn cancellation_interrupts_actual_sqlite_vacuum_and_keeps_source_usable() {
    let (_dir, repo, service, _lease) = fixture().await;
    sqlx::query("CREATE TABLE synthetic_maintenance_payload(bytes BLOB)")
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO synthetic_maintenance_payload VALUES (zeroblob(33554432))")
        .execute(repo.pool())
        .await
        .unwrap();
    let mut conn = repo.pool().acquire().await.unwrap();
    let token = CancellationToken::new();
    let target = service.directory.join("cancelled-vacuum.sqlite");
    let (result, ()) = tokio::join!(vacuum_into(&mut conn, &target, &token, None), async {
        tokio::time::sleep(Duration::from_millis(1)).await;
        token.cancel();
    });
    assert!(matches!(result, Err(MaintenanceError::Cancelled)));
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check(1)")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(integrity, "ok");
    let bytes: i64 = sqlx::query_scalar("SELECT length(bytes) FROM synthetic_maintenance_payload")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(bytes, 33554432);
}
#[cfg(unix)]
#[tokio::test]
async fn swapped_cache_ancestor_is_rejected_without_touching_external_or_held_bytes() {
    use std::os::unix::fs::symlink;
    let (dir, _repo, service, _lease) = fixture().await;
    let cache = &service.cache_lease.path;
    fs::write(cache.join("abc.png"), b"held cache").unwrap();
    let preview = service.cache_preview().await.unwrap();
    let held = dir.path().join("held-cache");
    let external = dir.path().join("external");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("abc.png"), b"external").unwrap();
    fs::rename(cache, &held).unwrap();
    symlink(&external, cache).unwrap();
    assert!(service.cache_confirm(preview.id).await.is_err());
    assert_eq!(fs::read(external.join("abc.png")).unwrap(), b"external");
    assert_eq!(fs::read(held.join("abc.png")).unwrap(), b"held cache");
}

#[tokio::test]
async fn restore_rejects_candidate_replaced_at_publication_and_keeps_recovery_material() {
    let (_dir, repo, service, lease) = fixture().await;
    let summary = backup(&service).await;
    let preview = service.restore_preview(summary.id, None).await.unwrap();
    service.restore_confirm(preview.id, None).await.unwrap();
    repo.pool().close().await;
    TEST_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(|stage, path| {
            if stage == "restore_before_publish" {
                fs::rename(path, path.with_extension("verified.sqlite")).unwrap();
                fs::write(path, b"external unvalidated candidate").unwrap();
            }
        }))
    });
    let result = lease.apply_pending_restore(&service.directory).await;
    TEST_HOOK.with(|hook| hook.borrow_mut().take());
    assert!(matches!(result, Err(MaintenanceError::RecoveryRequired)));
    assert_eq!(
        fs::read(service.database_path()).unwrap(),
        b"external unvalidated candidate"
    );
    assert!(service.directory.join("restore-journal.json").exists());
    assert!(service.directory.join("pending-restore.json").exists());
    let journal: RestoreJournal =
        read_json(&service.directory.join("restore-journal.json")).unwrap();
    let work = service.directory.join(format!("restore-{}", journal.id));
    assert!(work.join("original.sqlite").exists());
    assert!(work.join("before-restore.sqlite").exists());
    assert!(work.join("candidate.verified.sqlite").exists());
    assert!(matches!(
        lease.apply_pending_restore(&service.directory).await,
        Err(MaintenanceError::RecoveryRequired)
    ));
}
#[cfg(unix)]
#[tokio::test]
async fn restore_rejects_symlink_candidate_at_publication_without_following_it() {
    use std::os::unix::fs::symlink;
    let (dir, repo, service, lease) = fixture().await;
    let summary = backup(&service).await;
    let preview = service.restore_preview(summary.id, None).await.unwrap();
    service.restore_confirm(preview.id, None).await.unwrap();
    repo.pool().close().await;
    let external = dir.path().join("external.sqlite");
    fs::write(&external, b"external preserved").unwrap();
    TEST_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move |stage, path| {
            if stage == "restore_before_publish" {
                fs::rename(path, path.with_extension("verified.sqlite")).unwrap();
                symlink(&external, path).unwrap();
            }
        }))
    });
    let result = lease.apply_pending_restore(&service.directory).await;
    TEST_HOOK.with(|hook| hook.borrow_mut().take());
    assert!(matches!(result, Err(MaintenanceError::RecoveryRequired)));
    assert!(fs::symlink_metadata(service.database_path())
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read(dir.path().join("external.sqlite")).unwrap(),
        b"external preserved"
    );
    assert!(service.directory.join("restore-journal.json").exists());
}
#[tokio::test]
async fn interrupted_recovery_checks_actual_published_original() {
    let (_dir, repo, service, lease) = fixture().await;
    repo.pool().close().await;
    let original_sha256 = hash_file(service.database_path(), CancellationToken::new())
        .await
        .unwrap();
    let id = Uuid::new_v4();
    let work = service.directory.join(format!("restore-{id}"));
    fs::create_dir(&work).unwrap();
    fs::rename(service.database_path(), work.join("original.sqlite")).unwrap();
    write_json_new(
        &service.directory.join("restore-journal.json"),
        &RestoreJournal {
            format_version: 1,
            id,
            original_sha256,
            candidate_sha256: "never-published".into(),
        },
    )
    .unwrap();
    TEST_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(|stage, path| {
            if stage == "recovery_before_publish" {
                fs::rename(path, path.with_extension("verified.sqlite")).unwrap();
                fs::write(path, b"replacement during recovery").unwrap();
            }
        }))
    });
    let result = lease.apply_pending_restore(&service.directory).await;
    TEST_HOOK.with(|hook| hook.borrow_mut().take());
    assert!(matches!(result, Err(MaintenanceError::RecoveryRequired)));
    assert_eq!(
        fs::read(service.database_path()).unwrap(),
        b"replacement during recovery"
    );
    assert!(work.join("original.verified.sqlite").exists());
    assert!(service.directory.join("restore-journal.json").exists());
}
#[tokio::test]
async fn backup_replaced_before_publication_cannot_return_success_or_catalogue_manifest() {
    let (_dir, _repo, service, _lease) = fixture().await;
    let preview = service.backup_preview(None).await.unwrap();
    TEST_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(|stage, path| {
            if stage == "backup_before_publish" {
                fs::rename(path, path.with_extension("verified.sqlite")).unwrap();
                fs::write(path, b"external replacement backup").unwrap();
            }
        }))
    });
    let result = service.backup_confirm(preview.id, None).await;
    TEST_HOOK.with(|hook| hook.borrow_mut().take());
    assert!(matches!(result, Err(MaintenanceError::Conflict)));
    assert!(service.status().await.unwrap().backups.is_empty());
}
#[tokio::test]
async fn backup_manifest_replaced_before_publication_cannot_return_success() {
    let (_dir, _repo, service, _lease) = fixture().await;
    let preview = service.backup_preview(None).await.unwrap();
    TEST_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(|stage, path| {
            if stage == "backup_before_manifest_publish" {
                let mut manifest: Manifest = read_json(path).unwrap();
                manifest.summary.contains_private_metadata =
                    !manifest.summary.contains_private_metadata;
                fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
            }
        }))
    });
    let result = service.backup_confirm(preview.id, None).await;
    TEST_HOOK.with(|hook| hook.borrow_mut().take());
    assert!(matches!(result, Err(MaintenanceError::Conflict)));
    assert!(service.status().await.unwrap().backups.is_empty());
}
#[tokio::test]
async fn crlf_backup_is_read_only_validated_then_only_restore_candidate_is_normalized() {
    use sha2::Sha384;
    let (_dir, repo, service, lease) = fixture().await;
    let summary = backup(&service).await;
    let path = backup_path(&service.directory, summary.id);
    let database = path.join("library.sqlite");
    let mut connection = open_connection(&database, false).await.unwrap();
    for migration in crate::migration_compat::canonical_migrator().iter() {
        sqlx::query("UPDATE _sqlx_migrations SET checksum=? WHERE version=?")
            .bind(Sha384::digest(migration.sql.replace('\n', "\r\n").as_bytes()).to_vec())
            .bind(migration.version)
            .execute(&mut connection)
            .await
            .unwrap();
    }
    connection.close().await.unwrap();
    let expected = hash_file(&database, CancellationToken::new())
        .await
        .unwrap();
    let manifest_path = path.join("manifest.json");
    let mut manifest: Manifest = read_json(&manifest_path).unwrap();
    manifest.sha256 = expected.clone();
    manifest.summary.bytes = std::fs::metadata(&database).unwrap().len();
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let preview = service.restore_preview(summary.id, None).await.unwrap();
    assert_eq!(
        hash_file(&database, CancellationToken::new())
            .await
            .unwrap(),
        expected
    );
    service.restore_confirm(preview.id, None).await.unwrap();
    repo.pool().close().await;
    lease
        .apply_pending_restore(&service.directory)
        .await
        .unwrap();
    assert_eq!(
        hash_file(&database, CancellationToken::new())
            .await
            .unwrap(),
        expected
    );
    let restored = LibraryRepository::open(service.database_path())
        .await
        .unwrap();
    for migration in crate::migration_compat::canonical_migrator().iter() {
        let checksum: Vec<u8> =
            sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=?")
                .bind(migration.version)
                .fetch_one(restored.pool())
                .await
                .unwrap();
        assert_eq!(checksum.as_slice(), migration.checksum.as_ref());
    }
}
#[tokio::test]
async fn restore_copy_is_bound_to_validated_manifest_before_candidate_normalization() {
    let (_dir, repo, service, lease) = fixture().await;
    let summary = backup(&service).await;
    let preview = service.restore_preview(summary.id, None).await.unwrap();
    service.restore_confirm(preview.id, None).await.unwrap();
    let (_other, other_repo, other_service, _other_lease) = fixture().await;
    sqlx::query("INSERT INTO media_items(id,kind,title,adult_isolated,created_at,updated_at) VALUES ('external','adult_video','synthetic private replacement',1,1,1)").execute(other_repo.pool()).await.unwrap();
    let grant = AdultOutputAccess::new();
    let other_preview = other_service
        .backup_preview(Some(grant.clone()))
        .await
        .unwrap();
    let other_summary = other_service
        .backup_confirm(other_preview.id, Some(grant))
        .await
        .unwrap();
    let replacement =
        backup_path(&other_service.directory, other_summary.id).join("library.sqlite");
    repo.pool().close().await;
    let before = hash_file(service.database_path(), CancellationToken::new())
        .await
        .unwrap();
    TEST_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move |stage, path| {
            if stage == "restore_before_copy" {
                fs::copy(&replacement, path).unwrap();
            }
        }))
    });
    let result = lease.apply_pending_restore(&service.directory).await;
    TEST_HOOK.with(|hook| hook.borrow_mut().take());
    assert!(matches!(result, Err(MaintenanceError::Conflict)));
    assert_eq!(
        hash_file(service.database_path(), CancellationToken::new())
            .await
            .unwrap(),
        before
    );
    assert!(!service.directory.join("restore-journal.json").exists());
    assert!(service.directory.join("pending-restore.json").exists());
}
