//! Capability-confined NFO exports and bounded artwork cache.
//! Automatic sidecars are preserved; explicit NFO updates never replace media.
use crate::{
    library::{LibraryRepository, RepositoryError},
    network::{CancellationToken, EngineKind, NetworkEngine, RequestSpec},
};
use cap_fs_ext::DirExt;
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use chrono::Utc;
use image::{ImageFormat, ImageReader, Limits};
use mediaatlas_domain::{
    ArtworkKind, MediaItem, MediaKind, MetadataValue, ProviderFailure, ProviderJob,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
};
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

mod kodi_export;

const MAX_NFO: usize = 2 * 1024 * 1024;
const MAX_IMAGE: usize = 8 * 1024 * 1024;
const MAX_PIXELS: u64 = 20_000_000;
const MAX_ADULT_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NfoExportMode {
    #[default]
    Online,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputFileDiff {
    pub destination: String,
    pub kind: String,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub expected_hash: Option<String>,
    pub content_hash: String,
    pub protected: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputPreview {
    pub schema_version: u16,
    pub preview_id: Uuid,
    pub media_item_id: Uuid,
    pub strategy: String,
    pub files: Vec<OutputFileDiff>,
    pub state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputResult {
    pub schema_version: u16,
    pub preview_id: Uuid,
    pub written: usize,
    pub preserved: usize,
    pub already_applied: bool,
    pub state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OutputPlan {
    #[serde(default)]
    channel: OutputChannel,
    preview: OutputPreview,
    root: PathBuf,
    root_id: Uuid,
    source_file_id: Uuid,
    source_relative: String,
    source_fingerprint: SourceFingerprint,
    #[serde(default)]
    series_target: bool,
    #[serde(default)]
    nfo_only: bool,
    relative_parent: PathBuf,
    payloads: Vec<Vec<u8>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SourceFingerprint {
    len: u64,
    modified_nanos: u128,
    identity: String,
}
fn source_fingerprint(
    dir: &Dir,
    name: impl AsRef<Path>,
) -> Result<SourceFingerprint, RepositoryError> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = dir.open_with(name, &options).map_err(io_error)?;
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() {
        return Err(RepositoryError::InvalidInput);
    }
    let modified_nanos = metadata
        .modified()
        .map_err(io_error)?
        .into_std()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| RepositoryError::InvalidInput)?
        .as_nanos();
    let identity = metadata_identity(&metadata)?;
    Ok(SourceFingerprint {
        len: metadata.len(),
        modified_nanos,
        identity,
    })
}
#[cfg(unix)]
fn metadata_identity(metadata: &cap_std::fs::Metadata) -> Result<String, RepositoryError> {
    use cap_std::fs::MetadataExt;
    Ok(format!("{}:{}", metadata.dev(), metadata.ino()))
}
#[cfg(not(unix))]
fn metadata_identity(_: &cap_std::fs::Metadata) -> Result<String, RepositoryError> {
    #[cfg(windows)]
    {
        static SESSION: OnceLock<String> = OnceLock::new();
        Ok(format!(
            "windows-session:{}",
            SESSION.get_or_init(|| Uuid::new_v4().to_string())
        ))
    }
    #[cfg(not(windows))]
    {
        Err(RepositoryError::InvalidInput)
    }
}
// Windows uses a short-lived, process-local no-delete lease rather than
// persisting a truncated FileIndex that can collide on ReFS. Every pathname
// ancestor and the source remain opened without FILE_SHARE_DELETE. This makes
// replacement impossible while the preview is valid. Recovery across processes
// requires a fresh preview; it never falls back to path-only identity.
#[cfg(windows)]
struct WindowsSourceLease {
    adult_access: Option<AdultOutputAccess>,
    _directories: Vec<Dir>,
    _source: cap_std::fs::File,
    created: std::time::Instant,
}
#[cfg(windows)]
fn windows_leases() -> &'static Mutex<std::collections::HashMap<Uuid, Arc<WindowsSourceLease>>> {
    static LEASES: OnceLock<Mutex<std::collections::HashMap<Uuid, Arc<WindowsSourceLease>>>> =
        OnceLock::new();
    LEASES.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}
#[cfg(windows)]
fn windows_source_lease(
    root: &Path,
    relative: &Path,
    adult_access: Option<AdultOutputAccess>,
) -> Result<WindowsSourceLease, RepositoryError> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
    use cap_std::fs::OpenOptionsExt;
    let path = root.join(relative);
    let mut anchor = PathBuf::new();
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(p) => anchor.push(p.as_os_str()),
            Component::RootDir => anchor.push(component.as_os_str()),
            Component::Normal(v) => parts.push(v.to_os_string()),
            _ => return Err(RepositoryError::InvalidInput),
        }
    }
    let source = parts.pop().ok_or(RepositoryError::InvalidInput)?;
    let mut directories =
        vec![Dir::open_ambient_dir(anchor, ambient_authority()).map_err(io_error)?];
    // Windows FILE_SHARE_READ | FILE_SHARE_WRITE deliberately excludes DELETE.
    for name in parts {
        let mut options = OpenOptions::new();
        options
            .read(true)
            .maybe_dir(true)
            .follow(FollowSymlinks::No)
            .share_mode(0x1 | 0x2);
        let file = directories
            .last()
            .ok_or(RepositoryError::InvalidInput)?
            .open_with(name, &options)
            .map_err(io_error)?;
        if !file.metadata().map_err(io_error)?.is_dir() {
            return Err(RepositoryError::InvalidInput);
        }
        directories.push(Dir::from_std_file(file.into_std()));
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .follow(FollowSymlinks::No)
        .share_mode(0x1 | 0x2);
    let source = directories
        .last()
        .ok_or(RepositoryError::InvalidInput)?
        .open_with(source, &options)
        .map_err(io_error)?;
    if !source.metadata().map_err(io_error)?.is_file() {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(WindowsSourceLease {
        adult_access,
        _directories: directories,
        _source: source,
        created: std::time::Instant::now(),
    })
}
#[cfg(windows)]
async fn check_windows_lease(id: Uuid) -> Result<Arc<WindowsSourceLease>, RepositoryError> {
    let mut leases = windows_leases().lock().await;
    leases.retain(|_, lease| lease.created.elapsed() < std::time::Duration::from_secs(600));
    let lease = leases.get(&id).cloned().ok_or(RepositoryError::Conflict)?;
    if let Some(access) = &lease.adult_access {
        access.ensure()?;
    }
    Ok(lease)
}
fn existing_nfo_name(
    dir: &Dir,
    desired: &str,
    generic: bool,
) -> Result<Option<String>, RepositoryError> {
    let mut matching = None;
    let mut movie = None;
    for (index, entry) in dir.entries().map_err(io_error)?.enumerate() {
        if index >= 10000 {
            return Err(RepositoryError::InvalidInput);
        }
        let name = entry.map_err(io_error)?.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if generic && name.eq_ignore_ascii_case("movie.nfo") {
            if movie.replace(name.to_owned()).is_some() {
                return Err(RepositoryError::Conflict);
            }
        } else if name.eq_ignore_ascii_case(desired) && matching.replace(name.to_owned()).is_some()
        {
            return Err(RepositoryError::Conflict);
        }
    }
    Ok(movie.or(matching))
}
fn is_video_source(relative: &str) -> bool {
    Path::new(relative)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            crate::library_ready::VIDEO_EXTENSIONS.contains(&value.to_ascii_lowercase().as_str())
        })
}
fn verify_source(plan: &OutputPlan) -> Result<(), RepositoryError> {
    let dir = descend(
        open_absolute_dir(&plan.root, false).map_err(io_error)?,
        checked_relative(&plan.source_relative)?
            .parent()
            .unwrap_or_else(|| Path::new("")),
        false,
    )
    .map_err(io_error)?;
    let relative = checked_relative(&plan.source_relative)?;
    let fingerprint = source_fingerprint(
        &dir,
        relative.file_name().ok_or(RepositoryError::CorruptData)?,
    )?;
    if fingerprint != plan.source_fingerprint {
        return Err(RepositoryError::Conflict);
    }
    Ok(())
}
/// An in-process privacy grant issued only by the adult privacy service.
/// It is deliberately absent from serde/IPC and is revoked across cloned services.
#[derive(Clone)]
pub struct AdultOutputAccess(Arc<AtomicBool>);
impl AdultOutputAccess {
    pub(crate) fn new() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }
    pub(crate) fn revoke(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
    pub fn ensure(&self) -> Result<(), RepositoryError> {
        if self.0.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(RepositoryError::Provider(ProviderFailure::Cancelled))
        }
    }
}
/// Lock cleanup never needs a usable grant or a UI round trip. Active executions
/// retain their own handle until completion, while unused previews release now.
pub(crate) async fn release_revoked_adult_leases() {
    #[cfg(windows)]
    windows_leases().lock().await.retain(|_, lease| {
        lease
            .adult_access
            .as_ref()
            .is_none_or(|access| access.ensure().is_ok())
    });
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OutputChannel {
    #[default]
    Ordinary,
    Adult,
}
#[derive(Clone)]
pub struct MediaOutputService {
    repo: LibraryRepository,
    base_cache: PathBuf,
    gate: Arc<Mutex<()>>,
    channel: OutputChannel,
    adult_access: Option<AdultOutputAccess>,
    network: Option<Arc<NetworkEngine>>,
}
impl MediaOutputService {
    pub fn new(repo: LibraryRepository, base_cache: PathBuf) -> Self {
        Self {
            repo,
            base_cache,
            gate: Arc::default(),
            channel: OutputChannel::Ordinary,
            adult_access: None,
            network: None,
        }
    }
    /// Trusted backend composition only; IPC cannot turn an ordinary plan adult.
    pub fn new_adult(
        repo: LibraryRepository,
        base_cache: PathBuf,
        access: AdultOutputAccess,
    ) -> Self {
        Self {
            repo,
            base_cache,
            gate: Arc::default(),
            channel: OutputChannel::Adult,
            adult_access: Some(access),
            network: None,
        }
    }
    pub fn with_network_engine(mut self, network: Arc<NetworkEngine>) -> Self {
        self.network = Some(network);
        self
    }
    #[cfg(test)]
    pub(crate) async fn lock_for_test(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.gate.clone().lock_owned().await
    }
    pub(crate) fn ensure_access(&self) -> Result<(), RepositoryError> {
        if let Some(access) = &self.adult_access {
            access.ensure()?;
        }
        Ok(())
    }
    pub(crate) fn is_adult_channel(&self) -> bool {
        self.channel == OutputChannel::Adult
    }
    /// Opening/recovering never replays file I/O; execution needs a fresh UI call.
    pub async fn recover_interrupted(&self) -> Result<u64, RepositoryError> {
        Ok(sqlx::query("UPDATE file_operations SET state='interrupted',updated_at=MAX(updated_at,?) WHERE state='running' AND id IN (SELECT id FROM output_previews)")
            .bind(Utc::now().timestamp_millis()).execute(self.repo.pool()).await?.rows_affected())
    }
    pub async fn preview(&self, media_id: Uuid) -> Result<OutputPreview, RepositoryError> {
        self.preview_inner(media_id, false, None, NfoExportMode::Offline, false).await
    }
    pub async fn preview_with_mode(
        &self,
        media_id: Uuid,
        mode: NfoExportMode,
    ) -> Result<OutputPreview, RepositoryError> {
        if self.channel != OutputChannel::Ordinary {
            return Err(RepositoryError::InvalidInput);
        }
        self.preview_inner(media_id, false, None, mode, true).await
    }
    async fn preview_inner(
        &self,
        media_id: Uuid,
        nfo_only: bool,
        source_id: Option<Uuid>,
        mode: NfoExportMode,
        update_existing: bool,
    ) -> Result<OutputPreview, RepositoryError> {
        let _gate = self.gate.lock().await;
        self.ensure_access()?;
        let item = self
            .repo
            .get_media(media_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let files = self.output_files(media_id).await?;
        let source = files
            .iter()
            .find(|file| {
                source_id.is_none_or(|id| file.id == id) && is_video_source(&file.relative_path)
            })
            .ok_or(RepositoryError::NotFound)?;
        let root = self
            .repo
            .get_root(source.root_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        self.ensure_channel_media(media_id).await?;
        if !root.enabled || !self.root_matches_channel(root.kind) {
            return Err(RepositoryError::InvalidInput);
        }
        let relative = checked_relative(&source.relative_path)?;
        let source_parent = relative
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let parent = if matches!(item.kind, MediaKind::TvSeries | MediaKind::TvSeason) {
            let mut parent = source_parent.clone();
            for file in files.iter().filter(|file| {
                file.root_id == source.root_id && is_video_source(&file.relative_path)
            }) {
                let path = checked_relative(&file.relative_path)?;
                while !path
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .starts_with(&parent)
                {
                    parent.pop();
                }
            }
            // The only safe automatic series target is its actual common local
            // directory, never a provider-derived path or title.
            if item.kind == MediaKind::TvSeries
                && parent
                    .file_name()
                    .and_then(|v| v.to_str())
                    .is_some_and(|name| {
                        let name = name.to_ascii_lowercase().replace([' ', '.', '_', '-'], "");
                        name.strip_prefix("season")
                            .or_else(|| name.strip_prefix('s'))
                            .is_some_and(|number| {
                                !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
                            })
                    })
            {
                parent.pop();
            }
            parent
        } else {
            source_parent.clone()
        };
        if (nfo_only || update_existing) && matches!(item.kind, MediaKind::TvSeries | MediaKind::TvSeason) {
            let mut connection = self.repo.pool().acquire().await?;
            self.ensure_series_directory(media_id, root.id, &parent, &mut connection)
                .await?;
        }
        let source_dir = open_absolute_dir(Path::new(&root.path), false)
            .and_then(|d| descend(d, &source_parent, false))
            .map_err(io_error)?;
        let dir = open_absolute_dir(Path::new(&root.path), false)
            .and_then(|d| descend(d, &parent, false))
            .map_err(io_error)?;
        let source_name = relative.file_name().ok_or(RepositoryError::InvalidInput)?;
        let meta = source_dir.symlink_metadata(source_name).map_err(io_error)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(RepositoryError::InvalidInput);
        }
        #[cfg(windows)]
        let windows_lease =
            windows_source_lease(Path::new(&root.path), &relative, self.adult_access.clone())?;
        let source_fingerprint = source_fingerprint(&source_dir, source_name)?;
        let mut metadata = self.repo.list_metadata_values(media_id).await?;
        let mut ids = self.repo.list_external_ids(media_id).await?;
        // Once an identity was corrected, old sidecars cannot refill metadata
        // from the misidentified work, even before that NFO is explicitly updated.
        let corrected_identity: bool = if self.channel == OutputChannel::Ordinary && update_existing {
            self.had_identity_correction(media_id).await?
        } else { false };
        let name = relative
            .file_stem()
            .and_then(|v| v.to_str())
            .ok_or(RepositoryError::InvalidInput)?;
        let desired_nfo = if matches!(item.kind, MediaKind::TvSeries | MediaKind::TvSeason) {
            "tvshow.nfo".to_owned()
        } else {
            format!("{name}.nfo")
        };
        let nfo_name = if nfo_only || update_existing {
            // Producer and filename casing do not authorize duplicate sidecars.
            // A generic movie.nfo represents all versions in this directory.
            existing_nfo_name(
                &dir,
                &desired_nfo,
                !matches!(item.kind, MediaKind::TvSeries | MediaKind::TvSeason),
            )?
            .unwrap_or(desired_nfo)
        } else {
            desired_nfo
        };
        let old = read_optional(&dir, &nfo_name, MAX_NFO)?;
        let old_text = old
            .as_ref()
            .map(|b| {
                std::str::from_utf8(b)
                    .map(str::to_owned)
                    .map_err(|_| RepositoryError::InvalidInput)
            })
            .transpose()?;
        if nfo_only && old.is_none() {
            let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM output_previews p JOIN file_operations f ON f.id=p.id WHERE p.media_item_id=? AND f.state IN ('running','failed','interrupted') AND json_extract(p.snapshot,'$.preview.strategy')='update_nfo')")
                .bind(media_id.to_string()).fetch_one(self.repo.pool()).await?;
            if pending { return Err(RepositoryError::Conflict); }
        }
        if let Some(existing) = old_text.as_ref().filter(|_| !corrected_identity) {
            let local = mediaatlas_media_scanner::parse_nfo(existing)
                .map_err(|_| RepositoryError::InvalidInput)?;
            let saved_keys = metadata.iter().map(|field| field.field_key.clone()).collect::<std::collections::HashSet<_>>();
            for id in local.external_ids {
                if !ids.iter().any(|known| known.namespace == id.namespace) {
                    ids.push(mediaatlas_domain::ExternalId {
                        id: Uuid::new_v4(), media_item_id: media_id,
                        provider_id: id.provider_id, namespace: id.namespace,
                        external_id: id.value, is_primary: false, created_at: Utc::now(),
                    });
                }
            }
            for (key, value) in local.fields {
                let aliases: &[&str] = match key.as_str() {
                    "cast" | "people" => &["cast", "people"],
                    "ratings" | "rating" | "votes" => &["ratings", "rating", "votes"],
                    "studios" | "production_companies" => &["studios", "production_companies"],
                    "writers" | "credits" => &["writers", "credits"],
                    "collection" | "series" => &["collection", "series"],
                    "premiered" | "release_date" => &["premiered", "release_date"],
                    "runtime" | "runtime_minutes" => &["runtime", "runtime_minutes"],
                    _ => &[],
                };
                if !saved_keys.contains(&key) && !aliases.iter().any(|alias| saved_keys.contains(*alias)) {
                    metadata.push(MetadataValue {
                        id: Uuid::new_v4(), media_item_id: media_id, field_key: key,
                        locale: None, value, provider_id: Some("local_nfo".into()),
                        source_url: None, fetched_at: None, confidence: None,
                        is_user_modified: false, is_locked: false,
                    });
                }
            }
        }
        let prepared = if self.channel == OutputChannel::Ordinary && (update_existing || (nfo_only && old.is_none())) {
            self.prepare_kodi_images(&item, &metadata, name, &dir, mode, source.root_id, &parent).await?
        } else {
            kodi_export::PreparedExport {
                artwork: Vec::new(),
                actor_thumbs: None,
                images: Vec::new(),
            }
        };
        let generated = crate::kodi_nfo::render_nfo(
            &item,
            &metadata,
            &ids,
            &prepared.artwork,
            prepared.actor_thumbs.as_ref(),
        )?;
        // Automatic output preserves existing XML; explicit export reviews a
        // merged before/after image while retaining unrelated top-level XML.
        let nfo = if let Some(old) = &old_text {
            validate_nfo(old)?;
            if nfo_only {
                let document =
                    roxmltree::Document::parse(old).map_err(|_| RepositoryError::InvalidInput)?;
                if !document.root_element().children().any(|node| {
                    node.has_tag_name("title")
                        && node.text().is_some_and(|title| !title.trim().is_empty())
                }) {
                    // Preserve unusable external XML, but do not claim that an
                    // empty sidecar is a successful automatic metadata export.
                    return Err(RepositoryError::InvalidInput);
                }
            }
            if update_existing {
                merge_kodi_export(old, &generated, corrected_identity)?
            } else {
                old.clone()
            }
        } else {
            generated.clone()
        };
        let mut diffs = vec![OutputFileDiff {
            destination: parent.join(&nfo_name).to_string_lossy().replace('\\', "/"),
            kind: "nfo".into(),
            old_text,
            new_text: Some(nfo.clone()),
            expected_hash: old.as_ref().map(|b| hash(b)),
            content_hash: hash(nfo.as_bytes()),
            protected: old.is_some() && !update_existing,
        }];
        let mut payloads = vec![nfo.into_bytes()];
        if let Some(existing) = old.as_ref().filter(|_| !nfo_only && !update_existing) {
            let existing =
                std::str::from_utf8(existing).map_err(|_| RepositoryError::InvalidInput)?;
            let generated_doc = roxmltree::Document::parse(&generated)
                .map_err(|_| RepositoryError::InvalidInput)?;
            let fields: BTreeMap<String, String> = generated_doc
                .root_element()
                .children()
                .filter(roxmltree::Node::is_element)
                .filter_map(|node| {
                    if node.tag_name().name() == "uniqueid" {
                        None
                    } else {
                        node.text()
                            .map(|text| (node.tag_name().name().to_owned(), text.to_owned()))
                    }
                })
                .collect();
            let protected: Vec<String> = roxmltree::Document::parse(existing)
                .map_err(|_| RepositoryError::InvalidInput)?
                .root_element()
                .children()
                .filter(roxmltree::Node::is_element)
                .map(|node| node.tag_name().name().to_owned())
                .collect();
            let revised = merge_nfo_known_fields(existing, &fields, &protected)?;
            if revised != existing {
                // A content-addressed alternate is app-owned, so future exports
                // can add a new revision without replacing a manual sidecar.
                let content_hash = hash(revised.as_bytes());
                let revised_name = format!("{name}.mediaatlas-{}.nfo", &content_hash[..12]);
                let previous = read_optional(&dir, &revised_name, MAX_NFO)?;
                diffs.push(OutputFileDiff {
                    destination: parent.join(&revised_name).to_string_lossy().replace('\\', "/"),
                    kind: "nfo".into(),
                    old_text: previous
                        .as_ref()
                        .and_then(|b| String::from_utf8(b.clone()).ok()),
                    new_text: Some(revised.clone()),
                    expected_hash: previous.as_ref().map(|b| hash(b)),
                    content_hash,
                    protected: previous.is_some(),
                });
                payloads.push(revised.into_bytes());
            }
        }
        if self.channel == OutputChannel::Ordinary {
            for image in prepared.images {
                let relative_name = checked_relative(&image.name)?;
                if relative_name.components().count() != 1 {
                    return Err(RepositoryError::InvalidInput);
                }
                let existing = read_optional(&dir, &relative_name, MAX_IMAGE)?;
                if let Some(bytes) = &existing {
                    decode_image(bytes).map_err(|_| RepositoryError::InvalidInput)?;
                    if bytes != &image.bytes {
                        return Err(RepositoryError::Conflict);
                    }
                }
                diffs.push(OutputFileDiff {
                    destination: parent.join(&relative_name).to_string_lossy().replace('\\', "/"),
                    kind: image.kind,
                    old_text: None,
                    new_text: None,
                    expected_hash: existing.as_ref().map(|bytes| hash(bytes)),
                    content_hash: hash(&image.bytes),
                    protected: existing.is_some(),
                });
                payloads.push(image.bytes);
            }
        }
        // Selected artwork is persisted by the merge workflow. Only validated
        // content-addressed image bytes inside this service's cache are eligible.
        let mut artwork_counts = BTreeMap::<String, usize>::new();
        for art in self
            .repo
            .list_artworks(media_id)
            .await?
            .into_iter()
            .filter(|_| !nfo_only && (self.channel == OutputChannel::Adult || !update_existing))
        {
            if !self.artwork_allowed(art.kind) {
                continue;
            }
            let (Some(cache_path), Some(expected_hash)) = (&art.cache_path, &art.content_hash)
            else {
                continue;
            };
            let cache = open_absolute_dir(&self.base_cache, false).map_err(io_error)?;
            let cache_path = Path::new(cache_path);
            let relative_cache = cache_path
                .strip_prefix(&self.base_cache)
                .map_err(|_| RepositoryError::InvalidInput)?;
            let relative_cache = checked_relative(&relative_cache.to_string_lossy())?;
            if relative_cache.components().count() != 1 {
                return Err(RepositoryError::InvalidInput);
            }
            let bytes = read_optional(&cache, &relative_cache, MAX_IMAGE)?
                .ok_or(RepositoryError::NotFound)?;
            if hash(&bytes) != *expected_hash {
                return Err(RepositoryError::Conflict);
            }
            let (format, _, _) = decode_image(&bytes).map_err(|_| RepositoryError::InvalidInput)?;
            let extension = extension(format).ok_or(RepositoryError::InvalidInput)?;
            let kind = art.kind.as_str();
            let count = artwork_counts.entry(kind.to_owned()).or_default();
            *count += 1;
            if *count
                > if art.kind == ArtworkKind::Sample {
                    20
                } else {
                    1
                }
            {
                return Err(RepositoryError::InvalidInput);
            }
            let suffix = if art.kind == ArtworkKind::Sample {
                format!("sample-{count:02}")
            } else {
                kind.into()
            };
            let poster_name = format!("{name}-{suffix}.{extension}");
            let existing = read_optional(&dir, &poster_name, MAX_IMAGE)?;
            diffs.push(OutputFileDiff {
                destination: parent.join(&poster_name).to_string_lossy().replace('\\', "/"),
                kind: kind.into(),
                old_text: None,
                new_text: None,
                expected_hash: existing.as_ref().map(|b| hash(b)),
                content_hash: hash(&bytes),
                protected: existing.is_some(),
            });
            if self.channel == OutputChannel::Adult
                && payloads
                    .iter()
                    .map(Vec::len)
                    .sum::<usize>()
                    .saturating_add(bytes.len())
                    > MAX_ADULT_OUTPUT_BYTES
            {
                return Err(RepositoryError::Provider(ProviderFailure::ResponseTooLarge));
            }
            payloads.push(bytes);
            if self.channel == OutputChannel::Ordinary {
                break;
            }
        }
        self.ensure_access()?;
        if self.channel == OutputChannel::Ordinary && payloads.iter().map(Vec::len).sum::<usize>() > MAX_ADULT_OUTPUT_BYTES {
            return Err(RepositoryError::Provider(ProviderFailure::ResponseTooLarge));
        }
        let preview = OutputPreview {
            schema_version: 1,
            preview_id: Uuid::new_v4(),
            media_item_id: media_id,
            strategy: if update_existing && old.is_some() { "update_nfo" } else { "preserve" }.into(),
            files: diffs,
            state: "planned".into(),
        };
        let plan = OutputPlan {
            channel: self.channel,
            preview: preview.clone(),
            root: PathBuf::from(root.path),
            root_id: root.id,
            source_file_id: source.id,
            source_relative: source.relative_path.clone(),
            source_fingerprint,
            nfo_only,
            series_target: (nfo_only || update_existing)
                && matches!(item.kind, MediaKind::TvSeries | MediaKind::TvSeason),
            relative_parent: parent,
            payloads,
        };
        let serialized = serde_json::to_string(&plan).map_err(|_| RepositoryError::InvalidInput)?;
        let now = Utc::now().timestamp_millis();
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("INSERT INTO file_operations(id,plan,state,created_at,updated_at) VALUES (?,?,'planned',?,?)").bind(preview.preview_id.to_string()).bind(&serialized).bind(now).bind(now).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO output_previews(id,media_item_id,snapshot) VALUES (?,?,?)")
            .bind(preview.preview_id.to_string())
            .bind(media_id.to_string())
            .bind(serialized)
            .execute(&mut *tx)
            .await?;
        self.ensure_access()?;
        tx.commit().await?;
        #[cfg(windows)]
        {
            let mut leases = windows_leases().lock().await;
            leases.retain(|_, lease| lease.created.elapsed() < std::time::Duration::from_secs(600));
            if leases.len() >= 32 {
                if let Some(oldest) = leases
                    .iter()
                    .min_by_key(|(_, lease)| lease.created)
                    .map(|(id, _)| *id)
                {
                    leases.remove(&oldest);
                }
            }
            leases.insert(preview.preview_id, Arc::new(windows_lease));
            let preview_id = preview.preview_id;
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(600)).await;
                windows_leases().lock().await.remove(&preview_id);
            });
        }
        if let Err(error) = self.ensure_access() {
            let _ = self.discard(preview.preview_id).await;
            return Err(error);
        }
        Ok(preview)
    }
    /// User-authorized default export: create only absent NFO sidecars. Never
    /// outputs artwork or alternate revisions, replaces an existing file, or
    /// converts a failed file export into a failed metadata merge.
    pub(crate) async fn automatic_nfo(
        &self,
        media_id: Uuid,
        cancel: Option<&CancellationToken>,
        context: Option<&crate::scraper::automatic::AutomaticContext>,
    ) -> Result<(), RepositoryError> {
        self.ensure_access()?;
        let result = self.automatic_nfo_inner(media_id, cancel, context).await;
        self.ensure_access()?;
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
        }
        let (state, code) = match &result {
            Ok(written) => (if *written > 0 { "written" } else { "preserved" }, None),
            Err(RepositoryError::Provider(ProviderFailure::Cancelled)) => {
                return result.map(|_| ())
            }
            Err(_) => ("failed", Some("nfo_output_failed")),
        };
        sqlx::query("INSERT INTO automatic_nfo_exports(media_item_id,state,error_code,updated_at) VALUES (?,?,?,?) ON CONFLICT(media_item_id) DO UPDATE SET state=excluded.state,error_code=excluded.error_code,updated_at=excluded.updated_at")
            .bind(media_id.to_string()).bind(state).bind(code).bind(Utc::now().timestamp_millis())
            .execute(self.repo.pool()).await?;
        // File failures are represented separately and retried by the next
        // refresh or the existing explicit output workflow.
        Ok(())
    }
    async fn automatic_nfo_inner(
        &self,
        media_id: Uuid,
        cancel: Option<&CancellationToken>,
        context: Option<&crate::scraper::automatic::AutomaticContext>,
    ) -> Result<usize, RepositoryError> {
        self.ensure_channel_media(media_id).await?;
        let files = self.output_files(media_id).await?;
        let sources = files
            .into_iter()
            .filter(|file| {
                Path::new(&file.relative_path)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| {
                        crate::library_ready::VIDEO_EXTENSIONS
                            .contains(&ext.to_ascii_lowercase().as_str())
                    })
            })
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Err(RepositoryError::NotFound);
        }
        let mut written = 0;
        let mut failure = None;
        for source in sources {
            match self
                .automatic_nfo_source(media_id, source, cancel, context)
                .await
            {
                Ok(count) => written += count,
                Err(error @ RepositoryError::Provider(ProviderFailure::Cancelled)) => {
                    return Err(error)
                }
                Err(error) => failure = Some(error),
            }
        }
        failure.map_or(Ok(written), Err)
    }
    async fn automatic_nfo_source(
        &self,
        media_id: Uuid,
        source: mediaatlas_domain::MediaFile,
        cancel: Option<&CancellationToken>,
        context: Option<&crate::scraper::automatic::AutomaticContext>,
    ) -> Result<usize, RepositoryError> {
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
        }
        let preview = self.preview_inner(media_id, true, Some(source.id), NfoExportMode::Online, false).await?;
        let execution = self
            .execute_inner(preview.preview_id, cancel, context)
            .await;
        #[cfg(windows)]
        if execution.is_ok() {
            windows_leases().lock().await.remove(&preview.preview_id);
        }
        if execution.is_err() {
            let _ = self.discard(preview.preview_id).await;
        }
        let result = execution?;
        // Sidecar output has its own index. Do not mutate the source-file
        // inventory here: queue confirmations bind its exact immutable scope,
        // and NFO must not appear as a newly selectable native video target.
        Ok(result.written)
    }
    /// Release platform source leases when a user dismisses an output preview.
    /// The persisted immutable plan remains as history and cannot regain a lease.
    pub async fn discard(&self, preview_id: Uuid) -> Result<(), RepositoryError> {
        let snapshot: Option<String> =
            sqlx::query_scalar("SELECT snapshot FROM output_previews WHERE id=?")
                .bind(preview_id.to_string())
                .fetch_optional(self.repo.pool())
                .await?;
        let Some(snapshot) = snapshot else {
            return Err(RepositoryError::NotFound);
        };
        let plan: OutputPlan =
            serde_json::from_str(&snapshot).map_err(|_| RepositoryError::CorruptData)?;
        if plan.channel != self.channel {
            return Err(RepositoryError::InvalidInput);
        }
        #[cfg(windows)]
        {
            windows_leases().lock().await.remove(&preview_id);
        }
        sqlx::query("UPDATE file_operations SET state='interrupted',updated_at=MAX(updated_at,?) WHERE id=? AND state IN ('planned','failed') AND id IN (SELECT id FROM output_previews)").bind(Utc::now().timestamp_millis()).bind(preview_id.to_string()).execute(self.repo.pool()).await?;
        Ok(())
    }
    fn root_matches_channel(&self, kind: Option<MediaKind>) -> bool {
        match self.channel {
            OutputChannel::Ordinary => kind != Some(MediaKind::AdultVideo),
            OutputChannel::Adult => kind == Some(MediaKind::AdultVideo),
        }
    }
    fn artwork_allowed(&self, kind: ArtworkKind) -> bool {
        match self.channel {
            OutputChannel::Ordinary => kind == ArtworkKind::Poster,
            OutputChannel::Adult => matches!(
                kind,
                ArtworkKind::Cover | ArtworkKind::BackCover | ArtworkKind::Sample
            ),
        }
    }
    async fn ensure_series_directory(
        &self,
        media_id: Uuid,
        root_id: Uuid,
        parent: &Path,
        connection: &mut sqlx::SqliteConnection,
    ) -> Result<(), RepositoryError> {
        let typed = crate::library_ready::typed_descendant("parent", "m");
        let owned: Vec<String> = sqlx::query_scalar(&format!("SELECT f.id FROM media_files f WHERE f.media_item_id=? OR EXISTS(SELECT 1 FROM media_items m JOIN media_items parent ON parent.id=? WHERE m.id=f.media_item_id AND ({typed}))"))
            .bind(media_id.to_string()).bind(media_id.to_string()).fetch_all(&mut *connection).await?;
        let owned = owned.into_iter().collect::<std::collections::HashSet<_>>();
        let prefix = parent
            .to_string_lossy()
            .replace('\\', "/")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = if prefix.is_empty() {
            "%".to_owned()
        } else {
            format!("{prefix}/%")
        };
        let rows = sqlx::query("SELECT id,relative_path FROM media_files WHERE root_id=? AND relative_path LIKE ? ESCAPE '\\' LIMIT 10001")
            .bind(root_id.to_string()).bind(pattern).fetch_all(&mut *connection).await?;
        if rows.len() > 10000 {
            return Err(RepositoryError::InvalidInput);
        }
        for row in rows {
            if is_video_source(&row.try_get::<String, _>("relative_path")?)
                && !owned.contains(&row.try_get::<String, _>("id")?)
            {
                // Flat mixed-show roots cannot own a single tvshow.nfo. Do not
                // let the first series silently identify all the other shows.
                return Err(RepositoryError::Conflict);
            }
        }
        Ok(())
    }
    async fn output_files(
        &self,
        media_id: Uuid,
    ) -> Result<Vec<mediaatlas_domain::MediaFile>, RepositoryError> {
        let item = self
            .repo
            .get_media(media_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let mut files = self.repo.list_files(media_id).await?;
        if matches!(item.kind, MediaKind::TvSeries | MediaKind::TvSeason)
            && self.channel == OutputChannel::Ordinary
        {
            let typed = crate::library_ready::typed_descendant("parent", "m");
            let ancestor = crate::library_ready::ancestor_isolation("m");
            let children: Vec<String> = sqlx::query_scalar(&format!("SELECT m.id FROM media_items m JOIN media_items parent ON parent.id=? WHERE ({typed}) AND m.adult_isolated=0 AND m.kind<>'adult_video' AND ({ancestor})"))
                .bind(media_id.to_string()).fetch_all(self.repo.pool()).await?;
            for child in children {
                let id = Uuid::parse_str(&child).map_err(|_| RepositoryError::CorruptData)?;
                files.extend(self.repo.list_files(id).await?);
            }
        }
        files.sort_by(|left, right| {
            (&left.part, &left.relative_path, &left.id).cmp(&(
                &right.part,
                &right.relative_path,
                &right.id,
            ))
        });
        files.dedup_by_key(|file| file.id);
        Ok(files)
    }
    async fn ensure_channel_media(&self, media_id: Uuid) -> Result<(), RepositoryError> {
        self.ensure_access()?;
        let isolated: bool =
            sqlx::query_scalar("SELECT adult_isolated FROM media_items WHERE id=?")
                .bind(media_id.to_string())
                .fetch_one(self.repo.pool())
                .await?;
        let item = self
            .repo
            .get_media(media_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let valid = match self.channel {
            OutputChannel::Ordinary => {
                !isolated
                    && matches!(
                        item.kind,
                        MediaKind::Movie
                            | MediaKind::TvSeries
                            | MediaKind::TvSeason
                            | MediaKind::TvEpisode
                            | MediaKind::Anime
                    )
            }
            OutputChannel::Adult => isolated && item.kind == MediaKind::AdultVideo,
        };
        if !valid {
            return Err(RepositoryError::InvalidInput);
        }
        let ancestor = crate::library_ready::ancestor_isolation("m");
        if self.channel == OutputChannel::Ordinary {
            let ordinary: bool = sqlx::query_scalar(&format!(
                "SELECT ({ancestor}) FROM media_items m WHERE m.id=?"
            ))
            .bind(media_id.to_string())
            .fetch_one(self.repo.pool())
            .await?;
            if !ordinary {
                return Err(RepositoryError::InvalidInput);
            }
        }
        for file in self.output_files(media_id).await? {
            let root = self
                .repo
                .get_root(file.root_id)
                .await?
                .ok_or(RepositoryError::NotFound)?;
            if !root.enabled {
                return Err(RepositoryError::Conflict);
            }
            if !self.root_matches_channel(root.kind) {
                return Err(RepositoryError::InvalidInput);
            }
        }
        Ok(())
    }
    async fn had_identity_correction(&self, media_id: Uuid) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM video_merge_previews p, json_each(p.preview_json,'$.external_ids') d WHERE p.media_item_id=? AND p.state='succeeded' AND json_array_length(json_extract(d.value,'$.old'))>0 AND json_extract(d.value,'$.old')!=json_extract(d.value,'$.new'))")
            .bind(media_id.to_string()).fetch_one(self.repo.pool()).await?)
    }

    pub async fn execute(&self, preview_id: Uuid) -> Result<OutputResult, RepositoryError> {
        self.execute_cancellable(preview_id, None).await
    }
    pub(crate) async fn execute_cancellable(
        &self,
        preview_id: Uuid,
        cancel: Option<&CancellationToken>,
    ) -> Result<OutputResult, RepositoryError> {
        let result = self.execute_inner(preview_id, cancel, None).await;
        #[cfg(windows)]
        if result.is_ok() {
            // A rejected channel/root check or an external sidecar collision
            // must not consume another channel's lease or a retryable preview.
            // Discard, privacy lock and the bounded TTL release unused leases.
            windows_leases().lock().await.remove(&preview_id);
        }
        if let Ok(output) = &result {
            self.ensure_access()?;
            // Explicit recovery/export also resolves a previous automatic file
            // failure, without waiting for a scan or altering its source proof.
            sqlx::query("INSERT INTO automatic_nfo_exports(media_item_id,state,error_code,updated_at) SELECT media_item_id,?,NULL,? FROM output_previews WHERE id=? ON CONFLICT(media_item_id) DO UPDATE SET state=excluded.state,error_code=NULL,updated_at=excluded.updated_at")
                .bind(if output.written > 0 { "written" } else { "preserved" })
                .bind(Utc::now().timestamp_millis()).bind(preview_id.to_string()).execute(self.repo.pool()).await?;
            self.ensure_access()?;
        }
        result
    }
    async fn execute_inner(
        &self,
        preview_id: Uuid,
        cancel: Option<&CancellationToken>,
        context: Option<&crate::scraper::automatic::AutomaticContext>,
    ) -> Result<OutputResult, RepositoryError> {
        let _gate = self.gate.lock().await;
        let row = sqlx::query("SELECT o.state,p.snapshot FROM file_operations o JOIN output_previews p ON p.id=o.id WHERE o.id=?").bind(preview_id.to_string()).fetch_optional(self.repo.pool()).await?.ok_or(RepositoryError::NotFound)?;
        let state: String = row.try_get("state")?;
        let snapshot: String = row.try_get("snapshot")?;
        let plan: OutputPlan =
            serde_json::from_str(&snapshot).map_err(|_| RepositoryError::CorruptData)?;
        self.ensure_access()?;
        if plan.channel != self.channel {
            return Err(RepositoryError::InvalidInput);
        }
        if self.channel == OutputChannel::Adult {
            let revoked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM adult_revoked_output_previews WHERE preview_id=?)",
            )
            .bind(preview_id.to_string())
            .fetch_one(self.repo.pool())
            .await?;
            if revoked {
                return Err(RepositoryError::Conflict);
            }
        }
        self.ensure_channel_media(plan.preview.media_item_id)
            .await?;
        let root = self
            .repo
            .get_root(plan.root_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        if !root.enabled
            || !self.root_matches_channel(root.kind)
            || Path::new(&root.path) != plan.root
        {
            return Err(RepositoryError::Conflict);
        }
        if !self
            .output_files(plan.preview.media_item_id)
            .await?
            .iter()
            .any(|file| {
                file.id == plan.source_file_id
                    && file.root_id == plan.root_id
                    && file.relative_path == plan.source_relative
            })
        {
            return Err(RepositoryError::Conflict);
        }
        #[cfg(windows)]
        let _windows_lease = if state == "succeeded" {
            None
        } else {
            Some(check_windows_lease(preview_id).await?)
        };
        if state == "succeeded" {
            // Replaying an already completed plan is a current-state check:
            // a disappeared source is a stale plan, not invalid caller input.
            verify_source(&plan).map_err(|_| RepositoryError::Conflict)?;
            verify_completed_plan(&plan)?;
            return Ok(OutputResult {
                schema_version: 1,
                preview_id,
                written: 0,
                preserved: plan.preview.files.len(),
                already_applied: true,
                state,
            });
        }
        verify_source(&plan)?;
        if state == "running" {
            return Err(RepositoryError::Conflict);
        }
        let acquired = sqlx::query("UPDATE file_operations SET state='running',updated_at=MAX(updated_at,?) WHERE id=? AND state IN ('planned','failed','interrupted')")
            .bind(Utc::now().timestamp_millis()).bind(preview_id.to_string()).execute(self.repo.pool()).await?;
        if acquired.rows_affected() != 1 {
            return Err(RepositoryError::Conflict);
        }
        let result = async {
            // Keep the writer reservation until synchronous publication ends:
            // root edits, reclassification and task cancellation cannot slip
            // between the automatic workflow guard and atomic create-new.
            let mut tx = self.repo.pool().begin().await?;
            sqlx::query("UPDATE library_roots SET updated_at=updated_at WHERE id=?")
                .bind(plan.root_id.to_string())
                .execute(&mut *tx)
                .await?;
            let current = sqlx::query("SELECT path,kind,enabled FROM library_roots WHERE id=?")
                .bind(plan.root_id.to_string())
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(RepositoryError::NotFound)?;
            let kind = current.try_get::<Option<String>, _>("kind")?;
            if !current.try_get::<bool, _>("enabled")?
                || Path::new(&current.try_get::<String, _>("path")?) != plan.root
                || (kind.as_deref() == Some("adult_video"))
                    != (self.channel == OutputChannel::Adult)
            {
                return Err(RepositoryError::Conflict);
            }
            let ancestor = crate::library_ready::ancestor_isolation("m");
            let lane = if self.channel == OutputChannel::Adult {
                "m.adult_isolated=1 AND m.kind='adult_video'".to_owned()
            } else {
                format!("m.adult_isolated=0 AND m.kind IN ('movie','tv_series','tv_season','tv_episode','anime') AND ({ancestor})")
            };
            let safe: bool = sqlx::query_scalar(&format!("SELECT EXISTS(SELECT 1 FROM media_items m WHERE m.id=? AND ({lane}))"))
                .bind(plan.preview.media_item_id.to_string()).fetch_one(&mut *tx).await?;
            if !safe { return Err(RepositoryError::InvalidInput); }
            if let Some(context) = context {
                context
                    .validate_tx(&mut tx, plan.preview.media_item_id)
                    .await?;
            }
            if plan.series_target {
                self.ensure_series_directory(
                    plan.preview.media_item_id,
                    plan.root_id,
                    &plan.relative_parent,
                    &mut tx,
                )
                .await?;
            }
            let result = execute_plan(&plan, self.adult_access.as_ref(), cancel);
            tx.commit().await?;
            result
        }
        .await;
        let state = if result.is_ok() {
            "succeeded"
        } else {
            "failed"
        };
        sqlx::query("UPDATE file_operations SET state=?,updated_at=MAX(updated_at,?) WHERE id=?")
            .bind(state)
            .bind(Utc::now().timestamp_millis())
            .bind(preview_id.to_string())
            .execute(self.repo.pool())
            .await?;
        let (written, preserved) = result?;
        Ok(OutputResult {
            schema_version: 1,
            preview_id,
            written,
            preserved,
            already_applied: false,
            state: state.into(),
        })
    }
}

