use crate::{
    parse_filename, read_audio_tags, read_nfo, FileStamp, LocalArtwork, LocalMetadata, ScanError,
    ScanOptions, ScanStats, ScannedFile,
};
use chrono::{DateTime, Utc};
use globset::{Glob, GlobSet, GlobSetBuilder};
use mediaatlas_domain::{normalize_relative_path, ArtworkKind, MediaKind};
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use walkdir::WalkDir;

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "m4v", "wmv", "webm", "ts", "m2ts", "mpg", "mpeg", "strm",
];
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "m4a", "ogg", "opus", "wav", "aiff", "ape", "wv",
];
const MAX_ASSOCIATED_FILES: usize = 1024;

fn file_mtime(info: &fs::Metadata) -> Option<DateTime<Utc>> {
    info.modified()
        .ok()
        .map(DateTime::<Utc>::from)
        .and_then(|time| DateTime::from_timestamp_millis(time.timestamp_millis()))
}

fn relative(root: &Path, path: &Path) -> Result<String, ScanError> {
    path.strip_prefix(root)
        .ok()
        .and_then(Path::to_str)
        .map(|v| v.replace(std::path::MAIN_SEPARATOR, "/"))
        .ok_or(ScanError::Io)
}

// Reserved application staging/quarantine prefixes are never media candidates.
// Hidden user files and Git ignore files retain the configured exclusion semantics.
fn private_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            let name = name.to_ascii_lowercase();
            name.starts_with(".mediaatlas-") || name.starts_with(".mediaatlas_")
        })
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn exclude_set(patterns: &[String]) -> Result<GlobSet, ScanError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).map_err(|_| ScanError::InvalidPattern)?);
    }
    builder.build().map_err(|_| ScanError::InvalidPattern)
}

#[derive(Default)]
struct Sidecars {
    associated: Vec<String>,
    artworks: Vec<LocalArtwork>,
    nfo: Vec<PathBuf>,
    fingerprint: Vec<(String, u64, Option<DateTime<Utc>>)>,
}

struct SidecarFile {
    path: PathBuf,
    relative_path: String,
    filename: String,
    extension: String,
    size: u64,
    modified_at: Option<DateTime<Utc>>,
}

#[derive(Default)]
struct DirectorySidecars {
    files: Vec<SidecarFile>,
    by_stem: HashMap<String, Vec<usize>>,
    generic: Vec<usize>,
}

impl DirectorySidecars {
    fn read(
        root: &Path,
        parent: &Path,
        excludes: &GlobSet,
        cancel: &AtomicBool,
    ) -> Result<Self, ScanError> {
        let mut index = Self::default();
        let canonical_root = root.canonicalize().map_err(|_| ScanError::InvalidRoot)?;
        for entry in fs::read_dir(parent).map_err(|_| ScanError::Io)? {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let entry = entry.map_err(|_| ScanError::Io)?;
            let path = entry.path();
            let filename = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let extension = path
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !matches!(
                extension.as_str(),
                "nfo"
                    | "srt"
                    | "ass"
                    | "ssa"
                    | "sub"
                    | "idx"
                    | "vtt"
                    | "jpg"
                    | "jpeg"
                    | "png"
                    | "webp"
            ) {
                continue;
            }
            let rel = relative(root, &path)?;
            if private_name(&path) || excludes.is_match(&rel) {
                continue;
            }
            let info = fs::symlink_metadata(&path).map_err(|_| ScanError::Io)?;
            if !info.is_file() || info.file_type().is_symlink() {
                continue;
            }
            if !path
                .canonicalize()
                .map_err(|_| ScanError::Io)?
                .starts_with(&canonical_root)
            {
                continue;
            }
            // Directory-local bounded cache: no media candidate tree or unbounded path map.
            if index.files.len() >= 65_536 {
                return Err(ScanError::MetadataTooLarge);
            }
            let id = index.files.len();
            let generic = matches!(
                filename.as_str(),
                "movie.nfo"
                    | "tvshow.nfo"
                    | "album.nfo"
                    | "poster.jpg"
                    | "poster.png"
                    | "poster.jpeg"
                    | "folder.jpg"
                    | "cover.jpg"
                    | "cover.png"
            ) || filename.starts_with("fanart.")
                || filename.starts_with("backdrop.");
            if generic {
                index.generic.push(id);
            }
            for (boundary, separator) in filename.char_indices() {
                if boundary > 0 && matches!(separator, '.' | '-') {
                    index
                        .by_stem
                        .entry(filename[..boundary].into())
                        .or_default()
                        .push(id);
                }
            }
            index.files.push(SidecarFile {
                path,
                relative_path: rel,
                filename,
                extension,
                size: info.len(),
                modified_at: file_mtime(&info),
            });
        }
        Ok(index)
    }

