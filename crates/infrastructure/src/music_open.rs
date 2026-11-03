//! Explicit dispatch of a verified local music file to its OS association.
//! Construction and library reads never launch an application.
use crate::{
    library::{LibraryRepository, RepositoryError},
    music_library::{ensure_music_file, ensure_music_file_tx},
};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, File, Metadata, OpenOptions},
};
use mediaatlas_domain::{normalize_relative_path, LibraryRoot, MediaFile};
use serde::{Deserialize, Serialize};
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

/// Implemented by the native adapter and deterministic tests; never exposed over IPC.
pub trait NativeMusicOpener: Send + Sync {
    fn open(&self, path: &Path) -> std::io::Result<()>;
}
struct SystemMusicOpener;
impl NativeMusicOpener for SystemMusicOpener {
    fn open(&self, path: &Path) -> std::io::Result<()> {
        open::that_detached_with_timeout(path, std::time::Duration::from_secs(3))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicOpenResult {
    pub requested: bool,
}
#[derive(Clone)]
pub struct MusicOpenService {
    repo: LibraryRepository,
    opener: Arc<dyn NativeMusicOpener>,
}
impl MusicOpenService {
    pub fn new(repo: LibraryRepository) -> Self {
        Self {
            repo,
            opener: Arc::new(SystemMusicOpener),
        }
    }
    pub fn with_opener(repo: LibraryRepository, opener: Arc<dyn NativeMusicOpener>) -> Self {
        Self { repo, opener }
    }
    /// Call only for a user's explicit Open action. The frontend supplies a file ID,
    /// never a pathname, shell command, URL, or guessed edition.
    pub async fn open(&self, file_id: Uuid) -> Result<MusicOpenResult, RepositoryError> {
        let (file, root) = ensure_music_file(&self.repo, file_id).await?;
        let initial_file = file.clone();
        let initial_root = root.clone();
        let verified = tokio::task::spawn_blocking(move || VerifiedMusicPath::open(&root, &file))
            .await
            .map_err(|_| RepositoryError::Database)??;
        let mut tx = self.repo.pool().begin().await?;
        // The writer reservation prevents concurrent Root/kind/link changes while
        // final authority is checked and the native request is submitted.
        sqlx::query("UPDATE media_files SET size_bytes=size_bytes WHERE id=?")
            .bind(file_id.to_string())
            .execute(&mut *tx)
            .await?;
        let (file, root) = ensure_music_file_tx(&mut tx, file_id).await?;
        if file != initial_file || root != initial_root {
            return Err(RepositoryError::Conflict);
        }
        let opener = self.opener.clone();
        tokio::task::spawn_blocking(move || {
            verified.recheck()?;
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
        Ok(MusicOpenResult { requested: true })
    }
}
struct VerifiedMusicPath {
    path: PathBuf,
    _directories: Vec<Dir>,
    root: LibraryRoot,
    file: MediaFile,
    source: File,
    snapshot: MusicOpenFingerprint,
}
#[derive(PartialEq, Eq)]
struct MusicOpenFingerprint {
    size: u64,
    modified: std::time::SystemTime,
    identity: String,
}
impl VerifiedMusicPath {
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
            "mp3" | "flac" | "m4a" | "ogg" | "opus" | "wav"
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
fn fingerprint(metadata: &Metadata) -> Result<MusicOpenFingerprint, RepositoryError> {
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
    Ok(MusicOpenFingerprint {
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
    impl NativeMusicOpener for StubOpener {
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
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("library.sqlite"))
            .await
            .unwrap();
        let source = dir.path().join("audio");
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
        std::fs::write(source.join("song.wav"), &bytes).unwrap();
        let root = LibraryRoot {
            id: Uuid::new_v4(),
            name: "Audio".into(),
            path: source.to_str().unwrap().into(),
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
            title: "Temporary audio".into(),
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
            relative_path: "song.wav".into(),
            size_bytes: bytes.len() as u64,
            modified_at: None,
            part: None,
        };
        repo.save_file(&file).await.unwrap();
        (dir, repo, file, root, Arc::new(StubOpener::default()))
    }
    #[tokio::test]
    async fn only_explicit_open_dispatches_current_verified_file() {
        let (_dir, repo, file, root, opener) = fixture().await;
        let service = MusicOpenService::with_opener(repo.clone(), opener.clone());
        crate::music_library::MusicLibraryService::new(repo)
            .available_files()
            .await
            .unwrap();
        assert!(opener.calls.lock().unwrap().is_empty());
        assert_eq!(
            service.open(file.id).await.unwrap(),
            MusicOpenResult { requested: true }
        );
        assert_eq!(
            opener.calls.lock().unwrap().as_slice(),
            [Path::new(&root.path).join(&file.relative_path)]
        );
    }
    #[tokio::test]
    async fn adult_isolation_root_changes_and_non_music_files_never_dispatch() {
        let (_dir, repo, file, _root, opener) = fixture().await;
        let service = MusicOpenService::with_opener(repo.clone(), opener.clone());
        for statement in [
            "UPDATE media_items SET adult_isolated=1",
            "UPDATE media_items SET adult_isolated=0; UPDATE library_roots SET kind='adult_video'",
            "UPDATE library_roots SET kind='music_track',enabled=0",
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
        let service = MusicOpenService::with_opener(repo.clone(), opener.clone());
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
        let service = MusicOpenService::with_opener(repo, failing.clone());
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
        let service = MusicOpenService::with_opener(repo.clone(), opener.clone());
        let source = Path::new(&root.path).join(&file.relative_path);
        let outside = dir.path().join("outside.wav");
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
        let verified = VerifiedMusicPath::open(&root, &file).unwrap();
        let source = Path::new(&root.path).join(&file.relative_path);
        std::fs::rename(&source, dir.path().join("held-original.wav")).unwrap();
        std::fs::write(&source, vec![0; file.size_bytes as usize]).unwrap();
        assert_eq!(verified.recheck(), Err(RepositoryError::Conflict));
        let verified = VerifiedMusicPath::open(&root, &file).unwrap();
        std::fs::rename(&root.path, dir.path().join("moved")).unwrap();
        std::fs::create_dir(&root.path).unwrap();
        std::fs::write(&source, vec![0; file.size_bytes as usize]).unwrap();
        assert_eq!(verified.recheck(), Err(RepositoryError::Conflict));
    }
    #[tokio::test]
    async fn untrusted_database_relative_path_cannot_dispatch_url_or_traversal() {
        let (_dir, repo, file, _root, opener) = fixture().await;
        let service = MusicOpenService::with_opener(repo.clone(), opener.clone());
        for relative in [
            "../song.wav",
            "https://example.com/song.wav",
            "/tmp/song.wav",
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
}