fn verify_completed_plan(plan: &OutputPlan) -> Result<(), RepositoryError> {
    let dir = descend(
        open_absolute_dir(&plan.root, false).map_err(io_error)?,
        &plan.relative_parent,
        false,
    )
    .map_err(io_error)?;
    for diff in &plan.preview.files {
        let name = target_name(diff, &plan.relative_parent)?;
        let bytes =
            read_optional(&dir, name, MAX_IMAGE.max(MAX_NFO))?.ok_or(RepositoryError::Conflict)?;
        let expected = if diff.protected {
            diff.expected_hash
                .as_ref()
                .ok_or(RepositoryError::CorruptData)?
        } else {
            &diff.content_hash
        };
        if hash(&bytes) != *expected {
            return Err(RepositoryError::Conflict);
        }
    }
    Ok(())
}
fn execute_plan(
    plan: &OutputPlan,
    access: Option<&AdultOutputAccess>,
    cancel: Option<&CancellationToken>,
) -> Result<(usize, usize), RepositoryError> {
    let update_nfo = plan.preview.strategy == "update_nfo";
    if (plan.preview.strategy != "preserve" && !update_nfo)
        || (update_nfo && (plan.channel != OutputChannel::Ordinary || plan.nfo_only))
        || (update_nfo && plan.preview.files.iter().filter(|file| file.kind == "nfo").count() != 1)
        || plan.payloads.len() != plan.preview.files.len()
    {
        return Err(RepositoryError::CorruptData);
    }
    let dir = descend(
        open_absolute_dir(&plan.root, false).map_err(io_error)?,
        &plan.relative_parent,
        false,
    )
    .map_err(io_error)?;
    let mut written = 0;
    let mut preserved = 0;
    // Check all before-images first; explicit NFO retries may resume publication
    // of this same payload or a retained capture of the original bytes.
    for (diff, payload) in plan.preview.files.iter().zip(&plan.payloads) {
        if hash(payload) != diff.content_hash {
            return Err(RepositoryError::CorruptData);
        }
        let name = target_name(diff, &plan.relative_parent)?;
        if update_nfo && diff.kind == "nfo" {
            let source = checked_relative(&plan.source_relative)?;
            let stem = source.file_stem().and_then(|value| value.to_str()).ok_or(RepositoryError::CorruptData)?;
            let target = name.to_str().ok_or(RepositoryError::CorruptData)?;
            let safe = if plan.series_target {
                target.eq_ignore_ascii_case("tvshow.nfo")
            } else {
                target.eq_ignore_ascii_case(&format!("{stem}.nfo")) || target.eq_ignore_ascii_case("movie.nfo")
            };
            if !safe { return Err(RepositoryError::CorruptData); }
        }
        let actual = read_optional(&dir, &name, MAX_IMAGE.max(MAX_NFO))?;
        match (&diff.expected_hash, &actual) {
            (Some(expected), Some(bytes)) if hash(bytes) == *expected => {}
            (Some(_), Some(bytes)) if update_nfo && diff.kind == "nfo" && !diff.protected && hash(bytes) == diff.content_hash => {}
            (Some(expected), None) if update_nfo && diff.kind == "nfo" && !diff.protected
                && read_optional(&dir, nfo_backup_name(plan.preview.preview_id), MAX_NFO)?.is_some_and(|bytes| hash(&bytes) == *expected) => {}
            (None, None) => {}
            (None, Some(bytes)) if hash(bytes) == diff.content_hash => {} // completed pre-crash operation
            _ => return Err(RepositoryError::Conflict),
        }
    }
    // Publish referenced images before updating the NFO which points to them.
    let mut entries = plan.preview.files.iter().zip(&plan.payloads).collect::<Vec<_>>();
    entries.sort_by_key(|(diff, _)| diff.kind == "nfo");
    for (diff, payload) in entries {
        if let Some(access) = access {
            access.ensure()?;
        }
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
        }
        let name = target_name(diff, &plan.relative_parent)?;
        if diff.protected {
            preserved += 1;
            continue;
        }
        if update_nfo && diff.kind == "nfo" {
            if let Some(expected) = &diff.expected_hash {
                match atomic_replace_nfo(&dir, &name, payload, expected, plan.preview.preview_id, || {
                    verify_source(plan)?;
                    if cancel.is_some_and(CancellationToken::is_cancelled) {
                        return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
                    }
                    Ok(())
                })? {
                    true => written += 1,
                    false => preserved += 1,
                }
                continue;
            }
        }
        match atomic_create_checked(&dir, &name, payload, || {
            if let Some(access) = access {
                access.ensure()?;
            }
            if cancel.is_some_and(CancellationToken::is_cancelled) {
                return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
            }
            if plan.nfo_only {
                let desired = name.to_str().ok_or(RepositoryError::CorruptData)?;
                if existing_nfo_name(&dir, desired, !plan.series_target)?
                    .is_some_and(|existing| existing != desired)
                {
                    return Err(RepositoryError::Conflict);
                }
            }
            Ok(())
        }) {
            Ok(true) => written += 1,
            Ok(false) => preserved += 1,
            Err(error) => return Err(error),
        }
    }
    Ok((written, preserved))
}
fn target_name(diff: &OutputFileDiff, parent: &Path) -> Result<PathBuf, RepositoryError> {
    let relative = checked_relative(&diff.destination)?;
    if relative.parent().unwrap_or_else(|| Path::new("")) != parent {
        return Err(RepositoryError::CorruptData);
    }
    let name = relative.file_name().ok_or(RepositoryError::CorruptData)?;
    if diff.kind == "nfo" && relative.extension().and_then(|value| value.to_str()).is_none_or(|value| !value.eq_ignore_ascii_case("nfo")) {
        return Err(RepositoryError::CorruptData);
    }
    if !matches!(
        diff.kind.as_str(),
        "nfo" | "poster" | "fanart" | "actor_thumb" | "cover" | "back_cover" | "sample"
    ) {
        return Err(RepositoryError::CorruptData);
    }
    Ok(PathBuf::from(name))
}
fn io_error(_: std::io::Error) -> RepositoryError {
    RepositoryError::InvalidInput
}
fn checked_relative(value: &str) -> Result<PathBuf, RepositoryError> {
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || value.contains('\\')
        || value.contains('\0')
    {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(path)
}
/// Open every directory component without following links. Once open, all I/O
/// is relative to directory capabilities, surviving ancestor rename/replacement.
fn open_absolute_dir(path: &Path, create: bool) -> std::io::Result<Dir> {
    if !path.is_absolute() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    let mut prefix = PathBuf::new();
    let mut normals = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(p) => prefix.push(p.as_os_str()),
            Component::RootDir => prefix.push(component.as_os_str()),
            Component::Normal(v) => normals.push(v),
            _ => return Err(std::io::ErrorKind::InvalidInput.into()),
        }
    }
    descend(
        Dir::open_ambient_dir(prefix, ambient_authority())?,
        &normals,
        create,
    )
}
fn descend(mut dir: Dir, relative: &Path, create: bool) -> std::io::Result<Dir> {
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(std::io::ErrorKind::InvalidInput.into());
        };
        if create {
            match dir.create_dir(name) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        dir = dir.open_dir_nofollow(name)?;
    }
    Ok(dir)
}
fn read_optional(
    dir: &Dir,
    name: impl AsRef<Path>,
    max: usize,
) -> Result<Option<Vec<u8>>, RepositoryError> {
    let name = name.as_ref();
    let meta = match dir.symlink_metadata(name) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_error(e)),
    };
    if meta.file_type().is_symlink() || !meta.is_file() || meta.len() > max as u64 {
        return Err(RepositoryError::InvalidInput);
    }
    // Open with no-follow on the final component too; checks alone would race.
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut bytes = Vec::new();
    let file = dir.open_with(name, &options).map_err(io_error)?;
    if !file.metadata().map_err(io_error)?.is_file() {
        return Err(RepositoryError::InvalidInput);
    }
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > max {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(Some(bytes))
}
#[cfg(test)]
fn atomic_create(dir: &Dir, name: impl AsRef<Path>, bytes: &[u8]) -> Result<bool, RepositoryError> {
    atomic_create_checked(dir, name, bytes, || Ok(()))
}
fn nfo_backup_name(preview_id: Uuid) -> String {
    format!(".mediaatlas-nfo-{preview_id}.backup")
}
fn atomic_replace_nfo(
    dir: &Dir,
    name: &Path,
    bytes: &[u8],
    expected: &str,
    preview_id: Uuid,
    check: impl Fn() -> Result<(), RepositoryError>,
) -> Result<bool, RepositoryError> {
    if name.components().count() != 1 || name.extension().and_then(|value| value.to_str()).is_none_or(|value| !value.eq_ignore_ascii_case("nfo")) {
        return Err(RepositoryError::InvalidInput);
    }
    validate_nfo(std::str::from_utf8(bytes).map_err(|_| RepositoryError::InvalidInput)?)?;
    let temporary = format!(".mediaatlas-{}.tmp", Uuid::new_v4());
    let backup = nfo_backup_name(preview_id);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = dir.open_with(&temporary, &options).map_err(io_error)?;
    let mut captured = false;
    let result = (|| {
        file.write_all(bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        drop(file);
        check()?;
        let current = read_optional(dir, name, MAX_NFO)?;
        if current.as_deref() == Some(bytes) { return Ok(false); }
        let prior_backup = read_optional(dir, &backup, MAX_NFO)?;
        if let Some(prior) = prior_backup {
            // A captured before-image survives process interruption. Resume only
            // when the destination is still absent; never remove a new writer's file.
            if current.is_some() || hash(&prior) != expected { return Err(RepositoryError::Conflict); }
        } else {
            let current = current.ok_or(RepositoryError::Conflict)?;
            if hash(&current) != expected { return Err(RepositoryError::Conflict); }
            // Capture whichever inode currently occupies the name before testing
            // its bytes, then publish create-only so concurrent edits are retained.
            dir.rename(name, dir, &backup).map_err(io_error)?;
            captured = true;
            let actual = read_optional(dir, &backup, MAX_NFO)?.ok_or(RepositoryError::Conflict)?;
            if hash(&actual) != expected { return Err(RepositoryError::Conflict); }
        }
        check()?;
        match dir.hard_link(&temporary, dir, name) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let actual = read_optional(dir, name, MAX_NFO)?.ok_or(RepositoryError::Conflict)?;
                if actual == bytes { Ok(false) } else { Err(RepositoryError::Conflict) }
            }
            Err(error) => Err(io_error(error)),
        }
    })();
    if result.is_err() && captured {
        // Create-only restoration cannot clobber a file created during export.
        let _ = dir.hard_link(&backup, dir, name);
    }
    let _ = dir.remove_file(&temporary);
    result
}
fn atomic_create_checked(
    dir: &Dir,
    name: impl AsRef<Path>,
    bytes: &[u8],
    check: impl FnOnce() -> Result<(), RepositoryError>,
) -> Result<bool, RepositoryError> {
    let name = name.as_ref();
    let temporary = format!(".mediaatlas-{}.tmp", Uuid::new_v4());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = dir.open_with(&temporary, &options).map_err(io_error)?;
    let result = (|| {
        file.write_all(bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        // Recheck privacy/cancellation after staging and immediately before publication.
        check()?;
        // Hard-link publication is atomic and cannot clobber an intervening file.
        match dir.hard_link(&temporary, dir, name) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing =
                    read_optional(dir, name, bytes.len())?.ok_or(RepositoryError::Conflict)?;
                if existing == bytes {
                    Ok(false)
                } else {
                    Err(RepositoryError::Conflict)
                }
            }
            Err(e) => Err(io_error(e)),
        }
    })();
    let _ = dir.remove_file(&temporary);
    result
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn validate_nfo(value: &str) -> Result<(), RepositoryError> {
    if value.len() > MAX_NFO
        || value.to_ascii_uppercase().contains("<!DOCTYPE")
        || value.to_ascii_uppercase().contains("<!ENTITY")
    {
        return Err(RepositoryError::InvalidInput);
    }
    let document = roxmltree::Document::parse(value).map_err(|_| RepositoryError::InvalidInput)?;
    if !matches!(
        document.root_element().tag_name().name(),
        "movie" | "tvshow" | "episodedetails"
    ) {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(())
}
fn merge_kodi_export(
    existing: &str,
    generated: &str,
    clear_missing: bool,
) -> Result<String, RepositoryError> {
    validate_nfo(existing)?;
    validate_nfo(generated)?;
    let source = roxmltree::Document::parse(generated).map_err(|_| RepositoryError::InvalidInput)?;
    let old = roxmltree::Document::parse(existing).map_err(|_| RepositoryError::InvalidInput)?;
    if source.root_element().tag_name().name() != old.root_element().tag_name().name() {
        return Err(RepositoryError::InvalidInput);
    }
    let compound = ["actor", "genre", "studio", "country", "director", "credits", "ratings", "set", "thumb", "fanart", "uniqueid", "episodeguide", "tag", "showlink"];
    let fields = source.root_element().children().filter(roxmltree::Node::is_element)
        .filter(|node| !compound.contains(&node.tag_name().name()))
        .filter_map(|node| node.text().map(|text| (node.tag_name().name().to_owned(), text.to_owned())))
        .collect();
    let mut merged = merge_nfo_known_fields(existing, &fields, &[])?;
    let document = roxmltree::Document::parse(&merged).map_err(|_| RepositoryError::InvalidInput)?;
    let root = document.root_element();
    let mut replacements = Vec::new();
    let mut additions = String::new();
    for tag in compound {
        let replacement = source.root_element().children()
            .filter(|node| node.is_element() && node.tag_name().name() == tag)
            .map(|node| &generated[node.range()]).collect::<Vec<_>>().join("\n  ");
        if replacement.is_empty() && !clear_missing && !matches!(tag, "thumb" | "fanart" | "actor") {
            continue;
        }
        let nodes = root.children().filter(|node| node.is_element() && node.tag_name().name() == tag).collect::<Vec<_>>();
        if nodes.is_empty() {
            if !replacement.is_empty() { additions.push_str(&format!("\n  {replacement}")); }
        } else {
            for (index, node) in nodes.iter().enumerate() {
                replacements.push((node.range(), if index == 0 { replacement.clone() } else { String::new() }));
            }
        }
    }
    if clear_missing {
        for tag in ["originaltitle", "plot", "outline", "sorttitle", "showtitle", "runtime", "year", "premiered", "aired", "tagline", "mpaa", "status", "sourceattribution", "sourcelicense", "rating", "votes"] {
            if !source.root_element().children().any(|node| node.is_element() && node.tag_name().name() == tag) {
                for node in root.children().filter(|node| node.is_element() && node.tag_name().name() == tag) {
                    replacements.push((node.range(), String::new()));
                }
            }
        }
    }
    if !additions.is_empty() {
        let closing = root.range().start + merged[root.range()].rfind("</").ok_or(RepositoryError::InvalidInput)?;
        replacements.push((closing..closing, additions));
    }
    replacements.sort_by(|left, right| right.0.start.cmp(&left.0.start));
    drop(document);
    for (range, value) in replacements { merged.replace_range(range, &value); }
    validate_nfo(&merged)?;
    Ok(merged)
}

/// Merge a small allowlist of scalar NFO fields while preserving every byte
/// outside changed known elements. Protected existing fields remain authoritative.
/// Unknown elements, attributes, comments, namespaces and ordering survive.
pub fn merge_nfo_known_fields(
    existing: &str,
    fields: &BTreeMap<String, String>,
    protected_fields: &[String],
) -> Result<String, RepositoryError> {
    validate_nfo(existing)?;
    let document =
        roxmltree::Document::parse(existing).map_err(|_| RepositoryError::InvalidInput)?;
    let root = document.root_element();
    let allowed = [
        "title",
        "originaltitle",
        "plot",
        "outline",
        "year",
        "runtime",
        "season",
        "episode",
        "premiered",
        "aired",
        "sourceattribution",
        "sourcelicense",
        "catalognumber",
        "catalognamespace",
        "studio",
        "label",
        "set",
        "releasedate",
        "tagline",
        "sorttitle",
        "mpaa",
        "status",
        "userrating",
        "rating",
        "votes",
    ];
    let mut replacements = Vec::new();
    let mut additions = String::new();
    for (key, value) in fields {
        if !allowed.contains(&key.as_str()) || value.trim().is_empty() {
            continue;
        }
        let nodes: Vec<_> = root
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == key)
            .collect();
        if nodes.is_empty() {
            additions.push_str(&format!("\n  <{key}>{}</{key}>", xml_escape(value)));
        } else if !protected_fields.contains(key) {
            for node in nodes {
                let range = node.range();
                let raw = &existing[range.clone()];
                // Preserve even unknown attributes on a known scalar element.
                let mut quoted = None;
                let mut end = None;
                for (index, character) in raw.char_indices() {
                    match (quoted, character) {
                        (None, '\'' | '"') => quoted = Some(character),
                        (Some(quote), c) if c == quote => quoted = None,
                        (None, '>') => {
                            end = Some(index);
                            break;
                        }
                        _ => {}
                    }
                }
                let end = end.ok_or(RepositoryError::InvalidInput)?;
                let opening = raw[..end].trim_end_matches('/');
                let new = format!("{opening}>{}</{key}>", xml_escape(value));
                replacements.push((range, new));
            }
        }
    }
    if !additions.is_empty() {
        let raw = &existing[root.range()];
        if let Some(closing) = raw.rfind("</") {
            let closing = closing + root.range().start;
            replacements.push((closing..closing, additions));
        } else {
            let close = raw.rfind("/>").ok_or(RepositoryError::InvalidInput)?;
            replacements.push((
                root.range().start + close..root.range().end,
                format!(">{additions}</{}>", root.tag_name().name()),
            ));
        }
    }
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut result = existing.to_owned();
    for (range, value) in replacements {
        result.replace_range(range, &value);
    }
    validate_nfo(&result)?;
    Ok(result)
}