    fn for_media(&self, media: &Path) -> Result<Sidecars, ScanError> {
        let stem = media
            .file_stem()
            .and_then(|p| p.to_str())
            .ok_or(ScanError::Io)?
            .to_ascii_lowercase();
        let mut result = Sidecars::default();
        let mut ids = self.generic.clone();
        if let Some(matching) = self.by_stem.get(&stem) {
            ids.extend(matching);
        }
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            let file = &self.files[id];
            let filename = &file.filename;
            let extension = &file.extension;
            let same_stem = filename.starts_with(&format!("{stem}."))
                || filename.starts_with(&format!("{stem}-"));
            let generic_nfo = matches!(filename.as_str(), "movie.nfo" | "tvshow.nfo" | "album.nfo");
            let art_kind = if matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "webp") {
                if matches!(
                    filename.as_str(),
                    "poster.jpg"
                        | "poster.png"
                        | "poster.jpeg"
                        | "folder.jpg"
                        | "cover.jpg"
                        | "cover.png"
                ) || (same_stem && filename.contains("poster"))
                {
                    Some(ArtworkKind::Poster)
                } else if filename.starts_with("fanart.")
                    || filename.starts_with("backdrop.")
                    || (same_stem && filename.contains("fanart"))
                {
                    Some(ArtworkKind::Backdrop)
                } else {
                    None
                }
            } else {
                None
            };
            let associated = (extension == "nfo" && (same_stem || generic_nfo))
                || (matches!(
                    extension.as_str(),
                    "srt" | "ass" | "ssa" | "sub" | "idx" | "vtt"
                ) && same_stem)
                || art_kind.is_some();
            if !associated {
                continue;
            }
            if result.associated.len() >= MAX_ASSOCIATED_FILES {
                return Err(ScanError::MetadataTooLarge);
            }
            let rel = file.relative_path.clone();
            if extension == "nfo" {
                result.nfo.push(file.path.clone());
            }
            if let Some(kind) = art_kind {
                result.artworks.push(LocalArtwork {
                    relative_path: rel.clone(),
                    kind,
                });
            }
            result
                .fingerprint
                .push((rel.clone(), file.size, file.modified_at));
            result.associated.push(rel);
        }
        result.associated.sort();
        result.nfo.sort();
        result
            .artworks
            .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        result.fingerprint.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }
}

fn series_media(kind: MediaKind) -> bool {
    matches!(kind, MediaKind::TvEpisode | MediaKind::Anime)
}
fn include_series_sidecar(
    root: &Path,
    media: &Path,
    kind: MediaKind,
    excludes: &GlobSet,
    cancel: &AtomicBool,
    sidecars: &mut Sidecars,
) -> Result<(), ScanError> {
    if !series_media(kind)
        || sidecars.nfo.iter().any(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("tvshow.nfo"))
        })
    {
        return Ok(());
    }
    let parent = media.parent().ok_or(ScanError::Io)?;
    let Some(series_dir) = parent.parent().filter(|path| path.starts_with(root)) else {
        return Ok(());
    };
    // Only the conventional one-level season folder can inherit a show NFO.
    let name = parent
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let season = name
        .strip_prefix("season")
        .or_else(|| name.strip_prefix('s'));
    if !season.is_some_and(|suffix| {
        !suffix.trim().is_empty() && suffix.trim().chars().all(|c| c.is_ascii_digit())
    }) {
        return Ok(());
    }
    let index = DirectorySidecars::read(root, series_dir, excludes, cancel)?;
    if let Some(file) = index
        .files
        .iter()
        .find(|file| file.filename == "tvshow.nfo")
    {
        sidecars.nfo.push(file.path.clone());
        sidecars.associated.push(file.relative_path.clone());
        sidecars
            .fingerprint
            .push((file.relative_path.clone(), file.size, file.modified_at));
        sidecars.nfo.sort();
        sidecars.associated.sort();
        sidecars.fingerprint.sort_by(|a, b| a.0.cmp(&b.0));
    }
    Ok(())
}

