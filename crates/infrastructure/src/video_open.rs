//! Explicit dispatch of a verified local video file to its OS association.
//! Construction and library reads never launch an application.
use crate::{
    library::{LibraryRepository, RepositoryError},
    library_ready::ready_predicate,
    media_output::AdultOutputAccess,
};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, File, Metadata, OpenOptions},
};
use chrono::DateTime;
use mediaatlas_domain::{normalize_relative_path, LibraryRoot, MediaFile};
use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, Transaction};
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

/// Implemented by the native adapter and deterministic tests; never exposed over IPC.
pub trait NativeVideoOpener: Send + Sync {
    fn open(&self, path: &Path) -> std::io::Result<()>;
}
struct SystemVideoOpener;
impl NativeVideoOpener for SystemVideoOpener {
    fn open(&self, path: &Path) -> std::io::Result<()> {
        open::that_detached_with_timeout(path, std::time::Duration::from_secs(3))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoOpenResult {
    pub requested: bool,
}
#[derive(Clone)]
pub struct VideoOpenService {
    repo: LibraryRepository,
    opener: Arc<dyn NativeVideoOpener>,
    adult_access: Option<AdultOutputAccess>,
}
impl VideoOpenService {
    pub fn new(repo: LibraryRepository) -> Self {
        Self {
            repo,
            opener: Arc::new(SystemVideoOpener),
            adult_access: None,
        }
    }
    pub fn with_opener(repo: LibraryRepository, opener: Arc<dyn NativeVideoOpener>) -> Self {
        Self {
            repo,
            opener,
            adult_access: None,
        }
    }
    pub fn new_adult(repo: LibraryRepository, access: AdultOutputAccess) -> Self {
        Self {
            repo,
            opener: Arc::new(SystemVideoOpener),
            adult_access: Some(access),
        }
    }
    pub fn with_adult_opener(
        repo: LibraryRepository,
        access: AdultOutputAccess,
        opener: Arc<dyn NativeVideoOpener>,
    ) -> Self {
        Self {
            repo,
            opener,
            adult_access: Some(access),
        }
    }
    fn ensure_access(&self) -> Result<(), RepositoryError> {
        self.adult_access
            .as_ref()
            .map_or(Ok(()), AdultOutputAccess::ensure)
    }
    /// Call only for a user's explicit Open action. The frontend supplies a file ID,
    /// never a pathname, shell command, URL, or guessed edition.
    pub async fn open(&self, file_id: Uuid) -> Result<VideoOpenResult, RepositoryError> {
        self.ensure_access()?;
        let mut initial_tx = self.repo.pool().begin().await?;
        let (file, root) =
            ensure_video_file_tx(&mut initial_tx, file_id, self.adult_access.is_some()).await?;
        initial_tx.commit().await?;
        let initial_file = file.clone();
        let initial_root = root.clone();
        let verified = tokio::task::spawn_blocking(move || VerifiedVideoPath::open(&root, &file))
            .await
            .map_err(|_| RepositoryError::Database)??;
        let mut tx = self.repo.pool().begin().await?;
        // The writer reservation prevents concurrent Root/kind/link changes while
        // final authority is checked and the native request is submitted.
        sqlx::query("UPDATE media_files SET size_bytes=size_bytes WHERE id=?")
            .bind(file_id.to_string())
            .execute(&mut *tx)
            .await?;
        let (file, root) =
            ensure_video_file_tx(&mut tx, file_id, self.adult_access.is_some()).await?;
        if file != initial_file || root != initial_root {
            return Err(RepositoryError::Conflict);
        }
        self.ensure_access()?;
        let access = self.adult_access.clone();
        let opener = self.opener.clone();
        tokio::task::spawn_blocking(move || {
            verified.recheck()?;
            if let Some(access) = &access {
                access.ensure()?;
            }
            opener
                .open(&verified.path)
                .map_err(|_| RepositoryError::InvalidInput)?;
            // All capabilities (including Windows no-delete leases) stay alive
            // through native dispatch. Success means requested, never playback.
            drop(verified);
            Ok::<_, RepositoryError>(())
        })
        .await
        .map_err(|_| RepositoryError::Database)??;
        tx.commit().await?;
        Ok(VideoOpenResult { requested: true })
    }
}
struct VerifiedVideoPath {
    path: PathBuf,
    _directories: Vec<Dir>,
    root: LibraryRoot,
    file: MediaFile,
    source: File,
    snapshot: VideoOpenFingerprint,
}
#[derive(PartialEq, Eq)]
struct VideoOpenFingerprint {
    size: u64,
    modified: std::time::SystemTime,
    identity: String,
}
impl VerifiedVideoPath {
    fn open(root: &LibraryRoot, file: &MediaFile) -> Result<Self, RepositoryError> {
        if file.relative_path.contains("://") {
            return Err(RepositoryError::InvalidInput);
        }
        normalize_relative_path(&file.relative_path, root.path_style)
            .map_err(|_| RepositoryError::InvalidInput)?;
        // Path components are independently validated; no root escape, URL or
        // executable dispatch is possible through this extension-limited API.
        let relative = Path::new(&file.relative_path);
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(RepositoryError::InvalidInput);
        }
        let extension = relative
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(
            extension.as_str(),
            "mkv"
                | "mp4"
                | "avi"
                | "mov"
                | "m4v"
                | "wmv"
                | "webm"
                | "ts"
                | "m2ts"
                | "mpg"
                | "mpeg"
                | "vob"
                | "ogv"
        ) {
            return Err(RepositoryError::InvalidInput);
        }
        let path = Path::new(&root.path).join(relative);
        if !path.is_absolute() {
            return Err(RepositoryError::InvalidInput);
        }
        let mut anchor = PathBuf::new();
        let mut parts = vec![];
        for component in path.components() {
            match component {
                Component::Prefix(p) => anchor.push(p.as_os_str()),
                Component::RootDir => anchor.push(component.as_os_str()),
                Component::Normal(n) => parts.push(n.to_os_string()),
                _ => return Err(RepositoryError::InvalidInput),
            }
        }
        let name = parts.pop().ok_or(RepositoryError::InvalidInput)?;
        let mut directories =
            vec![Dir::open_ambient_dir(anchor, ambient_authority()).map_err(io_error)?];
        for part in parts {
            let mut options = OpenOptions::new();
            options
                .read(true)
                .maybe_dir(true)
                .follow(FollowSymlinks::No);
            no_delete_share(&mut options);
            let opened = directories
                .last()
                .ok_or(RepositoryError::InvalidInput)?
                .open_with(part, &options)
                .map_err(io_error)?;
            if !opened.metadata().map_err(io_error)?.is_dir() {
                return Err(RepositoryError::InvalidInput);
            }
            directories.push(Dir::from_std_file(opened.into_std()));
        }
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        no_delete_share(&mut options);
        let source = directories
            .last()
            .ok_or(RepositoryError::InvalidInput)?
            .open_with(name, &options)
            .map_err(io_error)?;
        let metadata = source.metadata().map_err(io_error)?;
        let snapshot = fingerprint(&metadata)?;
        if snapshot.size != file.size_bytes
            || file.modified_at.is_some_and(|t| {
                snapshot
                    .modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|m| m.as_millis() as i128 != i128::from(t.timestamp_millis()))
                    .unwrap_or(true)
            })
        {
            return Err(RepositoryError::Conflict);
        }
        Ok(Self {
            path,
            _directories: directories,
            root: root.clone(),
            file: file.clone(),
            source,
            snapshot,
        })
    }
    fn recheck(&self) -> Result<(), RepositoryError> {
        if fingerprint(&self.source.metadata().map_err(io_error)?)? != self.snapshot {
            return Err(RepositoryError::Conflict);
        }
        // Reopen the full pathname too: a renamed Unix ancestor must not cause
        // a pathname-based native launcher to consume a different file.
        let current = Self::open(&self.root, &self.file)?;
        if current.snapshot != self.snapshot {
            return Err(RepositoryError::Conflict);
        }
        Ok(())
    }
}
fn fingerprint(metadata: &Metadata) -> Result<VideoOpenFingerprint, RepositoryError> {
    if !metadata.is_file() {
        return Err(RepositoryError::InvalidInput);
    }
    #[cfg(unix)]
    let identity = {
        use cap_std::fs::MetadataExt;
        format!("{}:{}", metadata.dev(), metadata.ino())
    };
    #[cfg(windows)]
    let identity = "held-windows-no-delete-lease".to_owned();
    #[cfg(not(any(unix, windows)))]
    return Err(RepositoryError::InvalidInput);
    Ok(VideoOpenFingerprint {
        size: metadata.len(),
        modified: metadata.modified().map_err(io_error)?.into_std(),
        identity,
    })
}
fn no_delete_share(options: &mut OpenOptions) {
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.share_mode(0x1 | 0x2);
    }
    #[cfg(not(windows))]
    let _ = options;
}
fn io_error(error: std::io::Error) -> RepositoryError {
    match error.kind() {
        std::io::ErrorKind::NotFound => RepositoryError::NotFound,
        _ => RepositoryError::InvalidInput,
    }
}