#[cfg(test)]
fn render_nfo(
    item: &MediaItem,
    values: &[MetadataValue],
    ids: &[mediaatlas_domain::ExternalId],
) -> Result<String, RepositoryError> {
    crate::kodi_nfo::render_nfo(item, values, ids, &[], None)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedArtwork {
    pub cache_path: String,
    pub thumbnail_path: String,
    pub content_hash: String,
    pub width: u32,
    pub height: u32,
    pub mime: String,
}
fn extension(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Png => Some("png"),
        ImageFormat::Jpeg => Some("jpg"),
        ImageFormat::WebP => Some("webp"),
        _ => None,
    }
}
fn mime(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Png => Some("image/png"),
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::WebP => Some("image/webp"),
        _ => None,
    }
}
fn decode_validated(bytes: &[u8]) -> Result<(ImageFormat, image::DynamicImage), ProviderFailure> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE {
        return Err(ProviderFailure::ResponseTooLarge);
    }
    let format = image::guess_format(bytes).map_err(|_| ProviderFailure::ParseError)?;
    if extension(format).is_none() {
        return Err(ProviderFailure::ParseError);
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(80 * 1024 * 1024);
    reader.limits(limits.clone());
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| ProviderFailure::ParseError)?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(ProviderFailure::ResponseTooLarge);
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| ProviderFailure::ParseError)?;
    Ok((format, image))
}
fn decode_budget() -> Arc<Semaphore> {
    static BUDGET: OnceLock<Arc<Semaphore>> = OnceLock::new();
    BUDGET.get_or_init(|| Arc::new(Semaphore::new(2))).clone()
}
fn decode_image(bytes: &[u8]) -> Result<(ImageFormat, u32, u32), ProviderFailure> {
    let _permit = decode_budget()
        .try_acquire_owned()
        .map_err(|_| ProviderFailure::ResponseTooLarge)?;
    let (format, image) = decode_validated(bytes)?;
    Ok((format, image.width(), image.height()))
}
struct DecodedArtwork {
    bytes: Vec<u8>,
    format: ImageFormat,
    width: u32,
    height: u32,
    thumbnail: Vec<u8>,
}
async fn decode_thumbnail(
    bytes: Vec<u8>,
    cancel: &CancellationToken,
) -> Result<DecodedArtwork, ProviderFailure> {
    let permit = tokio::select! {biased;_=cancel.cancelled()=>return Err(ProviderFailure::Cancelled),permit=decode_budget().acquire_owned()=>permit.map_err(|_|ProviderFailure::Cancelled)?};
    let task = tokio::task::spawn_blocking(move || {
        // Permit lives in the blocking task, including when its caller cancels.
        let _permit = permit;
        let (format, image) = decode_validated(&bytes)?;
        let (width, height) = (image.width(), image.height());
        let mut thumbnail = Cursor::new(Vec::new());
        image
            .thumbnail(240, 360)
            .write_to(&mut thumbnail, ImageFormat::Png)
            .map_err(|_| ProviderFailure::ParseError)?;
        Ok(DecodedArtwork {
            bytes,
            format,
            width,
            height,
            thumbnail: thumbnail.into_inner(),
        })
    });
    tokio::select! {biased;_=cancel.cancelled()=>Err(ProviderFailure::Cancelled),result=task=>result.map_err(|_|ProviderFailure::ParseError)?}
}
impl MediaOutputService {
    /// Validate a persisted selection without performing a Provider request.
    pub(crate) fn validated_cached_thumbnail(
        &self,
        cached: &CachedArtwork,
    ) -> Result<Vec<u8>, RepositoryError> {
        self.ensure_access()?;
        if cached.content_hash.len() != 64
            || !cached
                .content_hash
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(RepositoryError::InvalidInput);
        }
        let path = Path::new(&cached.cache_path);
        if path.parent() != Some(self.base_cache.as_path()) {
            return Err(RepositoryError::InvalidInput);
        }
        let expected_extension = match cached.mime.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            _ => return Err(RepositoryError::InvalidInput),
        };
        let name = format!("{}.{}", cached.content_hash, expected_extension);
        if path.file_name().and_then(|v| v.to_str()) != Some(name.as_str()) {
            return Err(RepositoryError::InvalidInput);
        }
        let cache = open_absolute_dir(&self.base_cache, false).map_err(io_error)?;
        let bytes = read_optional(&cache, &name, MAX_IMAGE)?.ok_or(RepositoryError::NotFound)?;
        if hash(&bytes) != cached.content_hash {
            return Err(RepositoryError::Conflict);
        }
        let format = image::guess_format(&bytes).map_err(|_| RepositoryError::InvalidInput)?;
        if mime(format) != Some(cached.mime.as_str()) {
            return Err(RepositoryError::InvalidInput);
        }
        self.thumbnail_bytes(cached)
    }
    pub fn thumbnail_bytes(&self, cached: &CachedArtwork) -> Result<Vec<u8>, RepositoryError> {
        self.ensure_access()?;
        let path = Path::new(&cached.thumbnail_path);
        if path.parent() != Some(self.base_cache.as_path())
            || path.file_name().and_then(|v| v.to_str())
                != Some(format!("{}.thumb.png", cached.content_hash).as_str())
        {
            return Err(RepositoryError::InvalidInput);
        }
        let cache = open_absolute_dir(&self.base_cache, false).map_err(io_error)?;
        let bytes = read_optional(
            &cache,
            path.file_name().ok_or(RepositoryError::InvalidInput)?,
            1024 * 1024,
        )?
        .ok_or(RepositoryError::NotFound)?;
        if image::guess_format(&bytes).map_err(|_| RepositoryError::InvalidInput)?
            != ImageFormat::Png
        {
            return Err(RepositoryError::InvalidInput);
        }
        // Reject maliciously enlarged cached thumbnails before allocating pixels.
        let mut limits = Limits::default();
        limits.max_image_width = Some(240);
        limits.max_image_height = Some(360);
        limits.max_alloc = Some(1024 * 1024);
        let mut reader = ImageReader::with_format(Cursor::new(&bytes), ImageFormat::Png);
        reader.limits(limits.clone());
        let (width, height) = reader
            .into_dimensions()
            .map_err(|_| RepositoryError::InvalidInput)?;
        if width == 0 || height == 0 || width > 240 || height > 360 {
            return Err(RepositoryError::InvalidInput);
        }
        let _permit = decode_budget()
            .try_acquire_owned()
            .map_err(|_| RepositoryError::Conflict)?;
        let mut reader = ImageReader::with_format(Cursor::new(&bytes), ImageFormat::Png);
        reader.limits(limits);
        reader.decode().map_err(|_| RepositoryError::InvalidInput)?;
        self.ensure_access()?;
        Ok(bytes)
    }
    pub async fn cache_artwork(
        &self,
        engine: &NetworkEngine,
        job: &ProviderJob,
        url: &str,
        cancel: &CancellationToken,
    ) -> Result<CachedArtwork, ProviderFailure> {
        self.ensure_access()
            .map_err(|_| ProviderFailure::Cancelled)?;
        if (self.channel == OutputChannel::Adult && job.media_kind != Some(MediaKind::AdultVideo))
            || (self.channel == OutputChannel::Ordinary
                && job.media_kind == Some(MediaKind::AdultVideo))
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        if cancel.is_cancelled() {
            return Err(ProviderFailure::Cancelled);
        }
        let _gate = self.gate.lock().await;
        if cancel.is_cancelled() {
            return Err(ProviderFailure::Cancelled);
        }
        let cache = open_absolute_dir(&self.base_cache, true)
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        let mapping = format!("url-{}.json", hash(url.as_bytes()));
        if let Ok(Some(bytes)) = read_optional(&cache, &mapping, 4096) {
            if let Ok(cached) = serde_json::from_slice::<CachedArtwork>(&bytes) {
                let path = Path::new(&cached.cache_path);
                if let Some(name) = path.file_name() {
                    if path.parent() == Some(self.base_cache.as_path()) {
                        if let Ok(Some(bytes)) = read_optional(&cache, name, MAX_IMAGE) {
                            if hash(&bytes) == cached.content_hash
                                && self.thumbnail_bytes(&cached).is_ok()
                            {
                                return Ok(cached);
                            }
                        }
                    }
                }
            }
        }
        let mut bounded = job.clone();
        bounded.config.max_response_bytes = bounded.config.max_response_bytes.min(MAX_IMAGE);
        let response = engine
            .execute(&bounded, RequestSpec::get(url, EngineKind::Image), cancel)
            .await?;
        let decoded = decode_thumbnail(response.body, cancel).await?;
        self.ensure_access()
            .map_err(|_| ProviderFailure::Cancelled)?;
        let (format, width, height) = (decoded.format, decoded.width, decoded.height);
        let actual_mime = response
            .content_type
            .as_deref()
            .and_then(|s| s.split(';').next())
            .map(str::trim);
        if !actual_mime
            .zip(mime(format))
            .is_some_and(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
        {
            return Err(ProviderFailure::ParseError);
        }
        let content_hash = hash(&decoded.bytes);
        let file_name = format!(
            "{content_hash}.{}",
            extension(format).ok_or(ProviderFailure::ParseError)?
        );
        atomic_create_checked(&cache, &file_name, &decoded.bytes, || self.ensure_access())
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        let thumbnail_name = format!("{content_hash}.thumb.png");
        atomic_create_checked(&cache, &thumbnail_name, &decoded.thumbnail, || {
            self.ensure_access()
        })
        .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        let cached = CachedArtwork {
            cache_path: self
                .base_cache
                .join(&file_name)
                .to_string_lossy()
                .into_owned(),
            thumbnail_path: self
                .base_cache
                .join(&thumbnail_name)
                .to_string_lossy()
                .into_owned(),
            content_hash,
            width,
            height,
            mime: mime(format).ok_or(ProviderFailure::ParseError)?.into(),
        };
        // URL aliases are only create-new. A changed remote URL never overwrites
        // an existing cache snapshot or weakens content-hash validation.
        atomic_create_checked(
            &cache,
            &mapping,
            &serde_json::to_vec(&cached).map_err(|_| ProviderFailure::ParseError)?,
            || self.ensure_access(),
        )
        .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        self.ensure_access()
            .map_err(|_| ProviderFailure::Cancelled)?;
        Ok(cached)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mediaatlas_domain::{LibraryRoot, MediaFile, PathStyle, ProviderConfig};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn library() -> (
        tempfile::TempDir,
        LibraryRepository,
        MediaOutputService,
        Uuid,
    ) {
        let temporary = crate::test_support::tempdir().unwrap();
        let media_dir = temporary.path().join("media");
        std::fs::create_dir(&media_dir).unwrap();
        std::fs::write(media_dir.join("Example.mkv"), b"untouched video bytes").unwrap();
        let repo = LibraryRepository::open(&temporary.path().join("test.sqlite"))
            .await
            .unwrap();
        let now = Utc::now();
        let root_id = Uuid::new_v4();
        let media_id = Uuid::new_v4();
        repo.save_root(&LibraryRoot {
            id: root_id,
            name: "test".into(),
            path: media_dir.to_string_lossy().into_owned(),
            path_style: if cfg!(windows) {
                PathStyle::Windows
            } else {
                PathStyle::Posix
            },
            kind: Some(MediaKind::Movie),
            enabled: true,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
        repo.save_media(&MediaItem {
            id: media_id,
            kind: MediaKind::Movie,
            title: "A & B <fixture>".into(),
            original_title: None,
            year: Some(2024),
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
        repo.save_file(&MediaFile {
            id: Uuid::new_v4(),
            media_item_id: media_id,
            root_id,
            relative_path: "Example.mkv".into(),
            size_bytes: 21,
            modified_at: None,
            part: None,
        })
        .await
        .unwrap();
        let service = MediaOutputService::new(repo.clone(), temporary.path().join("cache"));
        (temporary, repo, service, media_id)
    }
    fn kodi_value(id: Uuid, key: &str, value: serde_json::Value) -> MetadataValue {
        MetadataValue {
            id: Uuid::new_v4(), media_item_id: id, field_key: key.into(), locale: None,
            value, provider_id: Some("tmdb".into()), source_url: None, fetched_at: None,
            confidence: None, is_user_modified: false, is_locked: false,
        }
    }
    async fn save_kodi_value(repo: &LibraryRepository, value: MetadataValue) {
        repo.save_metadata_value(&value, crate::library::MetadataWriteMode::User).await.unwrap();
    }

    #[tokio::test]
    async fn online_kodi_export_writes_complete_metadata_and_https_artwork_without_images() {
        let (temp, repo, service, id) = library().await;
        save_kodi_value(&repo, kodi_value(id, "cast", serde_json::json!([
            {"name":"Actor & Name", "role":"A <hero>", "order":0, "thumb":"https://image.tmdb.org/t/p/w185/actor.jpg"}
        ]))).await;
        save_kodi_value(&repo, kodi_value(id, "nfo_artwork", serde_json::json!([
            {"kind":"poster","url":"https://image.tmdb.org/t/p/original/poster.jpg"},
            {"kind":"backdrop","url":"https://image.tmdb.org/t/p/original/fanart.jpg"}
        ]))).await;
        let preview = service.preview_with_mode(id, NfoExportMode::Online).await.unwrap();
        assert_eq!(preview.files.len(), 1);
        let xml = preview.files[0].new_text.as_ref().unwrap();
        assert!(xml.contains("<role>A &lt;hero&gt;</role>"));
        assert!(xml.contains("<thumb aspect=\"poster\">https://image.tmdb.org/t/p/original/poster.jpg</thumb>"));
        assert!(xml.contains("<fanart><thumb>https://image.tmdb.org/t/p/original/fanart.jpg</thumb></fanart>"));
        assert!(!temp.path().join("media/Example.nfo").exists());
        let result = service.execute(preview.preview_id).await.unwrap();
        assert_eq!(result.written, 1);
        assert!(!temp.path().join("media/Example-poster.jpg").exists());
        assert_eq!(std::fs::read(temp.path().join("media/Example.mkv")).unwrap(), b"untouched video bytes");
        repo.pool().close().await;
    }

    #[tokio::test]
    async fn offline_kodi_export_writes_real_image_formats_and_only_local_references() {
        let (temp, repo, service, id) = library().await;
        let image = png();
        for name in ["poster.png", "fanart.png", "actor.png"] {
            std::fs::write(temp.path().join("media").join(name), &image).unwrap();
        }
        save_kodi_value(&repo, kodi_value(id, "nfo_artwork", serde_json::json!([
            {"kind":"poster","url":"poster.png"}, {"kind":"backdrop","url":"fanart.png"}
        ]))).await;
        save_kodi_value(&repo, kodi_value(id, "cast", serde_json::json!([
            {"name":"Actor", "role":"Lead", "thumb":"actor.png"}
        ]))).await;
        let preview = service.preview_with_mode(id, NfoExportMode::Offline).await.unwrap();
        assert_eq!(preview.files.len(), 4);
        assert!(preview.files.iter().any(|file| file.kind == "poster"));
        assert!(preview.files.iter().any(|file| file.kind == "fanart"));
        assert!(preview.files.iter().any(|file| file.kind == "actor_thumb"));
        let nfo = preview.files.iter().find(|file| file.kind == "nfo").unwrap().new_text.as_ref().unwrap();
        assert!(nfo.contains("<thumb aspect=\"poster\">Example-poster.png</thumb>"));
        assert!(nfo.contains("<fanart><thumb>Example-fanart.png</thumb></fanart>"));
        assert!(!nfo.contains("https://"));
        assert!(!temp.path().join("media/Example-poster.png").exists());
        assert_eq!(service.execute(preview.preview_id).await.unwrap().written, 4);
        for file in preview.files.iter().filter(|file| file.kind != "nfo") {
            assert_eq!(std::fs::read(temp.path().join("media").join(&file.destination)).unwrap(), image);
        }
        assert_eq!(std::fs::read(temp.path().join("media/Example.mkv")).unwrap(), b"untouched video bytes");
        repo.pool().close().await;
    }

    #[tokio::test]
    async fn explicit_kodi_export_updates_existing_nfo_but_keeps_locks_and_unknown_xml() {
        let (temp, repo, service, id) = library().await;
        let path = temp.path().join("media/Example.nfo");
        let old = "<movie custom=\"keep\"><title>Hand title</title><plot>Old plot</plot><!-- keep --><extension value=\"local\"/></movie>";
        std::fs::write(&path, old).unwrap();
        let mut title = kodi_value(id, "title", serde_json::json!("Hand title"));
        title.is_locked = true;
        title.is_user_modified = true;
        save_kodi_value(&repo, title).await;
        save_kodi_value(&repo, kodi_value(id, "overview", serde_json::json!("Updated plot"))).await;
        save_kodi_value(&repo, kodi_value(id, "genres", serde_json::json!(["Drama"]))).await;
        save_kodi_value(&repo, kodi_value(id, "cast", serde_json::json!([{"name":"Actor", "role":"Lead"}]))).await;
        let preview = service.preview_with_mode(id, NfoExportMode::Online).await.unwrap();
        assert_eq!(preview.strategy, "update_nfo");
        assert_eq!(preview.files.len(), 1);
        assert!(!preview.files[0].protected);
        let xml = preview.files[0].new_text.as_ref().unwrap();
        for expected in ["custom=\"keep\"", "<title>Hand title</title>", "<plot>Updated plot</plot>", "<!-- keep -->", "<extension value=\"local\"/>", "<genre>Drama</genre>", "<role>Lead</role>"] {
            assert!(xml.contains(expected), "{expected}");
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), old);
        assert_eq!(service.execute(preview.preview_id).await.unwrap().written, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), *xml);
        assert!(service.execute(preview.preview_id).await.unwrap().already_applied);
        assert_eq!(std::fs::read(temp.path().join("media/Example.mkv")).unwrap(), b"untouched video bytes");
        repo.pool().close().await;
    }

    #[tokio::test]
    async fn changed_nfo_after_export_preview_is_not_overwritten() {
        let (temp, repo, service, id) = library().await;
        let path = temp.path().join("media/Example.nfo");
        std::fs::write(&path, "<movie><title>Original</title></movie>").unwrap();
        save_kodi_value(&repo, kodi_value(id, "overview", serde_json::json!("New plot"))).await;
        let preview = service.preview_with_mode(id, NfoExportMode::Online).await.unwrap();
        let changed = "<movie><title>Concurrent manual edit</title></movie>";
        std::fs::write(&path, changed).unwrap();
        assert_eq!(service.execute(preview.preview_id).await.unwrap_err(), RepositoryError::Conflict);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), changed);
        repo.pool().close().await;
    }

    #[tokio::test]
    async fn manual_rescrape_does_not_reimport_old_runtime_or_keep_cleared_remote_cast() {
        let (temp, repo, service, id) = library().await;
        let old = "<movie><title>Old title</title><runtime>95</runtime><uniqueid type=\"tmdb\">1</uniqueid><actor><name>Old actor</name><thumb>https://image.tmdb.org/t/p/w185/old.jpg</thumb></actor></movie >";
        std::fs::write(temp.path().join("media/Example.nfo"), old).unwrap();
        repo.save_external_id(&mediaatlas_domain::ExternalId {
            id: Uuid::new_v4(), media_item_id: id, provider_id: "tmdb".into(),
            namespace: "tmdb".into(), external_id: "2".into(),
            is_primary: true, created_at: Utc::now(),
        }).await.unwrap();
        save_kodi_value(&repo, kodi_value(id, "runtime_minutes", serde_json::json!(100))).await;
        save_kodi_value(&repo, kodi_value(id, "cast", serde_json::json!([]))).await;
        let preview = service.preview_with_mode(id, NfoExportMode::Offline).await.unwrap();
        let xml = preview.files[0].new_text.as_ref().unwrap();
        assert!(xml.contains("<runtime>100</runtime>"));
        assert!(!xml.contains("<actor>"));
        assert!(!xml.contains("https://"));
        assert!(xml.contains("<uniqueid type=\"tmdb\" default=\"true\">2</uniqueid>"));
        assert!(!xml.contains(">1</uniqueid>"));
        assert!(roxmltree::Document::parse(xml).is_ok());
        repo.pool().close().await;
    }

    #[test]
    fn identity_correction_does_not_retain_the_wrong_movies_metadata_in_nfo() {
        let old = "<movie><title>Wrong</title><plot>Wrong plot</plot><genre>Wrong genre</genre><runtime>99</runtime><uniqueid type=\"imdb\">tt1</uniqueid><playcount>2</playcount><extension>keep</extension></movie>";
        let generated = "<movie><title>Correct</title><uniqueid type=\"imdb\" default=\"true\">tt2</uniqueid></movie>";
        let merged = merge_kodi_export(old, generated, true).unwrap();
        assert!(merged.contains("<title>Correct</title>"));
        assert!(merged.contains(">tt2</uniqueid>"));
        assert!(merged.contains("<playcount>2</playcount>"));
        assert!(merged.contains("<extension>keep</extension>"));
        for obsolete in ["Wrong plot", "Wrong genre", "<runtime>", ">tt1</uniqueid>"] {
            assert!(!merged.contains(obsolete), "{obsolete}");
        }
    }

    #[test]
    fn nfo_publication_never_clobbers_a_file_created_after_the_before_image_was_captured() {
        let temp = crate::test_support::tempdir().unwrap();
        let dir = open_absolute_dir(temp.path(), false).unwrap();
        let old = b"<movie><title>Old</title></movie>";
        let new = b"<movie><title>Exported</title></movie>";
        let external = b"<movie><title>Concurrent editor</title></movie>";
        std::fs::write(temp.path().join("movie.nfo"), old).unwrap();
        let calls = std::cell::Cell::new(0);
        let preview = Uuid::new_v4();
        let result = atomic_replace_nfo(&dir, Path::new("movie.nfo"), new, &hash(old), preview, || {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                std::fs::write(temp.path().join("movie.nfo"), external).unwrap();
            }
            Ok(())
        });
        assert_eq!(result.unwrap_err(), RepositoryError::Conflict);
        assert_eq!(std::fs::read(temp.path().join("movie.nfo")).unwrap(), external);
        assert_eq!(std::fs::read(temp.path().join(nfo_backup_name(preview))).unwrap(), old);
    }

    #[tokio::test]
    async fn interrupted_nfo_capture_is_resumed_only_by_explicit_export_execution() {
        let (temp, repo, service, id) = library().await;
        let path = temp.path().join("media/Example.nfo");
        let old = "<movie><title>Existing title</title></movie>";
        std::fs::write(&path, old).unwrap();
        save_kodi_value(&repo, kodi_value(id, "overview", serde_json::json!("New plot"))).await;
        let preview = service.preview_with_mode(id, NfoExportMode::Online).await.unwrap();
        let backup = temp.path().join("media").join(nfo_backup_name(preview.preview_id));
        std::fs::rename(&path, &backup).unwrap();
        sqlx::query("UPDATE file_operations SET state='interrupted' WHERE id=?")
            .bind(preview.preview_id.to_string()).execute(repo.pool()).await.unwrap();
        let source = repo.list_files(id).await.unwrap().remove(0);
        assert_eq!(service.preview_inner(id, true, Some(source.id), NfoExportMode::Online, false).await.unwrap_err(), RepositoryError::Conflict);
        assert!(!path.exists());
        service.execute(preview.preview_id).await.unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("<plot>New plot</plot>"));
        assert_eq!(std::fs::read_to_string(backup).unwrap(), old);
        assert!(service.execute(preview.preview_id).await.unwrap().already_applied);
        repo.pool().close().await;
    }

    #[tokio::test]
    async fn nested_chinese_movie_exports_keep_same_named_files_in_their_own_directories() {
        let (temp, repo, service, first) = library().await;
        let original = repo.list_files(first).await.unwrap().remove(0);
        let mut ids = Vec::new();
        for (index, directory) in ["无敌破坏王/无敌破坏1", "无敌破坏王/无敌破坏2"].iter().enumerate() {
            let id = if index == 0 { first } else { Uuid::new_v4() };
            let mut item = repo.get_media(first).await.unwrap().unwrap();
            item.id = id;
            item.title = format!("无敌破坏{}", index + 1);
            repo.save_media(&item).await.unwrap();
            let relative = format!("{directory}/正片.mp4");
            let parent = temp.path().join("media").join(directory);
            std::fs::create_dir_all(&parent).unwrap();
            std::fs::write(parent.join("正片.mp4"), b"unchanged nested movie").unwrap();
            repo.save_file(&MediaFile { id: if index == 0 { original.id } else { Uuid::new_v4() }, media_item_id: id, root_id: original.root_id, relative_path: relative, size_bytes: 22, modified_at: None, part: None }).await.unwrap();
            let preview = service.preview_with_mode(id, NfoExportMode::Online).await.unwrap();
            assert_eq!(preview.files[0].destination, format!("{directory}/正片.nfo"));
            ids.push((preview.preview_id, parent, item.title));
        }
        for (preview, parent, title) in ids {
            service.execute(preview).await.unwrap();
            assert!(std::fs::read_to_string(parent.join("正片.nfo")).unwrap().contains(&format!("<title>{title}</title>")));
            assert_eq!(std::fs::read(parent.join("正片.mp4")).unwrap(), b"unchanged nested movie");
        }
        assert!(!temp.path().join("media/正片.nfo").exists());
        repo.pool().close().await;
    }

    async fn adult_library() -> (
        tempfile::TempDir,
        LibraryRepository,
        MediaOutputService,
        AdultOutputAccess,
        Uuid,
    ) {
        let (temp, repo, _, id) = library().await;
        let mut item = repo.get_media(id).await.unwrap().unwrap();
        item.kind = MediaKind::AdultVideo;
        repo.save_media(&item).await.unwrap();
        let root_id = repo.list_files(id).await.unwrap()[0].root_id;
        let mut root = repo.get_root(root_id).await.unwrap().unwrap();
        root.kind = Some(MediaKind::AdultVideo);
        repo.save_root(&root).await.unwrap();
        let access = AdultOutputAccess::new();
        let service = MediaOutputService::new_adult(
            repo.clone(),
            temp.path().join("adult-cache"),
            access.clone(),
        );
        (temp, repo, service, access, id)
    }
    #[tokio::test]
    async fn adult_channel_is_explicit_revocable_and_never_crosses_ordinary_plans() {
        let (temp, repo, service, access, id) = adult_library().await;
        let ordinary = MediaOutputService::new(repo.clone(), temp.path().join("cache"));
        assert_eq!(
            ordinary.preview(id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        let preview = service.preview(id).await.unwrap();
        assert_eq!(
            ordinary.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        assert_eq!(
            ordinary.discard(preview.preview_id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        assert!(!temp.path().join("media/Example.nfo").exists());
        access.revoke();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::Provider(ProviderFailure::Cancelled)
        );
        assert!(!temp.path().join("media/Example.nfo").exists());
        let fresh = MediaOutputService::new_adult(
            repo.clone(),
            temp.path().join("adult-cache"),
            AdultOutputAccess::new(),
        );
        // A fresh grant obtains its own preview and platform source lease.
        let fresh_preview = fresh.preview(id).await.unwrap();
        assert_eq!(
            fresh
                .execute(fresh_preview.preview_id)
                .await
                .unwrap()
                .written,
            1
        );
        assert_eq!(
            std::fs::read(temp.path().join("media/Example.mkv")).unwrap(),
            b"untouched video bytes"
        );
        sqlx::query("UPDATE media_items SET adult_isolated=0 WHERE id=?")
            .bind(id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        assert_eq!(
            fresh.execute(fresh_preview.preview_id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
    }
    #[tokio::test]
    async fn wrong_channel_rejection_does_not_consume_authorized_output_preview() {
        let (temp, repo, service, _access, id) = adult_library().await;
        let ordinary = MediaOutputService::new(repo, temp.path().join("cache"));
        let preview = service.preview(id).await.unwrap();
        assert_eq!(
            ordinary.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        assert_eq!(
            ordinary.discard(preview.preview_id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap().written,
            1
        );
        assert_eq!(
            std::fs::read(temp.path().join("media/Example.mkv")).unwrap(),
            b"untouched video bytes"
        );
    }
    #[test]
    fn adult_nfo_projects_catalog_studio_and_scoped_people_without_movie_matching() {
        let now = Utc::now();
        let item = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::AdultVideo,
            title: "Synthetic neutral metadata".into(),
            original_title: None,
            year: None,
            created_at: now,
            updated_at: now,
        };
        let values: Vec<_> = [("catalog_numbers", serde_json::json!([{"raw":"syn_001","normalized":"SYN-001","namespace":"synthetic","is_primary":true}])), ("studio",serde_json::json!("Synthetic & Co")), ("label",serde_json::json!("Synthetic label")), ("series",serde_json::json!("Synthetic series")), ("performers",serde_json::json!([{"provider_id":"synthetic","remote_id":"person-a","name":"Neutral person","aliases":[]}]))].into_iter().map(|(key,value)| MetadataValue { id:Uuid::new_v4(),media_item_id:item.id,field_key:key.into(),locale:None,value,provider_id:Some("synthetic".into()),source_url:None,fetched_at:Some(now),confidence:None,is_user_modified:false,is_locked:false }).collect();
        let nfo = render_nfo(&item, &values, &[]).unwrap();
        for expected in [
            "<catalognumber>SYN-001</catalognumber>",
            "<catalognamespace>synthetic</catalognamespace>",
            "<studio>Synthetic &amp; Co</studio>",
            "<label>Synthetic label</label>",
            "<set>Synthetic series</set>",
            "<actor><name>Neutral person</name></actor>",
        ] {
            assert!(nfo.contains(expected), "{expected}");
        }
    }
    #[tokio::test]
    async fn selected_adult_cover_and_samples_export_only_from_separate_approved_cache() {
        let (temp, repo, service, access, id) = adult_library().await;
        let cache = temp.path().join("adult-cache");
        std::fs::create_dir(&cache).unwrap();
        let bytes = png();
        let content_hash = hash(&bytes);
        let cached = cache.join(format!("{content_hash}.png"));
        std::fs::write(&cached, &bytes).unwrap();
        for kind in [
            ArtworkKind::Cover,
            ArtworkKind::BackCover,
            ArtworkKind::Sample,
            ArtworkKind::Sample,
        ] {
            repo.save_artwork(&mediaatlas_domain::Artwork {
                id: Uuid::new_v4(),
                media_item_id: id,
                kind,
                cache_path: Some(cached.to_string_lossy().into_owned()),
                source_url: None,
                provider_id: Some("synthetic.image".into()),
                width: Some(32),
                height: Some(48),
                content_hash: Some(content_hash.clone()),
                created_at: Utc::now(),
            })
            .await
            .unwrap();
        }
        let preview = service.preview(id).await.unwrap();
        assert_eq!(preview.files.len(), 5);
        assert!(!temp.path().join("media/Example-cover.png").exists());
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap().written,
            5
        );
        for name in [
            "Example-cover.png",
            "Example-back_cover.png",
            "Example-sample-01.png",
            "Example-sample-02.png",
        ] {
            assert_eq!(
                std::fs::read(temp.path().join("media").join(name)).unwrap(),
                bytes
            );
        }
        let ordinary = MediaOutputService::new(repo, temp.path().join("cache"));
        let mut job = image_job();
        job.media_kind = Some(MediaKind::AdultVideo);
        assert_eq!(
            ordinary
                .cache_artwork(
                    &NetworkEngine::for_loopback(1),
                    &job,
                    "http://127.0.0.1:1/not-requested",
                    &CancellationToken::new()
                )
                .await
                .unwrap_err(),
            ProviderFailure::InvalidConfiguration
        );
        access.revoke();
        assert_eq!(
            service
                .cache_artwork(
                    &NetworkEngine::for_loopback(1),
                    &job,
                    "http://127.0.0.1:1/not-requested",
                    &CancellationToken::new()
                )
                .await
                .unwrap_err(),
            ProviderFailure::Cancelled
        );
        assert_eq!(
            std::fs::read(temp.path().join("media/Example.mkv")).unwrap(),
            b"untouched video bytes"
        );
    }
    #[test]
    fn revoked_grant_after_staging_cannot_publish_and_cleans_the_temporary_file() {
        let temp = crate::test_support::tempdir().unwrap();
        let dir = open_absolute_dir(temp.path(), false).unwrap();
        let access = AdultOutputAccess::new();
        assert_eq!(
            atomic_create_checked(&dir, "never.nfo", b"synthetic staged payload", || {
                access.revoke();
                access.ensure()
            })
            .unwrap_err(),
            RepositoryError::Provider(ProviderFailure::Cancelled)
        );
        assert!(!temp.path().join("never.nfo").exists());
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }
    #[tokio::test]
    async fn cancelled_output_token_rejects_unstarted_publication() {
        let (temp, _, service, _, id) = adult_library().await;
        let preview = service.preview(id).await.unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            service
                .execute_cancellable(preview.preview_id, Some(&cancel))
                .await
                .unwrap_err(),
            RepositoryError::Provider(ProviderFailure::Cancelled)
        );
        assert!(!temp.path().join("media/Example.nfo").exists());
    }
    #[tokio::test]
    async fn automatic_nfo_creates_only_nfo_records_it_and_preserves_existing_bytes() {
        let (temp, repo, service, id) = library().await;
        repo.save_artwork(&mediaatlas_domain::Artwork {
            id: Uuid::new_v4(),
            media_item_id: id,
            kind: ArtworkKind::Poster,
            cache_path: Some("untrusted/missing.jpg".into()),
            source_url: None,
            provider_id: None,
            width: None,
            height: None,
            content_hash: Some("not-read-for-nfo-export".into()),
            created_at: Utc::now(),
        })
        .await
        .unwrap();
        service.automatic_nfo(id, None, None).await.unwrap();
        let path = temp.path().join("media/Example.nfo");
        let first = std::fs::read(&path).unwrap();
        assert!(String::from_utf8_lossy(&first).contains("A &amp; B &lt;fixture&gt;"));
        assert_eq!(
            std::fs::read(temp.path().join("media/Example.mkv")).unwrap(),
            b"untouched video bytes"
        );
        assert_eq!(
            std::fs::read_dir(temp.path().join("media"))
                .unwrap()
                .count(),
            2
        );
        assert_eq!(
            repo.list_files(id).await.unwrap().len(),
            1,
            "sidecar output does not alter source-file proof"
        );
        let mut item = repo.get_media(id).await.unwrap().unwrap();
        item.title = "New database title".into();
        repo.save_media(&item).await.unwrap();
        service.automatic_nfo(id, None, None).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
        assert_eq!(
            std::fs::read_dir(temp.path().join("media"))
                .unwrap()
                .count(),
            2
        );
        let state: String =
            sqlx::query_scalar("SELECT state FROM automatic_nfo_exports WHERE media_item_id=?")
                .bind(id.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
        assert_eq!(state, "preserved");
    }
    #[tokio::test]
    async fn automatic_nfo_failure_is_separate_from_metadata_and_cancel_never_writes() {
        let (temp, repo, service, id) = library().await;
        std::fs::write(
            temp.path().join("media/Example.nfo"),
            b"invalid external XML",
        )
        .unwrap();
        service.automatic_nfo(id, None, None).await.unwrap();
        let state: String =
            sqlx::query_scalar("SELECT state FROM automatic_nfo_exports WHERE media_item_id=?")
                .bind(id.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
        assert_eq!(state, "failed");
        assert_eq!(
            repo.get_media(id).await.unwrap().unwrap().title,
            "A & B <fixture>"
        );
        assert_eq!(
            std::fs::read(temp.path().join("media/Example.nfo")).unwrap(),
            b"invalid external XML"
        );
        std::fs::remove_file(temp.path().join("media/Example.nfo")).unwrap();
        let recovery = service.preview(id).await.unwrap();
        service.execute(recovery.preview_id).await.unwrap();
        let state: String =
            sqlx::query_scalar("SELECT state FROM automatic_nfo_exports WHERE media_item_id=?")
                .bind(id.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
        assert_eq!(
            state, "written",
            "explicit output clears stale automatic failure"
        );
        std::fs::remove_file(temp.path().join("media/Example.nfo")).unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            service
                .automatic_nfo(id, Some(&cancel), None)
                .await
                .unwrap_err(),
            RepositoryError::Provider(ProviderFailure::Cancelled)
        );
        assert!(!temp.path().join("media/Example.nfo").exists());
    }
    #[tokio::test]
    async fn automatic_nfo_does_not_let_a_stale_version_block_an_available_version() {
        let (temp, repo, service, id) = library().await;
        let original = repo.list_files(id).await.unwrap().remove(0);
        repo.save_file(&mediaatlas_domain::MediaFile {
            id: Uuid::new_v4(),
            relative_path: "A-missing.mkv".into(),
            ..original
        })
        .await
        .unwrap();
        service.automatic_nfo(id, None, None).await.unwrap();
        assert!(temp.path().join("media/Example.nfo").is_file());
        assert!(!temp.path().join("media/A-missing.nfo").exists());
        let state: String =
            sqlx::query_scalar("SELECT state FROM automatic_nfo_exports WHERE media_item_id=?")
                .bind(id.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
        assert_eq!(state, "failed");
    }
    #[tokio::test]
    async fn automatic_nfo_preserves_but_reports_empty_external_xml() {
        let (temp, repo, service, id) = library().await;
        std::fs::write(temp.path().join("media/Example.nfo"), b"<movie/>").unwrap();
        service.automatic_nfo(id, None, None).await.unwrap();
        assert_eq!(
            std::fs::read(temp.path().join("media/Example.nfo")).unwrap(),
            b"<movie/>"
        );
        let state: String =
            sqlx::query_scalar("SELECT state FROM automatic_nfo_exports WHERE media_item_id=?")
                .bind(id.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
        assert_eq!(state, "failed");
    }
    #[tokio::test]
    async fn automatic_adult_nfo_requires_current_grant() {
        let (temp, _, service, access, id) = adult_library().await;
        access.revoke();
        assert_eq!(
            service.automatic_nfo(id, None, None).await.unwrap_err(),
            RepositoryError::Provider(ProviderFailure::Cancelled)
        );
        assert!(!temp.path().join("media/Example.nfo").exists());
    }
    #[tokio::test]
    async fn automatic_nfo_rechecks_full_scan_context_at_file_publication() {
        let (temp, repo, service, id) = library().await;
        let source = repo.list_files(id).await.unwrap().remove(0);
        let root = repo.get_root(source.root_id).await.unwrap().unwrap();
        let task_id = Uuid::new_v4();
        let now = Utc::now().timestamp_millis();
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES(?,'scan_library','running','{}',?,?)").bind(task_id.to_string()).bind(now).bind(now).execute(repo.pool()).await.unwrap();
        sqlx::query("INSERT INTO scan_sessions(task_id,root_id,root_snapshot_json,mode,phase) VALUES(?,?,'{}','automatic','metadata')").bind(task_id.to_string()).bind(root.id.to_string()).execute(repo.pool()).await.unwrap();
        let normalized: String =
            sqlx::query_scalar("SELECT normalized_relative_path FROM media_files WHERE id=?")
                .bind(source.id.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
        sqlx::query("INSERT INTO scan_candidates(id,task_id,root_id,kind,relative_path,normalized_relative_path,payload_json,status,existing_media_id) VALUES(?,?,?,'movie',?,?,'{}','imported',?)").bind(Uuid::new_v4().to_string()).bind(task_id.to_string()).bind(root.id.to_string()).bind(&source.relative_path).bind(normalized).bind(id.to_string()).execute(repo.pool()).await.unwrap();
        let context = crate::scraper::automatic::AutomaticContext {
            metadata_providers: None,
            root: root.clone(),
            exclusions: vec![],
            task_id,
            cancel: CancellationToken::new(),
            adult_access: None,
        };
        sqlx::query("INSERT INTO root_settings(root_id,metadata_providers_json) VALUES(?,'[\"tmdb\"]') ON CONFLICT(root_id) DO UPDATE SET metadata_providers_json='[\"tmdb\"]'")
            .bind(root.id.to_string()).execute(repo.pool()).await.unwrap();
        let mut changed_source = repo.pool().begin().await.unwrap();
        assert_eq!(context.validate_tx(&mut changed_source, id).await.unwrap_err(), RepositoryError::Conflict);
        changed_source.rollback().await.unwrap();
        sqlx::query("UPDATE root_settings SET metadata_providers_json=NULL WHERE root_id=?")
            .bind(root.id.to_string()).execute(repo.pool()).await.unwrap();
        let preview = service
            .preview_inner(id, true, Some(source.id), NfoExportMode::Online, false)
            .await
            .unwrap();
        sqlx::query("INSERT INTO root_settings(root_id,exclude_patterns_json) VALUES(?, '[\"**/*.mkv\"]') ON CONFLICT(root_id) DO UPDATE SET exclude_patterns_json=excluded.exclude_patterns_json").bind(root.id.to_string()).execute(repo.pool()).await.unwrap();
        assert_eq!(
            service
                .execute_inner(preview.preview_id, Some(&context.cancel), Some(&context))
                .await
                .unwrap_err(),
            RepositoryError::Conflict
        );
        assert!(!temp.path().join("media/Example.nfo").exists());
    }
    #[tokio::test]
    async fn automatic_series_nfo_maps_to_common_show_directory_and_keeps_episode_nfo() {
        let (temp, repo, service, id) = library().await;
        let mut show = repo.get_media(id).await.unwrap().unwrap();
        show.kind = MediaKind::TvSeries;
        repo.save_media(&show).await.unwrap();
        let original = repo.list_files(id).await.unwrap().remove(0);
        let season = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::TvSeason,
            title: "Season 1".into(),
            ..show.clone()
        };
        let episode = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::TvEpisode,
            title: "Episode 1".into(),
            ..show.clone()
        };
        repo.save_media(&season).await.unwrap();
        repo.save_media(&episode).await.unwrap();
        sqlx::query("INSERT INTO media_relations(parent_id,child_id,relation_type) VALUES(?,?,'season'),(?,?,'episode')").bind(id.to_string()).bind(season.id.to_string()).bind(season.id.to_string()).bind(episode.id.to_string()).execute(repo.pool()).await.unwrap();
        std::fs::create_dir_all(temp.path().join("media/Show/Season 01")).unwrap();
        std::fs::rename(
            temp.path().join("media/Example.mkv"),
            temp.path().join("media/Show/Season 01/Show.S01E01.mkv"),
        )
        .unwrap();
        sqlx::query("DELETE FROM media_files WHERE id=?")
            .bind(original.id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        repo.save_file(&mediaatlas_domain::MediaFile {
            media_item_id: episode.id,
            relative_path: "Show/Season 01/Show.S01E01.mkv".into(),
            ..original
        })
        .await
        .unwrap();
        let episode_nfo = "<episodedetails><title>External episode title</title></episodedetails>";
        std::fs::write(
            temp.path().join("media/Show/Season 01/Show.S01E01.nfo"),
            episode_nfo,
        )
        .unwrap();
        service.automatic_nfo(id, None, None).await.unwrap();
        let destination: String = sqlx::query_scalar(
            "SELECT json_extract(snapshot,'$.preview.files[0].destination') FROM output_previews WHERE media_item_id=?",
        )
        .bind(id.to_string())
        .fetch_one(repo.pool())
        .await
        .unwrap();
        assert_eq!(destination, "Show/tvshow.nfo");
        let state: String =
            sqlx::query_scalar("SELECT state FROM automatic_nfo_exports WHERE media_item_id=?")
                .bind(id.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
        assert_eq!(state, "written");
        let output = std::fs::read_to_string(temp.path().join("media/Show/tvshow.nfo")).unwrap();
        assert!(output.contains("<tvshow>"));
        assert!(output.contains("A &amp; B"));
        assert_eq!(
            std::fs::read_to_string(temp.path().join("media/Show/Season 01/Show.S01E01.nfo"))
                .unwrap(),
            episode_nfo
        );
        assert!(!temp.path().join("media/Show/Season 01/tvshow.nfo").exists());
    }
    #[tokio::test]
    async fn automatic_series_nfo_rejects_mixed_show_directory() {
        for parent in ["", "Collection/Show"] {
            let (temp, repo, service, id) = library().await;
            let mut show = repo.get_media(id).await.unwrap().unwrap();
            show.kind = MediaKind::TvSeries;
            repo.save_media(&show).await.unwrap();
            let other = MediaItem {
                id: Uuid::new_v4(),
                kind: MediaKind::TvEpisode,
                title: "Other show".into(),
                ..show.clone()
            };
            repo.save_media(&other).await.unwrap();
            let mut source = repo.list_files(id).await.unwrap().remove(0);
            let directory = temp.path().join("media").join(parent);
            if !parent.is_empty() {
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::rename(
                    temp.path().join("media/Example.mkv"),
                    directory.join("Example.mkv"),
                )
                .unwrap();
                source.relative_path = format!("{parent}/Example.mkv");
                repo.save_file(&source).await.unwrap();
            }
            std::fs::write(directory.join("OtherShow.S01E01.mkv"), b"other series").unwrap();
            repo.save_file(&mediaatlas_domain::MediaFile {
                id: Uuid::new_v4(),
                media_item_id: other.id,
                relative_path: if parent.is_empty() {
                    "OtherShow.S01E01.mkv".into()
                } else {
                    format!("{parent}/OtherShow.S01E01.mkv")
                },
                ..source
            })
            .await
            .unwrap();
            service.automatic_nfo(id, None, None).await.unwrap();
            assert!(!directory.join("tvshow.nfo").exists());
            let state: String =
                sqlx::query_scalar("SELECT state FROM automatic_nfo_exports WHERE media_item_id=?")
                    .bind(id.to_string())
                    .fetch_one(repo.pool())
                    .await
                    .unwrap();
            assert_eq!(state, "failed");
        }
    }
    #[tokio::test]
    async fn automatic_nfo_preserves_a_late_external_sidecar_with_different_casing() {
        let (temp, _, service, id) = library().await;
        let preview = service.preview_inner(id, true, None, NfoExportMode::Online, false).await.unwrap();
        let local = "<movie><title>Late external sidecar</title></movie>";
        std::fs::write(temp.path().join("media/MOVIE.NFO"), local).unwrap();
        assert_eq!(
            service
                .execute_inner(preview.preview_id, None, None)
                .await
                .unwrap_err(),
            RepositoryError::Conflict
        );
        assert!(!temp.path().join("media/Example.nfo").exists());
        assert_eq!(
            std::fs::read_to_string(temp.path().join("media/MOVIE.NFO")).unwrap(),
            local
        );
    }
    #[tokio::test]
    async fn automatic_versions_preserve_each_target_and_generic_directory_sidecars() {
        let (temp, repo, service, id) = library().await;
        let original = repo.list_files(id).await.unwrap().remove(0);
        std::fs::write(temp.path().join("media/Other.mkv"), b"second version").unwrap();
        repo.save_file(&mediaatlas_domain::MediaFile {
            id: Uuid::new_v4(),
            relative_path: "Other.mkv".into(),
            ..original
        })
        .await
        .unwrap();
        let local = "<movie><title>Other tool</title></movie>";
        std::fs::write(temp.path().join("media/Example.NFO"), local).unwrap();
        service.automatic_nfo(id, None, None).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(temp.path().join("media/Example.NFO")).unwrap(),
            local
        );
        // Path lookup aliases casing on common APFS/NTFS volumes; inspect actual entries.
        let mut nfo_names = std::fs::read_dir(temp.path().join("media"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.to_ascii_lowercase().ends_with(".nfo"))
            .collect::<Vec<_>>();
        nfo_names.sort();
        assert_eq!(nfo_names, vec!["Example.NFO", "Other.nfo"]);
        assert!(temp.path().join("media/Other.nfo").exists());
        std::fs::remove_file(temp.path().join("media/Example.NFO")).unwrap();
        std::fs::remove_file(temp.path().join("media/Other.nfo")).unwrap();
        std::fs::write(temp.path().join("media/MOVIE.NFO"), local).unwrap();
        service.automatic_nfo(id, None, None).await.unwrap();
        assert!(!temp.path().join("media/Example.nfo").exists());
        assert!(!temp.path().join("media/Other.nfo").exists());
        assert_eq!(
            std::fs::read_to_string(temp.path().join("media/MOVIE.NFO")).unwrap(),
            local
        );
    }
    #[tokio::test]
    async fn explicit_preview_execute_records_idempotency_and_preserves_source() {
        let (temp, repo, service, id) = library().await;
        let preview = service.preview(id).await.unwrap();
        assert_eq!(preview.strategy, "preserve");
        assert_eq!(preview.files[0].old_text, None);
        assert!(preview.files[0]
            .new_text
            .as_ref()
            .unwrap()
            .contains("A &amp; B &lt;fixture&gt;"));
        let output = temp.path().join("media/Example.nfo");
        assert!(!output.exists());
        let result = service.execute(preview.preview_id).await.unwrap();
        assert_eq!(result.written, 1);
        assert_eq!(
            std::fs::read(temp.path().join("media/Example.mkv")).unwrap(),
            b"untouched video bytes"
        );
        let repeated = service.execute(preview.preview_id).await.unwrap();
        assert!(repeated.already_applied);
        assert_eq!(repeated.written, 0);
        let persisted: String = sqlx::query_scalar("SELECT state FROM file_operations WHERE id=?")
            .bind(preview.preview_id.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(persisted, "succeeded");
        let changed = sqlx::query("UPDATE output_previews SET snapshot='{}' WHERE id=?")
            .bind(preview.preview_id.to_string())
            .execute(repo.pool())
            .await;
        assert!(changed.is_err());
    }
    #[tokio::test]
    async fn existing_nfo_unknown_elements_comments_and_manual_values_roundtrip_exactly() {
        let (temp, _, service, id) = library().await;
        let path = temp.path().join("media/Example.nfo");
        let local="<?xml version=\"1.0\"?><movie custom=\"x\"><title>Manual title</title><!-- keep --><extension provider=\"local\"><nested value=\"42\"/></extension></movie>";
        std::fs::write(&path, local).unwrap();
        let preview = service.preview(id).await.unwrap();
        assert!(preview.files[0].protected);
        assert_eq!(preview.files[0].new_text.as_deref(), Some(local));
        let result = service.execute(preview.preview_id).await.unwrap();
        assert_eq!(result.written, 1);
        assert_eq!(result.preserved, 1);
        assert_eq!(std::fs::read_to_string(path).unwrap(), local);
        let alternate = preview
            .files
            .iter()
            .find(|file| file.destination.contains(".mediaatlas-"))
            .unwrap();
        let revised =
            std::fs::read_to_string(temp.path().join("media").join(&alternate.destination))
                .unwrap();
        assert!(revised.contains("<year>2024</year>"));
        assert!(revised.contains("<title>Manual title</title>"));
        assert!(
            revised.contains("<extension provider=\"local\"><nested value=\"42\"/></extension>")
        );
    }
    #[tokio::test]
    async fn changed_before_image_and_external_new_file_fail_without_clobber() {
        let (temp, repo, service, id) = library().await;
        let preview = service.preview(id).await.unwrap();
        let path = temp.path().join("media/Example.nfo");
        std::fs::write(&path, "<movie><title>intervening edit</title></movie>").unwrap();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("intervening edit"));
        let state: String = sqlx::query_scalar("SELECT state FROM file_operations WHERE id=?")
            .bind(preview.preview_id.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(state, "failed");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap().written,
            1
        );
        let preserved = service.preview(id).await.unwrap();
        std::fs::write(&path, "<movie><title>changed after preview</title></movie>").unwrap();
        assert_eq!(
            service.execute(preserved.preview_id).await.unwrap_err(),
            RepositoryError::Conflict
        );
    }
    #[tokio::test]
    async fn recovery_requires_explicit_execute_and_reconciles_precrash_completed_file() {
        let (temp, repo, service, id) = library().await;
        let preview = service.preview(id).await.unwrap();
        sqlx::query("UPDATE file_operations SET state='running' WHERE id=?")
            .bind(preview.preview_id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        assert_eq!(service.recover_interrupted().await.unwrap(), 1);
        let path = temp.path().join("media/Example.nfo");
        assert!(!path.exists());
        std::fs::write(&path, preview.files[0].new_text.as_ref().unwrap()).unwrap();
        let result = service.execute(preview.preview_id).await.unwrap();
        assert_eq!(result.written, 0);
        assert_eq!(result.preserved, 1);
    }
    #[test]
    fn selective_nfo_merge_preserves_unknowns_attributes_and_explicit_manual_protection() {
        let existing="<movie custom=\"keep\"><title lang=\"x\">Old</title><!--keep--><unknown><title>nested untouched</title></unknown><plot>Manual</plot></movie>";
        let fields = BTreeMap::from([
            ("title".to_owned(), "New & escaped".to_owned()),
            ("plot".to_owned(), "Remote".to_owned()),
            ("year".to_owned(), "2025".to_owned()),
        ]);
        let merged = merge_nfo_known_fields(existing, &fields, &["plot".into()]).unwrap();
        assert!(merged.contains("<title lang=\"x\">New &amp; escaped</title>"));
        assert!(merged.contains("<plot>Manual</plot>"));
        assert!(merged.contains("<!--keep--><unknown><title>nested untouched</title></unknown>"));
        assert!(merged.contains("<year>2025</year>"));
        assert_eq!(
            merge_nfo_known_fields(&merged, &fields, &["plot".into()]).unwrap(),
            merged
        );
        assert!(merge_nfo_known_fields(
            "<movie/>",
            &BTreeMap::from([("title".into(), "new".into())]),
            &[]
        )
        .unwrap()
        .contains("<title>new</title>"));
    }
    #[tokio::test]
    async fn output_rechecks_adult_isolation_root_state_and_disk_after_success() {
        let (temp, repo, service, id) = library().await;
        let preview = service.preview(id).await.unwrap();
        sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
            .bind(id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        assert_eq!(
            service.preview(id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        sqlx::query("UPDATE media_items SET adult_isolated=0 WHERE id=?")
            .bind(id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        let root_id = repo.list_files(id).await.unwrap()[0].root_id;
        let mut root = repo.get_root(root_id).await.unwrap().unwrap();
        root.enabled = false;
        repo.save_root(&root).await.unwrap();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        root.enabled = true;
        repo.save_root(&root).await.unwrap();
        service.execute(preview.preview_id).await.unwrap();
        std::fs::write(
            temp.path().join("media/Example.nfo"),
            "<movie><title>later edit</title></movie>",
        )
        .unwrap();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::Conflict
        );
    }
    #[tokio::test]
    async fn completed_output_with_missing_source_is_a_stale_plan_conflict() {
        let (temp, _, service, id) = library().await;
        let preview = service.preview(id).await.unwrap();
        service.execute(preview.preview_id).await.unwrap();
        service.discard(preview.preview_id).await.unwrap();
        std::fs::remove_file(temp.path().join("media/Example.mkv")).unwrap();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        assert!(temp.path().join("media/Example.nfo").is_file());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn missing_or_replaced_media_source_rejects_preview_plan_execution() {
        let (temp, _, service, id) = library().await;
        let preview = service.preview(id).await.unwrap();
        let source = temp.path().join("media/Example.mkv");
        std::fs::rename(&source, temp.path().join("media/original.mkv")).unwrap();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        std::fs::write(&source, b"untouched video bytes").unwrap();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        assert!(!temp.path().join("media/Example.nfo").exists());
    }
    #[tokio::test]
    async fn decode_global_budget_limits_independent_services_and_cancellation() {
        let budget = decode_budget();
        let permit1 = budget.clone().acquire_owned().await.unwrap();
        let permit2 = budget.acquire_owned().await.unwrap();
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(matches!(
            decode_thumbnail(png(), &cancelled).await,
            Err(ProviderFailure::Cancelled)
        ));
        drop(permit1);
        drop(permit2);
        let decoded = decode_thumbnail(png(), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!((decoded.width, decoded.height), (32, 48));
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn windows_same_session_output_holds_no_delete_lease_and_discard_releases() {
        let (temp, _, service, id) = library().await;
        let preview = service.preview(id).await.unwrap();
        let source = temp.path().join("media/Example.mkv");
        assert!(std::fs::remove_file(&source).is_err());
        assert!(std::fs::rename(temp.path().join("media"), temp.path().join("renamed")).is_err());
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap().written,
            1
        );
        assert!(
            service
                .execute(preview.preview_id)
                .await
                .unwrap()
                .already_applied
        );
        service.discard(preview.preview_id).await.unwrap();
        std::fs::remove_file(source).unwrap();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::Conflict
        );
    }
    #[tokio::test]
    async fn thumbnail_rejects_large_dimensions_before_decoding() {
        let (temp, _, service, _) = library().await;
        std::fs::create_dir(temp.path().join("cache")).unwrap();
        let image = image::DynamicImage::new_rgb8(241, 361);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let cached = CachedArtwork {
            cache_path: String::new(),
            thumbnail_path: temp
                .path()
                .join("cache/test.thumb.png")
                .to_string_lossy()
                .into_owned(),
            content_hash: "test".into(),
            width: 241,
            height: 361,
            mime: "image/png".into(),
        };
        std::fs::write(&cached.thumbnail_path, bytes.into_inner()).unwrap();
        assert_eq!(
            service.thumbnail_bytes(&cached).unwrap_err(),
            RepositoryError::InvalidInput
        );
    }
    #[tokio::test]
    async fn unsafe_xml_and_wrong_document_root_are_rejected_without_writes() {
        let (temp, _, service, id) = library().await;
        for xml in [
            "<!DOCTYPE movie [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><movie>&x;</movie>",
            "<movie><broken></movie>",
            "<html/>",
        ] {
            std::fs::write(temp.path().join("media/Example.nfo"), xml).unwrap();
            assert_eq!(
                service.preview(id).await.unwrap_err(),
                RepositoryError::InvalidInput
            );
            assert_eq!(
                std::fs::read(temp.path().join("media/Example.mkv")).unwrap(),
                b"untouched video bytes"
            );
        }
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn final_symlink_and_replaced_ancestor_never_write_outside_media_root() {
        use std::os::unix::fs::symlink;
        let (temp, _, service, id) = library().await;
        let external = temp.path().join("external");
        std::fs::create_dir(&external).unwrap();
        let preview = service.preview(id).await.unwrap();
        let destination = temp.path().join("media/Example.nfo");
        symlink(external.join("new.nfo"), &destination).unwrap();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        assert!(!external.join("new.nfo").exists());
        std::fs::remove_file(&destination).unwrap();
        std::fs::rename(temp.path().join("media"), temp.path().join("old-media")).unwrap();
        symlink(&external, temp.path().join("media")).unwrap();
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        assert!(!external.join("Example.nfo").exists());
        // Capability retained before an ancestor swap keeps writes on its opened
        // directory, even if a hostile symlink now occupies its old pathname.
        let dir = open_absolute_dir(&temp.path().join("old-media"), false).unwrap();
        std::fs::rename(
            temp.path().join("old-media"),
            temp.path().join("held-media"),
        )
        .unwrap();
        symlink(&external, temp.path().join("old-media")).unwrap();
        assert!(atomic_create(&dir, "held.nfo", b"safe").unwrap());
        assert_eq!(
            std::fs::read(temp.path().join("held-media/held.nfo")).unwrap(),
            b"safe"
        );
        assert!(!external.join("held.nfo").exists());
    }
    fn png() -> Vec<u8> {
        let image = image::DynamicImage::new_rgb8(32, 48);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }
    async fn image_server(
        bytes: Vec<u8>,
        mime: &str,
    ) -> (
        String,
        Arc<std::sync::atomic::AtomicUsize>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mime = mime.to_owned();
        let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = requests.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let bytes = bytes.clone();
                let mime = mime.clone();
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut request = [0; 2048];
                    let _ = stream.read(&mut request).await;
                    let header=format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",bytes.len());
                    let _ = stream.write_all(header.as_bytes()).await;
                    let _ = stream.write_all(&bytes).await;
                });
            }
        });
        (format!("http://{address}/image.png"), requests, task)
    }
    fn image_job() -> ProviderJob {
        ProviderJob {
            id: Uuid::new_v4(),
            provider_id: "mock.image".into(),
            media_item_id: None,
            media_kind: Some(MediaKind::Movie),
            file_identity: None,
            region: None,
            correlation_id: Uuid::new_v4(),
            operation: "artwork".into(),
            query: "fixture".into(),
            language: "en-US".into(),
            config: ProviderConfig {
                enabled: true,
                interval_ms: 0,
                max_retries: 0,
                max_response_bytes: MAX_IMAGE,
                ..Default::default()
            },
            created_at: Utc::now(),
        }
    }
    #[tokio::test]
    async fn mock_image_download_validates_caches_url_and_hash_deduplicates_with_thumbnail() {
        let (temp, repo, service, id) = library().await;
        let engine = NetworkEngine::for_loopback(2);
        let job = image_job();
        let cancel = CancellationToken::new();
        let (url, count, server) = image_server(png(), "image/png").await;
        let first = service
            .cache_artwork(&engine, &job, &url, &cancel)
            .await
            .unwrap();
        assert_eq!((first.width, first.height), (32, 48));
        assert!(Path::new(&first.thumbnail_path).exists());
        let second = service
            .cache_artwork(&engine, &job, &url, &cancel)
            .await
            .unwrap();
        assert_eq!(first.content_hash, second.content_hash);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        let (url2, _, server2) = image_server(png(), "image/png; charset=binary").await;
        let third = service
            .cache_artwork(&engine, &job, &url2, &cancel)
            .await
            .unwrap();
        assert_eq!(first.cache_path, third.cache_path);
        repo.save_artwork(&mediaatlas_domain::Artwork {
            id: Uuid::new_v4(),
            media_item_id: id,
            kind: ArtworkKind::Poster,
            cache_path: Some(first.cache_path.clone()),
            source_url: Some(url),
            provider_id: Some("mock.image".into()),
            width: Some(first.width),
            height: Some(first.height),
            content_hash: Some(first.content_hash),
            created_at: Utc::now(),
        })
        .await
        .unwrap();
        let preview = service.preview(id).await.unwrap();
        assert_eq!(preview.files.len(), 2);
        assert_eq!(
            service.execute(preview.preview_id).await.unwrap().written,
            2
        );
        assert!(temp.path().join("media/Example-poster.png").exists());
        server.abort();
        server2.abort();
    }
    #[tokio::test]
    async fn image_mime_magic_truncation_and_byte_caps_are_enforced() {
        let (_, _, service, _) = library().await;
        let engine = NetworkEngine::for_loopback(2);
        let job = image_job();
        let cancel = CancellationToken::new();
        for (bytes, mime) in [
            (png(), "text/html"),
            (b"<html>not image</html>".to_vec(), "image/png"),
            (png()[..20].to_vec(), "image/png"),
        ] {
            let (url, _, server) = image_server(bytes, mime).await;
            assert!(service
                .cache_artwork(&engine, &job, &url, &cancel)
                .await
                .is_err());
            server.abort();
        }
        assert_eq!(
            decode_image(&vec![0; MAX_IMAGE + 1]).unwrap_err(),
            ProviderFailure::ResponseTooLarge
        );
    }
}