/// Re-discover companions for confirm-time TOCTOU checks, including additions/deletions.
pub fn sidecar_fingerprint(root: &Path, entry: &ScannedFile) -> Result<String, ScanError> {
    let rel = Path::new(&entry.relative_path);
    if rel.is_absolute()
        || rel.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
            )
        })
    {
        return Err(ScanError::Io);
    }
    let path = root.join(rel);
    let canonical_root = root.canonicalize().map_err(|_| ScanError::InvalidRoot)?;
    let canonical = path.canonicalize().map_err(|_| ScanError::Io)?;
    if !canonical.starts_with(canonical_root)
        || fs::symlink_metadata(&path)
            .map_err(|_| ScanError::Io)?
            .file_type()
            .is_symlink()
    {
        return Err(ScanError::Io);
    }
    let excludes = exclude_set(&entry.exclude_patterns_snapshot)?;
    let mut sidecars = DirectorySidecars::read(
        root,
        path.parent().ok_or(ScanError::Io)?,
        &excludes,
        &AtomicBool::new(false),
    )?
    .for_media(&path)?;
    include_series_sidecar(
        root,
        &path,
        entry.parsed.kind,
        &excludes,
        &AtomicBool::new(false),
        &mut sidecars,
    )?;
    serde_json::to_string(&sidecars.fingerprint).map_err(|_| ScanError::InvalidMetadata)
}