async fn ensure_video_file_tx(
    tx: &mut Transaction<'_, Sqlite>,
    file_id: Uuid,
    adult: bool,
) -> Result<(MediaFile, LibraryRoot), RepositoryError> {
    let sql = format!("SELECT m.kind,m.adult_isolated,({}) ready,f.* FROM media_files f JOIN media_items m ON m.id=f.media_item_id WHERE f.id=?", ready_predicate());
    let row = sqlx::query(&sql)
        .bind(file_id.to_string())
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(RepositoryError::NotFound)?;
    let media_id: String = row.try_get("media_item_id")?;
    let kind: String = row.try_get("kind")?;
    let isolated: bool = row.try_get("adult_isolated")?;
    if !matches!(
        kind.as_str(),
        "movie" | "tv_episode" | "anime" | "music_video" | "concert" | "adult_video"
    ) {
        return Err(RepositoryError::InvalidInput);
    }
    if !row.try_get::<bool, _>("ready")?
        || (adult && (kind != "adult_video" || !isolated))
        || (!adult && (kind == "adult_video" || isolated))
    {
        return Err(RepositoryError::InvalidInput);
    }
    let forbidden_root: bool = sqlx::query_scalar(if adult {
        "SELECT EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=? AND (r.kind IS NULL OR r.kind<>'adult_video' OR r.enabled=0))"
    } else {
        "SELECT EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=? AND r.kind='adult_video')"
    }).bind(&media_id).fetch_one(&mut **tx).await?;
    let related_adult: bool = sqlx::query_scalar("WITH RECURSIVE connected(id,depth) AS (SELECT ?,0 UNION SELECT CASE WHEN r.parent_id=c.id THEN r.child_id ELSE r.parent_id END,c.depth+1 FROM connected c JOIN media_relations r ON r.parent_id=c.id OR r.child_id=c.id WHERE c.depth<2) SELECT EXISTS(SELECT 1 FROM connected c JOIN media_items m ON m.id=c.id WHERE m.kind='adult_video' OR m.adult_isolated=1 OR EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=m.id AND r.kind='adult_video'))").bind(&media_id).fetch_one(&mut **tx).await?;
    if forbidden_root || (!adult && related_adult) {
        return Err(RepositoryError::InvalidInput);
    }
    let root_id: String = row.try_get("root_id")?;
    let rootrow = sqlx::query("SELECT * FROM library_roots WHERE id=?")
        .bind(&root_id)
        .fetch_one(&mut **tx)
        .await?;
    if !rootrow.try_get::<bool, _>("enabled")? {
        return Err(RepositoryError::InvalidInput);
    }
    let root_kind: Option<String> = rootrow.try_get("kind")?;
    if root_kind.as_deref().is_some_and(|r| {
        r != kind
            && !(matches!(r, "tv_series" | "tv_season")
                && matches!(kind.as_str(), "tv_episode" | "anime"))
    }) {
        return Err(RepositoryError::InvalidInput);
    }
    let root = LibraryRoot {
        id: parse_id(&root_id)?,
        name: rootrow.try_get("name")?,
        path: rootrow.try_get("path")?,
        path_style: serde_json::from_value(serde_json::Value::String(
            rootrow.try_get("path_style")?,
        ))
        .map_err(|_| RepositoryError::CorruptData)?,
        kind: root_kind
            .map(|k| {
                serde_json::from_value(serde_json::Value::String(k))
                    .map_err(|_| RepositoryError::CorruptData)
            })
            .transpose()?,
        enabled: true,
        created_at: DateTime::from_timestamp_millis(rootrow.try_get("created_at")?)
            .ok_or(RepositoryError::CorruptData)?,
        updated_at: DateTime::from_timestamp_millis(rootrow.try_get("updated_at")?)
            .ok_or(RepositoryError::CorruptData)?,
    };
    let file = MediaFile {
        id: file_id,
        media_item_id: parse_id(&media_id)?,
        root_id: root.id,
        relative_path: row.try_get("relative_path")?,
        size_bytes: u64::try_from(row.try_get::<i64, _>("size_bytes")?)
            .map_err(|_| RepositoryError::CorruptData)?,
        modified_at: row
            .try_get::<Option<i64>, _>("modified_at")?
            .map(|t| DateTime::from_timestamp_millis(t).ok_or(RepositoryError::CorruptData))
            .transpose()?,
        part: row
            .try_get::<Option<i64>, _>("part")?
            .map(|n| u32::try_from(n).map_err(|_| RepositoryError::CorruptData))
            .transpose()?,
    };
    Ok((file, root))
}
fn parse_id(value: &str) -> Result<Uuid, RepositoryError> {
    Uuid::parse_str(value).map_err(|_| RepositoryError::CorruptData)
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use mediaatlas_domain::{MediaItem, MediaKind, PathStyle};
    use std::sync::Mutex;
    #[derive(Default)]
    struct StubOpener {
        calls: Mutex<Vec<PathBuf>>,
        fail: bool,
    }
    impl NativeVideoOpener for StubOpener {
        fn open(&self, path: &Path) -> std::io::Result<()> {
            self.calls.lock().unwrap().push(path.to_owned());
            if self.fail {
                Err(std::io::ErrorKind::NotFound.into())
            } else {
                Ok(())
            }
        }
    }
    async fn fixture() -> (
        tempfile::TempDir,
        LibraryRepository,
        MediaFile,
        LibraryRoot,
        Arc<StubOpener>,
    ) {
        let dir = crate::test_support::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("library.sqlite"))
            .await
            .unwrap();
        let source = dir.path().join("video");
        std::fs::create_dir(&source).unwrap();
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&38_u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&8000_u32.to_le_bytes());
        bytes.extend_from_slice(&16000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        std::fs::write(source.join("movie.mkv"), &bytes).unwrap();
        let root = LibraryRoot {
            id: Uuid::new_v4(),
            name: "Audio".into(),
            path: source.to_str().unwrap().into(),
            path_style: if cfg!(windows) {
                PathStyle::Windows
            } else {
                PathStyle::Posix
            },
            kind: Some(MediaKind::Movie),
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.save_root(&root).await.unwrap();
        let item = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::Movie,
            title: "Temporary video".into(),
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
            relative_path: "movie.mkv".into(),
            size_bytes: bytes.len() as u64,
            modified_at: None,
            part: None,
        };
        repo.save_file(&file).await.unwrap();
        sqlx::query("INSERT INTO local_metadata_provenance(media_item_id,source_kind,source_reference,confirmed_at) VALUES(?,'nfo','temporary verified fixture',0)").bind(item.id.to_string()).execute(repo.pool()).await.unwrap();
        (dir, repo, file, root, Arc::new(StubOpener::default()))
    }
    #[tokio::test]
    async fn only_explicit_open_dispatches_current_verified_file() {
        let (_dir, repo, file, root, opener) = fixture().await;
        let service = VideoOpenService::with_opener(repo.clone(), opener.clone());
        crate::browse::BrowseService::new(repo, PathBuf::from("unused-cache"))
            .list_media(&Default::default())
            .await
            .unwrap();
        assert!(opener.calls.lock().unwrap().is_empty());
        assert_eq!(
            service.open(file.id).await.unwrap(),
            VideoOpenResult { requested: true }
        );
        assert_eq!(
            opener.calls.lock().unwrap().as_slice(),
            [Path::new(&root.path).join(&file.relative_path)]
        );
    }
    #[tokio::test]
    async fn adult_isolation_root_changes_and_non_video_files_never_dispatch() {
        let (_dir, repo, file, _root, opener) = fixture().await;
        let service = VideoOpenService::with_opener(repo.clone(), opener.clone());
        for statement in [
            "UPDATE media_items SET adult_isolated=1",
            "UPDATE media_items SET adult_isolated=0; UPDATE library_roots SET kind='adult_video'",
            "UPDATE library_roots SET kind='movie',enabled=0",
        ] {
            sqlx::raw_sql(statement).execute(repo.pool()).await.unwrap();
            assert_eq!(
                service.open(file.id).await,
                Err(RepositoryError::InvalidInput)
            );
        }
        sqlx::query("UPDATE library_roots SET enabled=1")
            .execute(repo.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE media_files SET relative_path='program.exe'")
            .execute(repo.pool())
            .await
            .unwrap();
        assert_eq!(
            service.open(file.id).await,
            Err(RepositoryError::InvalidInput)
        );
        assert!(opener.calls.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn missing_files_scan_state_changes_and_launcher_failures_report_errors() {
        let (_dir, repo, file, root, opener) = fixture().await;
        let service = VideoOpenService::with_opener(repo.clone(), opener.clone());
        std::fs::write(Path::new(&root.path).join(&file.relative_path), b"changed").unwrap();
        assert_eq!(service.open(file.id).await, Err(RepositoryError::Conflict));
        assert!(opener.calls.lock().unwrap().is_empty());
        std::fs::remove_file(Path::new(&root.path).join(&file.relative_path)).unwrap();
        assert_eq!(service.open(file.id).await, Err(RepositoryError::NotFound));
        let (_dir, repo, file, _root, _opener) = fixture().await;
        let failing = Arc::new(StubOpener {
            calls: Mutex::new(vec![]),
            fail: true,
        });
        let service = VideoOpenService::with_opener(repo, failing.clone());
        assert_eq!(
            service.open(file.id).await,
            Err(RepositoryError::InvalidInput)
        );
        assert_eq!(failing.calls.lock().unwrap().len(), 1);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn final_and_ancestor_symlinks_cannot_reach_native_opener() {
        use std::os::unix::fs::symlink;
        let (dir, repo, file, root, opener) = fixture().await;
        let service = VideoOpenService::with_opener(repo.clone(), opener.clone());
        let source = Path::new(&root.path).join(&file.relative_path);
        let outside = dir.path().join("outside.mkv");
        std::fs::rename(&source, &outside).unwrap();
        symlink(&outside, &source).unwrap();
        assert_eq!(
            service.open(file.id).await,
            Err(RepositoryError::InvalidInput)
        );
        std::fs::remove_file(&source).unwrap();
        std::fs::rename(&outside, &source).unwrap();
        let moved = dir.path().join("moved");
        std::fs::rename(&root.path, &moved).unwrap();
        symlink(&moved, &root.path).unwrap();
        assert_eq!(
            service.open(file.id).await,
            Err(RepositoryError::InvalidInput)
        );
        assert!(opener.calls.lock().unwrap().is_empty());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn final_recheck_detects_same_size_replacement_and_renamed_ancestors() {
        let (dir, _repo, file, root, _opener) = fixture().await;
        let verified = VerifiedVideoPath::open(&root, &file).unwrap();
        let source = Path::new(&root.path).join(&file.relative_path);
        std::fs::rename(&source, dir.path().join("held-original.mkv")).unwrap();
        std::fs::write(&source, vec![0; file.size_bytes as usize]).unwrap();
        assert_eq!(verified.recheck(), Err(RepositoryError::Conflict));
        let verified = VerifiedVideoPath::open(&root, &file).unwrap();
        std::fs::rename(&root.path, dir.path().join("moved")).unwrap();
        std::fs::create_dir(&root.path).unwrap();
        std::fs::write(&source, vec![0; file.size_bytes as usize]).unwrap();
        assert_eq!(verified.recheck(), Err(RepositoryError::Conflict));
    }
    #[tokio::test]
    async fn untrusted_database_relative_path_cannot_dispatch_url_or_traversal() {
        let (_dir, repo, file, _root, opener) = fixture().await;
        let service = VideoOpenService::with_opener(repo.clone(), opener.clone());
        for relative in [
            "../movie.mkv",
            "https://example.com/movie.mkv",
            "/tmp/movie.mkv",
        ] {
            sqlx::query("UPDATE media_files SET relative_path=? WHERE id=?")
                .bind(relative)
                .bind(file.id.to_string())
                .execute(repo.pool())
                .await
                .unwrap();
            assert_eq!(
                service.open(file.id).await,
                Err(RepositoryError::InvalidInput)
            );
        }
        assert!(opener.calls.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn bare_filename_and_revoked_or_wrong_adult_lane_cannot_dispatch() {
        let (_dir, repo, file, _root, opener) = fixture().await;
        let ordinary = VideoOpenService::with_opener(repo.clone(), opener.clone());
        sqlx::query("DELETE FROM local_metadata_provenance")
            .execute(repo.pool())
            .await
            .unwrap();
        assert_eq!(
            ordinary.open(file.id).await,
            Err(RepositoryError::InvalidInput)
        );
        sqlx::query("INSERT INTO local_metadata_provenance VALUES(?,'nfo','fixture',0)")
            .bind(file.media_item_id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE media_items SET kind='adult_video',adult_isolated=1")
            .execute(repo.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE library_roots SET kind='adult_video'")
            .execute(repo.pool())
            .await
            .unwrap();
        assert_eq!(
            ordinary.open(file.id).await,
            Err(RepositoryError::InvalidInput)
        );
        let access = AdultOutputAccess::new();
        let adult =
            VideoOpenService::with_adult_opener(repo.clone(), access.clone(), opener.clone());
        assert!(adult.open(file.id).await.unwrap().requested);
        access.revoke();
        assert!(adult.open(file.id).await.is_err());
        let fresh = VideoOpenService::with_adult_opener(
            repo.clone(),
            AdultOutputAccess::new(),
            opener.clone(),
        );
        sqlx::query("UPDATE library_roots SET kind='movie'")
            .execute(repo.pool())
            .await
            .unwrap();
        assert_eq!(
            fresh.open(file.id).await,
            Err(RepositoryError::InvalidInput)
        );
        assert_eq!(opener.calls.lock().unwrap().len(), 1);
    }
}
