use super::*;
use std::{path::PathBuf, sync::Mutex as StdMutex};
type MoveHook = Box<dyn FnOnce() + Send>;
fn hooks() -> &'static StdMutex<HashMap<PathBuf, MoveHook>> {
    static H: OnceLock<StdMutex<HashMap<PathBuf, MoveHook>>> = OnceLock::new();
    H.get_or_init(|| StdMutex::new(HashMap::new()))
}
pub(super) fn before_move(path: &Path) {
    let hook = hooks().lock().unwrap().remove(path);
    if let Some(h) = hook {
        h();
    }
}
struct CopyGate {
    entered: AtomicBool,
    release: AtomicBool,
}
fn gates() -> &'static StdMutex<HashMap<Uuid, Arc<CopyGate>>> {
    static G: OnceLock<StdMutex<HashMap<Uuid, Arc<CopyGate>>>> = OnceLock::new();
    G.get_or_init(|| StdMutex::new(HashMap::new()))
}
thread_local! {static COPY_GATE:std::cell::RefCell<Option<Arc<CopyGate>>>=const{std::cell::RefCell::new(None)};}
pub(super) fn execution_start(id: Uuid) {
    COPY_GATE.with(|v| *v.borrow_mut() = gates().lock().unwrap().remove(&id));
}
pub(super) fn copy_chunk() {
    COPY_GATE.with(|v| {
        if let Some(gate) = v.borrow().as_ref() {
            gate.entered.store(true, Ordering::SeqCst);
            while !gate.release.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    });
}
fn faults() -> &'static StdMutex<HashMap<Uuid, String>> {
    static F: OnceLock<StdMutex<HashMap<Uuid, String>>> = OnceLock::new();
    F.get_or_init(|| StdMutex::new(HashMap::new()))
}
pub(super) fn checkpoint(id: Uuid, point: &str) -> Result<()> {
    let mut map = faults().lock().unwrap();
    if map.get(&id).is_some_and(|p| p == point) {
        map.remove(&id);
        Err(RepositoryError::Conflict)
    } else {
        Ok(())
    }
}
struct Fixture {
    _temp: tempfile::TempDir,
    repo: LibraryRepository,
    source: LibraryRoot,
    target: LibraryRoot,
    item: MediaItem,
    files: Vec<MediaFile>,
}
impl Fixture {
    async fn new() -> Self {
        let temp = crate::test_support::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("source")).unwrap();
        std::fs::create_dir(temp.path().join("target")).unwrap();
        let repo = LibraryRepository::open(&temp.path().join("db.sqlite"))
            .await
            .unwrap();
        let now = chrono::DateTime::from_timestamp_millis(Utc::now().timestamp_millis()).unwrap();
        let root = |name: &str| LibraryRoot {
            id: Uuid::new_v4(),
            name: name.into(),
            path: temp.path().join(name).to_str().unwrap().into(),
            path_style: crate::test_support::native_path_style(),
            kind: Some(MediaKind::Movie),
            enabled: true,
            created_at: now,
            updated_at: now,
        };
        let source = root("source");
        let target = root("target");
        repo.save_root(&source).await.unwrap();
        repo.save_root(&target).await.unwrap();
        let item = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::Movie,
            title: "Film".into(),
            original_title: None,
            year: Some(2024),
            created_at: now,
            updated_at: now,
        };
        repo.save_media(&item).await.unwrap();
        let mut files = vec![];
        for part in [1, 2] {
            let name = format!("Movie.part{part}.mkv");
            let path = temp.path().join("source").join(&name);
            std::fs::write(&path, format!("video bytes part {part}")).unwrap();
            let m = std::fs::metadata(path).unwrap();
            let file = MediaFile {
                id: Uuid::new_v4(),
                media_item_id: item.id,
                root_id: source.id,
                relative_path: name,
                size_bytes: m.len(),
                modified_at: Some(
                    chrono::DateTime::from_timestamp_millis(
                        chrono::DateTime::<Utc>::from(m.modified().unwrap()).timestamp_millis(),
                    )
                    .unwrap(),
                ),
                part: Some(part),
            };
            repo.save_file(&file).await.unwrap();
            files.push(file);
        }
        for (name, bytes) in [
            ("Movie.part1.en.srt", "subtitle one"),
            ("Movie.part2.zh.ass", "subtitle two"),
            ("Movie.part1.nfo", "<movie><title>Film</title></movie>"),
            ("poster.jpg", "poster bytes"),
            ("Movie.part1.attachment.bin", "opaque bytes"),
            ("Movie.part11.en.srt", "unrelated bytes"),
        ] {
            std::fs::write(temp.path().join("source").join(name), bytes).unwrap();
        }
        Self {
            _temp: temp,
            repo,
            source,
            target,
            item,
            files,
        }
    }
    fn request(&self, strategy: OrganizationStrategy) -> OrganizationRequest {
        OrganizationRequest {
            media_item_id: self.item.id,
            target_root_id: self.target.id,
            template: "Film/{title}{part}.{ext}".into(),
            strategy,
        }
    }
    fn service(&self) -> OrganizationService {
        OrganizationService::new(self.repo.clone())
    }
    fn source_path(&self, name: &str) -> PathBuf {
        Path::new(&self.source.path).join(name)
    }
    fn target_path(&self, name: &str) -> PathBuf {
        Path::new(&self.target.path).join(name)
    }
}
#[tokio::test]
async fn all_modes_keep_complete_parts_attachments_and_explicit_rollback() {
    for strategy in [
        OrganizationStrategy::Preserve,
        OrganizationStrategy::Move,
        OrganizationStrategy::Copy,
        OrganizationStrategy::Hardlink,
    ] {
        let f = Fixture::new().await;
        let s = f.service();
        let preview = s.preview(f.request(strategy)).await.unwrap();
        assert_eq!(preview.file_count, 7);
        assert!(preview.files.iter().all(|v| v.conflict.is_none()));
        assert!(!f.target_path("Film").exists());
        let before = preview
            .files
            .iter()
            .map(|v| (v.source.clone(), std::fs::read(&v.source).unwrap()))
            .collect::<Vec<_>>();
        let result = s.execute(preview.id).await.unwrap();
        assert_eq!(result.state, "succeeded");
        assert_eq!(result.completed, 7);
        assert!(s.execute(preview.id).await.unwrap().already_applied);
        for (file, (_, bytes)) in preview.files.iter().zip(&before) {
            if strategy == OrganizationStrategy::Preserve {
                assert!(!Path::new(&file.target).exists());
            } else {
                assert_eq!(std::fs::read(&file.target).unwrap(), *bytes);
            }
            assert_eq!(
                Path::new(&file.source).exists(),
                strategy != OrganizationStrategy::Move
            );
        }
        assert_eq!(
            std::fs::read(f.source_path("Movie.part11.en.srt")).unwrap(),
            b"unrelated bytes"
        );
        let recovery = s.rollback_preview(preview.id).await.unwrap();
        assert_eq!(recovery.kind, "rollback");
        let restored = s.rollback_execute(recovery.id).await.unwrap();
        assert_eq!(restored.state, "succeeded");
        assert!(
            s.rollback_execute(recovery.id)
                .await
                .unwrap()
                .already_applied
        );
        for (file, (_, bytes)) in preview.files.iter().zip(&before) {
            assert_eq!(std::fs::read(&file.source).unwrap(), *bytes);
            assert!(!Path::new(&file.target).exists());
        }
        assert_eq!(f.repo.list_files(f.item.id).await.unwrap().len(), 2);
    }
}
#[tokio::test]
async fn stale_source_and_target_changes_refuse_without_overwrite() {
    let f = Fixture::new().await;
    let s = f.service();
    let name = &f.files[0].relative_path;
    let old = std::fs::read(f.source_path(name)).unwrap();
    std::fs::write(f.source_path(name), b"changed before preview").unwrap();
    assert!(s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .is_err());
    std::fs::write(f.source_path(name), &old).unwrap();
    // Refresh cached stamp to permit a new preview, then alter source afterwards.
    let mut file = f.files[0].clone();
    file.modified_at = None;
    f.repo.save_file(&file).await.unwrap();
    let p = s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    std::fs::write(f.source_path(name), b"external after preview").unwrap();
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    assert_eq!(
        std::fs::read(f.source_path(name)).unwrap(),
        b"external after preview"
    );
    assert!(!f.target_path("Film").exists());
    let g = Fixture::new().await;
    let s = g.service();
    std::fs::create_dir(g.target_path("Film")).unwrap();
    let p = s
        .preview(g.request(OrganizationStrategy::Copy))
        .await
        .unwrap();
    std::fs::write(g.target_path("Film/FILM.part1.MKV"), b"external").unwrap();
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    assert_eq!(
        std::fs::read(g.target_path("Film/FILM.part1.MKV")).unwrap(),
        b"external"
    );
}
#[tokio::test]
async fn portable_templates_and_existing_name_collisions_are_previewed() {
    let f = Fixture::new().await;
    let s = f.service();
    for template in [
        "../evil.mkv",
        "C:/evil.mkv",
        "//host/share/file.mkv",
        "CON.mkv",
        "LPT1.txt",
        "folder/name. ",
        "foo\\bar.mkv",
        "file?.mkv",
        "nul .mkv",
    ] {
        let mut r = f.request(OrganizationStrategy::Copy);
        r.template = template.into();
        assert!(s.preview(r).await.is_err(), "{template}");
    }
    let mut r = f.request(OrganizationStrategy::Copy);
    r.template = "Film/{title}.{ext}".into();
    let p = s.preview(r).await.unwrap();
    assert!(p
        .files
        .iter()
        .any(|v| v.conflict.as_deref() == Some("portable_name_collision")));
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    std::fs::write(f.target_path("film.part1.mkv. "), b"external").unwrap();
    let mut r = f.request(OrganizationStrategy::Copy);
    r.template = "film{part}.{ext}".into();
    let p = s.preview(r).await.unwrap();
    assert!(p
        .files
        .iter()
        .any(|v| v.conflict.as_deref() == Some("target_exists")));
}
#[cfg(unix)]
#[tokio::test]
async fn changed_parent_root_scope_and_sidecar_sets_reject_preview() {
    let f = Fixture::new().await;
    std::fs::create_dir(f.target_path("Film")).unwrap();
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Copy))
        .await
        .unwrap();
    std::fs::rename(f.target_path("Film"), f.target_path("held")).unwrap();
    std::fs::create_dir(f.target_path("Film")).unwrap();
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    assert!(!f.target_path("Film/Film.part1.mkv").exists());
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    std::fs::write(f.source_path("Movie.part1.fr.srt"), b"new sidecar").unwrap();
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    assert!(f.source_path("Movie.part1.mkv").exists());
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
        .bind(f.item.id.to_string())
        .execute(f.repo.pool())
        .await
        .unwrap();
    assert!(s.execute(p.id).await.is_err());
    assert!(s.history().await.unwrap().is_empty());
    assert!(s.plan(p.id).await.is_err());
    assert!(s.logs(p.id).await.is_err());
    assert!(s.cancel(p.id).await.is_err());
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Copy))
        .await
        .unwrap();
    let mut root = f.target.clone();
    root.enabled = false;
    f.repo.save_root(&root).await.unwrap();
    assert!(s.execute(p.id).await.is_err());
}
#[cfg(unix)]
#[tokio::test]
async fn publication_and_actual_quarantine_races_preserve_external_files() {
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Copy))
        .await
        .unwrap();
    let destination = PathBuf::from(&p.files[0].target);
    let stage = f.target_path(&format!(".mediaatlas-organization-{}/0", p.id));
    let dest = destination.clone();
    hooks().lock().unwrap().insert(
        stage,
        Box::new(move || {
            std::fs::write(dest, b"external race destination").unwrap();
        }),
    );
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    assert_eq!(
        std::fs::read(destination).unwrap(),
        b"external race destination"
    );
    assert!(f.source_path("Movie.part1.mkv").exists());
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    let source = PathBuf::from(&p.files[0].source);
    let original = std::fs::read(&source).unwrap();
    let saved = source.with_extension("externally-held");
    let path = source.clone();
    let held = saved.clone();
    hooks().lock().unwrap().insert(
        source.clone(),
        Box::new(move || {
            std::fs::rename(&path, held).unwrap();
            std::fs::write(path, b"external source replacement").unwrap();
        }),
    );
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    assert_eq!(
        std::fs::read(&source).unwrap(),
        b"external source replacement"
    );
    assert_eq!(std::fs::read(saved).unwrap(), original);
    assert_eq!(std::fs::read(&p.files[0].target).unwrap(), original);
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Copy))
        .await
        .unwrap();
    assert_eq!(s.execute(p.id).await.unwrap().state, "succeeded");
    let rollback = s.rollback_preview(p.id).await.unwrap();
    let target = PathBuf::from(&p.files[0].target);
    let original = std::fs::read(&target).unwrap();
    let saved = target.with_extension("externally-held");
    let path = target.clone();
    let held = saved.clone();
    hooks().lock().unwrap().insert(
        target.clone(),
        Box::new(move || {
            std::fs::rename(&path, held).unwrap();
            std::fs::write(path, b"external rollback replacement").unwrap();
        }),
    );
    assert_eq!(
        s.rollback_execute(rollback.id).await.unwrap().state,
        "conflict"
    );
    assert_eq!(
        std::fs::read(target).unwrap(),
        b"external rollback replacement"
    );
    assert_eq!(std::fs::read(saved).unwrap(), original);
}
#[tokio::test]
async fn rollback_rejects_external_edits_and_post_preview_index_changes() {
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    assert_eq!(s.execute(p.id).await.unwrap().state, "succeeded");
    std::fs::write(&p.files[0].target, b"external edit").unwrap();
    assert!(s.rollback_preview(p.id).await.is_err());
    assert_eq!(std::fs::read(&p.files[0].target).unwrap(), b"external edit");
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Copy))
        .await
        .unwrap();
    assert_eq!(s.execute(p.id).await.unwrap().state, "succeeded");
    let rollback = s.rollback_preview(p.id).await.unwrap();
    sqlx::query("UPDATE media_files SET relative_path='external-new-path.mkv' WHERE id=?")
        .bind(f.files[0].id.to_string())
        .execute(f.repo.pool())
        .await
        .unwrap();
    assert_eq!(
        s.rollback_execute(rollback.id).await.unwrap().state,
        "conflict"
    );
    assert!(Path::new(&p.files[0].target).exists());
    let value: String = sqlx::query_scalar("SELECT relative_path FROM media_files WHERE id=?")
        .bind(f.files[0].id.to_string())
        .fetch_one(f.repo.pool())
        .await
        .unwrap();
    assert_eq!(value, "external-new-path.mkv");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_from_an_independent_service_interrupts_the_actual_copy() {
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Copy))
        .await
        .unwrap();
    let gate = Arc::new(CopyGate {
        entered: AtomicBool::new(false),
        release: AtomicBool::new(false),
    });
    gates().lock().unwrap().insert(p.id, gate.clone());
    let executor = s.clone();
    let id = p.id;
    let running = tokio::spawn(async move { executor.execute(id).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !gate.entered.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    let independent = OrganizationService::new(f.repo.clone());
    let cancellation = tokio::spawn(async move { independent.cancel(id).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if sessions()
                .lock()
                .await
                .get(&id)
                .is_some_and(|v| v.cancel.load(Ordering::SeqCst))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    gate.release.store(true, Ordering::SeqCst);
    assert_eq!(running.await.unwrap().unwrap().state, "cancelled");
    assert_eq!(cancellation.await.unwrap().unwrap().state, "cancelled");
    assert!(f.source_path("Movie.part1.mkv").exists());
    assert!(!f.target_path("Film/Film.part1.mkv").exists());
}
#[tokio::test]
async fn crash_boundaries_are_journaled_and_recovered_only_by_explicit_preview() {
    for point in [
        "stage_prepared",
        "published",
        "source_quarantined",
        "index_committed",
    ] {
        let f = Fixture::new().await;
        let s = f.service();
        let p = s
            .preview(f.request(OrganizationStrategy::Move))
            .await
            .unwrap();
        let originals = p
            .files
            .iter()
            .map(|v| std::fs::read(&v.source).unwrap())
            .collect::<Vec<_>>();
        faults().lock().unwrap().insert(p.id, point.into());
        assert_eq!(s.execute(p.id).await.unwrap().state, "conflict", "{point}");
        sqlx::query("UPDATE organization_operations SET state='running' WHERE id=?")
            .bind(p.id.to_string())
            .execute(f.repo.pool())
            .await
            .unwrap();
        let recovered = OrganizationService::new(f.repo.clone());
        assert_eq!(recovered.recover_interrupted().await.unwrap(), 1);
        assert_eq!(recovered.plan(p.id).await.unwrap().state, "interrupted");
        assert!(recovered.execute(p.id).await.is_err());
        let preview = recovered.rollback_preview(p.id).await.unwrap();
        assert_eq!(
            recovered.rollback_execute(preview.id).await.unwrap().state,
            "succeeded",
            "{point}"
        );
        for (file, bytes) in p.files.iter().zip(originals) {
            assert_eq!(std::fs::read(&file.source).unwrap(), bytes, "{point}");
            assert!(!Path::new(&file.target).exists(), "{point}");
        }
        let logs = recovered.logs(p.id).await.unwrap();
        assert!(logs.iter().any(|v| v.stage == "publish_prepared"));
    }
}
#[tokio::test]
async fn move_updates_only_local_location_references_and_rollback_restores_them() {
    let f = Fixture::new().await;
    let s = f.service();
    let source = format!("library://{}/Movie.part1.mkv", f.source.id);
    let poster = format!("library://{}/poster.jpg", f.source.id);
    sqlx::query("INSERT INTO local_metadata_provenance(media_item_id,source_kind,source_reference,confirmed_at) VALUES(?,'nfo',?,?)").bind(f.item.id.to_string()).bind(&source).bind(Utc::now().timestamp_millis()).execute(f.repo.pool()).await.unwrap();
    sqlx::query("INSERT INTO artworks(id,media_item_id,kind,source_url,provider_id,created_at) VALUES(?,?,'poster',?,'local',?)").bind(Uuid::new_v4().to_string()).bind(f.item.id.to_string()).bind(&poster).bind(Utc::now().timestamp_millis()).execute(f.repo.pool()).await.unwrap();
    sqlx::query("INSERT INTO metadata_values(id,media_item_id,field_key,value_json,source_url,is_locked,is_user_modified) VALUES(?,?,'title','\"Protected title\"',?,1,1)").bind(Uuid::new_v4().to_string()).bind(f.item.id.to_string()).bind(&source).execute(f.repo.pool()).await.unwrap();
    let p = s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    assert_eq!(s.execute(p.id).await.unwrap().state, "succeeded");
    let value: String = sqlx::query_scalar(
        "SELECT source_reference FROM local_metadata_provenance WHERE media_item_id=?",
    )
    .bind(f.item.id.to_string())
    .fetch_one(f.repo.pool())
    .await
    .unwrap();
    assert_eq!(
        value,
        format!("library://{}/Film/Film.part1.mkv", f.target.id)
    );
    let value: String = sqlx::query_scalar("SELECT source_url FROM artworks WHERE media_item_id=?")
        .bind(f.item.id.to_string())
        .fetch_one(f.repo.pool())
        .await
        .unwrap();
    assert_eq!(value, format!("library://{}/Film/poster.jpg", f.target.id));
    let rollback = s.rollback_preview(p.id).await.unwrap();
    assert_eq!(
        s.rollback_execute(rollback.id).await.unwrap().state,
        "succeeded"
    );
    let value: String = sqlx::query_scalar(
        "SELECT source_reference FROM local_metadata_provenance WHERE media_item_id=?",
    )
    .bind(f.item.id.to_string())
    .fetch_one(f.repo.pool())
    .await
    .unwrap();
    assert_eq!(value, source);
    let value: String =
        sqlx::query_scalar("SELECT value_json FROM metadata_values WHERE media_item_id=?")
            .bind(f.item.id.to_string())
            .fetch_one(f.repo.pool())
            .await
            .unwrap();
    assert_eq!(value, "\"Protected title\"");
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn actual_cross_volume_move_and_hardlink_capability_failure() {
    let mut f = Fixture::new().await;
    let destination = tempfile::Builder::new()
        .prefix("mediaatlas-cross-volume-")
        .tempdir_in(if Path::new("/workspace").is_dir() {
            PathBuf::from("/workspace")
        } else {
            std::env::current_dir().unwrap()
        })
        .unwrap();
    f.target.path = destination.path().to_str().unwrap().into();
    f.repo.save_root(&f.target).await.unwrap();
    let source = RootCap::open(Path::new(&f.source.path)).unwrap();
    let target = RootCap::open(Path::new(&f.target.path)).unwrap();
    if volume(&source).unwrap() != volume(&target).unwrap() {
        let s = f.service();
        let hardlink = s
            .preview(f.request(OrganizationStrategy::Hardlink))
            .await
            .unwrap();
        assert!(hardlink
            .files
            .iter()
            .any(|v| v.conflict.as_deref() == Some("hardlink_cross_volume")));
        assert_eq!(s.execute(hardlink.id).await.unwrap().state, "conflict");
        let p = s
            .preview(f.request(OrganizationStrategy::Move))
            .await
            .unwrap();
        assert_eq!(s.execute(p.id).await.unwrap().state, "succeeded");
        assert!(!f.source_path("Movie.part1.mkv").exists());
        assert_eq!(
            std::fs::read(f.target_path("Film/Film.part1.mkv")).unwrap(),
            b"video bytes part 1"
        );
        let rollback = s.rollback_preview(p.id).await.unwrap();
        assert_eq!(
            s.rollback_execute(rollback.id).await.unwrap().state,
            "succeeded"
        );
        assert!(f.source_path("Movie.part1.mkv").exists());
    }
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Hardlink))
        .await
        .unwrap();
    faults()
        .lock()
        .unwrap()
        .insert(p.id, "hardlink_unsupported".into());
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    assert!(f.source_path("Movie.part1.mkv").exists());
    assert!(!f.target_path("Film/Film.part1.mkv").exists());
}
#[tokio::test]
async fn same_path_modes_are_idempotent_and_adult_plans_never_regain_a_grant() {
    for strategy in [
        OrganizationStrategy::Move,
        OrganizationStrategy::Copy,
        OrganizationStrategy::Hardlink,
    ] {
        let f = Fixture::new().await;
        let s = f.service();
        let mut r = f.request(strategy);
        r.target_root_id = f.source.id;
        r.template = "{stem}.{ext}".into();
        let p = s.preview(r).await.unwrap();
        assert!(p.files.iter().all(|v| v.same_path));
        assert_eq!(s.execute(p.id).await.unwrap().state, "succeeded");
        assert!(s.execute(p.id).await.unwrap().already_applied);
        assert_eq!(f.repo.list_files(f.item.id).await.unwrap().len(), 2);
    }
    let f = Fixture::new().await;
    let mut item = f.item.clone();
    item.kind = MediaKind::AdultVideo;
    f.repo.save_media(&item).await.unwrap();
    sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
        .bind(item.id.to_string())
        .execute(f.repo.pool())
        .await
        .unwrap();
    let mut source = f.source.clone();
    source.kind = Some(MediaKind::AdultVideo);
    f.repo.save_root(&source).await.unwrap();
    let mut target = f.target.clone();
    target.kind = Some(MediaKind::AdultVideo);
    f.repo.save_root(&target).await.unwrap();
    assert!(f
        .service()
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .is_err());
    let access = AdultOutputAccess::new();
    let s = OrganizationService::new_adult(f.repo.clone(), access.clone());
    let p = s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    access.revoke();
    assert!(s.execute(p.id).await.is_err());
    let fresh = OrganizationService::new_adult(f.repo.clone(), AdultOutputAccess::new());
    assert!(fresh.execute(p.id).await.is_err());
    assert!(f.service().cancel(p.id).await.is_err());
    let new = fresh
        .preview(f.request(OrganizationStrategy::Copy))
        .await
        .unwrap();
    assert_eq!(fresh.execute(new.id).await.unwrap().state, "succeeded");
}
#[tokio::test]
async fn shared_generic_and_foreign_indexed_sidecars_require_review() {
    let f = Fixture::new().await;
    std::fs::write(f.source_path("Other Movie.mkv"), b"other film").unwrap();
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    assert!(p.files.iter().any(|v| v.source.ends_with("poster.jpg")
        && v.conflict.as_deref() == Some("ambiguous_shared_attachment")));
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    assert!(f.source_path("poster.jpg").exists());
    let f = Fixture::new().await;
    let mut neighbor = f.item.clone();
    neighbor.id = Uuid::new_v4();
    neighbor.title = "Neighbor".into();
    f.repo.save_media(&neighbor).await.unwrap();
    let mut file = f.files[0].clone();
    file.id = Uuid::new_v4();
    file.media_item_id = neighbor.id;
    file.relative_path = "Movie.part1.attachment.bin".into();
    file.size_bytes = std::fs::metadata(f.source_path(&file.relative_path))
        .unwrap()
        .len();
    file.modified_at = None;
    file.part = None;
    f.repo.save_file(&file).await.unwrap();
    let p = f
        .service()
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    assert!(p
        .files
        .iter()
        .any(|v| v.conflict.as_deref() == Some("attachment_owned_by_other_media")));
    assert_eq!(f.service().execute(p.id).await.unwrap().state, "conflict");
}
#[tokio::test]
async fn duplicate_concurrent_execute_and_unknown_cancel_have_typed_ownership() {
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Copy))
        .await
        .unwrap();
    assert!(s.cancel(Uuid::new_v4()).await.is_err());
    let other = OrganizationService::new(f.repo.clone());
    let (a, b) = tokio::join!(s.execute(p.id), other.execute(p.id));
    assert!(
        a.as_ref().is_ok_and(|v| v.state == "succeeded")
            || b.as_ref().is_ok_and(|v| v.state == "succeeded")
    );
    assert!(s.execute(p.id).await.unwrap().already_applied);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM organization_journal WHERE operation_id=? AND stage='file_complete'",
    )
    .bind(p.id.to_string())
    .fetch_one(f.repo.pool())
    .await
    .unwrap();
    assert_eq!(count, 7);
}
#[tokio::test]
async fn preview_time_budget_and_lock_during_copy_bound_work_and_preserve_sources() {
    let f = Fixture::new().await;
    let cap = RootCap::open(Path::new(&f.source.path)).unwrap();
    let parent = cap.parent(&f.files[0].relative_path, false).unwrap();
    let mut source = fs::file(&parent.dir, &parent.name).unwrap();
    let result = fs::with_budget(None, || {
        fs::expire_budget();
        fs::stamp(&mut source)
    });
    assert_eq!(result.unwrap_err(), RepositoryError::Conflict);
    let mut item = f.item.clone();
    item.kind = MediaKind::AdultVideo;
    f.repo.save_media(&item).await.unwrap();
    sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
        .bind(item.id.to_string())
        .execute(f.repo.pool())
        .await
        .unwrap();
    for mut root in [f.source.clone(), f.target.clone()] {
        root.kind = Some(MediaKind::AdultVideo);
        f.repo.save_root(&root).await.unwrap();
    }
    let access = AdultOutputAccess::new();
    let s = OrganizationService::new_adult(f.repo.clone(), access.clone());
    let p = s
        .preview(f.request(OrganizationStrategy::Copy))
        .await
        .unwrap();
    let gate = Arc::new(CopyGate {
        entered: AtomicBool::new(false),
        release: AtomicBool::new(false),
    });
    gates().lock().unwrap().insert(p.id, gate.clone());
    let running = tokio::spawn(async move { s.execute(p.id).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !gate.entered.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    access.revoke();
    gate.release.store(true, Ordering::SeqCst);
    assert_eq!(running.await.unwrap().unwrap().state, "conflict");
    assert!(f.source_path("Movie.part1.mkv").exists());
    assert!(!f.target_path("Film/Film.part1.mkv").exists());
}
#[cfg(unix)]
#[tokio::test]
async fn nofollow_and_full_hash_reject_same_stamp_edits_and_symlink_replacement() {
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    let path = f.source_path(&f.files[0].relative_path);
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let before = std::fs::read(&path).unwrap();
    std::fs::write(&path, vec![b'x'; before.len()]).unwrap();
    std::fs::File::open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    assert_eq!(std::fs::read(&path).unwrap(), vec![b'x'; before.len()]);
    let f = Fixture::new().await;
    let s = f.service();
    let p = s
        .preview(f.request(OrganizationStrategy::Move))
        .await
        .unwrap();
    let path = PathBuf::from(&p.files[0].source);
    let saved = path.with_extension("held-source");
    let outside = f._temp.path().join("outside-bytes");
    std::fs::write(&outside, b"outside bytes").unwrap();
    let source = path.clone();
    let held = saved.clone();
    let target = outside.clone();
    hooks().lock().unwrap().insert(
        path.clone(),
        Box::new(move || {
            std::fs::rename(&source, held).unwrap();
            std::os::unix::fs::symlink(target, source).unwrap();
        }),
    );
    assert_eq!(s.execute(p.id).await.unwrap().state, "conflict");
    assert!(std::fs::symlink_metadata(&path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(saved.exists());
    assert_eq!(std::fs::read(outside).unwrap(), b"outside bytes");
}
#[tokio::test]
async fn video_organization_rejects_incompatible_target_domains() {
    let f = Fixture::new().await;
    for kind in [
        MediaKind::MusicArtist,
        MediaKind::MusicRelease,
        MediaKind::MusicTrack,
        MediaKind::TvSeries,
        MediaKind::Anime,
    ] {
        let mut root = f.target.clone();
        root.kind = Some(kind);
        f.repo.save_root(&root).await.unwrap();
        assert!(
            f.service()
                .preview(f.request(OrganizationStrategy::Move))
                .await
                .is_err(),
            "{kind:?}"
        );
        assert!(f.source_path("Movie.part1.mkv").exists());
    }
    let mut root = f.target.clone();
    root.kind = None;
    f.repo.save_root(&root).await.unwrap();
    assert!(f
        .service()
        .preview(f.request(OrganizationStrategy::Preserve))
        .await
        .is_ok());
}