/// Stream candidates one at a time. No tree-sized candidate collection is retained.
pub fn run_scan(
    root: &Path,
    options: &ScanOptions,
    cancel: &AtomicBool,
    mut on_entry: impl FnMut(ScannedFile) -> bool,
    mut on_progress: impl FnMut(ScanStats),
) -> Result<ScanStats, ScanError> {
    let root_info = fs::symlink_metadata(root).map_err(|_| ScanError::InvalidRoot)?;
    if !root_info.is_dir() || root_info.file_type().is_symlink() {
        return Err(ScanError::InvalidRoot);
    }
    let canonical_root = root.canonicalize().map_err(|_| ScanError::InvalidRoot)?;
    let excludes = exclude_set(&options.exclude_patterns)?;
    let mut stats = ScanStats::default();
    // Depth-first walking usually visits a directory contiguously. Keep at most
    // eight sidecar-only indexes for interleaved parent/child entries.
    let mut sidecar_cache: VecDeque<(PathBuf, DirectorySidecars)> = VecDeque::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0
                || (!private_name(entry.path())
                    && !relative(root, entry.path()).is_ok_and(|path| {
                        excludes.is_match(&path) || excludes.is_match(format!("{path}/"))
                    }))
        });
    for next in walker {
        if cancel.load(Ordering::Relaxed) {
            stats.cancelled = true;
            break;
        }
        let entry = match next {
            Ok(entry) => entry,
            Err(_) => {
                stats.warnings += 1;
                on_progress(stats);
                continue;
            }
        };
        if entry.file_type().is_symlink() {
            stats.skipped_files += 1;
            on_progress(stats);
            continue;
        }
        if !entry.file_type().is_file() {
            on_progress(stats);
            continue;
        }
        stats.visited_files += 1;
        let path = entry.path();
        if !path
            .canonicalize()
            .is_ok_and(|path| path.starts_with(&canonical_root))
        {
            stats.warnings += 1;
            stats.skipped_files += 1;
            on_progress(stats);
            continue;
        }
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !VIDEO_EXTENSIONS.contains(&extension.as_str())
            && !AUDIO_EXTENSIONS.contains(&extension.as_str())
        {
            stats.skipped_files += 1;
            on_progress(stats);
            continue;
        }
        let rel = match relative(root, path) {
            Ok(value) => value,
            Err(_) => {
                stats.warnings += 1;
                stats.skipped_files += 1;
                on_progress(stats);
                continue;
            }
        };
        let normalized = match normalize_relative_path(&rel, options.path_style) {
            Ok(value) => value,
            Err(_) => {
                stats.warnings += 1;
                stats.skipped_files += 1;
                on_progress(stats);
                continue;
            }
        };
        let info = match entry.metadata() {
            Ok(value) => value,
            Err(_) => {
                stats.warnings += 1;
                stats.skipped_files += 1;
                on_progress(stats);
                continue;
            }
        };
        let modified_at = file_mtime(&info);
        let stamp = FileStamp {
            size_bytes: info.len(),
            modified_at,
        };
        let unchanged = options.known_files.get(&normalized) == Some(&stamp);
        let media_kind = if is_audio(path) {
            Some(MediaKind::MusicTrack)
        } else {
            options.kind
        };
        let mut parsed = parse_filename(&rel, media_kind);
        let mut local = LocalMetadata::default();
        if is_audio(path) {
            match read_audio_tags(path) {
                Ok(tags) => local = tags,
                Err(error) => {
                    parsed.warnings.push(error.code().into());
                    stats.warnings += 1;
                    parsed.needs_review = true;
                }
            }
        }
        let parent = path.parent().ok_or(ScanError::Io)?;
        let cached = sidecar_cache
            .iter()
            .position(|(directory, _)| directory == parent);
        let companions = if let Some(position) = cached {
            sidecar_cache[position].1.for_media(path)
        } else {
            match DirectorySidecars::read(root, parent, &excludes, cancel) {
                Ok(index) => {
                    let result = index.for_media(path);
                    if sidecar_cache.len() == 8 {
                        sidecar_cache.pop_front();
                    }
                    sidecar_cache.push_back((parent.to_owned(), index));
                    result
                }
                Err(error) => Err(error),
            }
        };
        if cancel.load(Ordering::Relaxed) {
            stats.cancelled = true;
            break;
        }
        let mut companions = match companions {
            Ok(value) => value,
            Err(error) => {
                stats.warnings += 1;
                parsed.warnings.push(error.code().into());
                parsed.needs_review = true;
                Sidecars::default()
            }
        };
        if let Err(error) =
            include_series_sidecar(root, path, parsed.kind, &excludes, cancel, &mut companions)
        {
            stats.warnings += 1;
            parsed.warnings.push(error.code().into());
            parsed.needs_review = true;
        }
        let mut series_metadata = None;
        let mut series_nfo = None;
        // Exact file sidecars override directory-wide movie metadata. A show
        // sidecar is applied only to the series graph, never an episode identity.
        let mut nfo_paths = companions.nfo.clone();
        nfo_paths.sort_by_key(|p| p.file_stem() == path.file_stem());
        for nfo in nfo_paths {
            if !nfo
                .canonicalize()
                .is_ok_and(|path| path.starts_with(&canonical_root))
            {
                parsed.warnings.push(ScanError::Io.code().into());
                parsed.needs_review = true;
                stats.warnings += 1;
                continue;
            }
            match read_nfo(&nfo) {
                Ok(metadata) => {
                    if series_media(parsed.kind)
                        && parsed.series_title.is_some()
                        && nfo.file_name().is_some_and(|name| {
                            name.to_string_lossy().eq_ignore_ascii_case("tvshow.nfo")
                        })
                    {
                        series_nfo = Some(relative(root, &nfo)?);
                        series_metadata = Some(metadata);
                    } else {
                        local.fields.extend(metadata.fields);
                        local.external_ids.extend(metadata.external_ids);
                    }
                }
                Err(error) => {
                    parsed.warnings.push(error.code().into());
                    stats.warnings += 1;
                    parsed.needs_review = true;
                }
            }
        }
        if let Some(value) = local
            .fields
            .get("title")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
        {
            parsed.title = value.to_owned();
        }
        if let Some(value) = local
            .fields
            .get("year")
            .and_then(|v| v.as_i64())
            .and_then(|v| i32::try_from(v).ok())
        {
            parsed.year = Some(value);
        }
        if let Some(value) = local
            .fields
            .get("season")
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
        {
            parsed.season_number = Some(value);
        }
        if let Some(value) = local
            .fields
            .get("episode")
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
        {
            parsed.episode_number = Some(value);
        }
        if parsed.kind == MediaKind::AdultVideo && parsed.catalog_number.is_some() {
            local.fields.insert(
                "catalog_number_raw".into(),
                json!(path.file_stem().and_then(|v| v.to_str()).unwrap_or("")),
            );
            local
                .fields
                .insert("catalog_namespace".into(), json!("filename_unverified"));
        }
        let sidecar_fingerprint = serde_json::to_string(&companions.fingerprint)
            .map_err(|_| ScanError::InvalidMetadata)?;
        stats.candidate_count += 1;
        if unchanged {
            stats.skipped_files += 1;
        }
        if !on_entry(ScannedFile {
            relative_path: rel,
            normalized_relative_path: normalized,
            size_bytes: info.len(),
            modified_at,
            parsed,
            local_metadata: local,
            series_metadata,
            series_nfo,
            artworks: companions.artworks,
            associated_files: companions.associated,
            sidecar_fingerprint,
            exclude_patterns_snapshot: options.exclude_patterns.clone(),
            unchanged,
        }) {
            stats.cancelled = true;
            break;
        }
        on_progress(stats);
    }
    on_progress(stats);
    Ok(stats)
}
