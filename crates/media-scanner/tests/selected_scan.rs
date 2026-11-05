use mediaatlas_domain::{MediaKind, PathStyle};
use mediaatlas_media_scanner::{run_scan, run_scan_selected, FileStamp, ScanOptions, ScanStats, ScannedFile};
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use tempfile::tempdir;

fn options(kind: MediaKind) -> ScanOptions {
    ScanOptions {
        kind: Some(kind),
        path_style: if cfg!(windows) { PathStyle::Windows } else { PathStyle::Posix },
        exclude_patterns: vec![],
        known_files: HashMap::new(),
    }
}

fn selected(root: &Path, options: &ScanOptions, paths: &[&str]) -> (Vec<ScannedFile>, ScanStats) {
    let mut entries = Vec::new();
    let paths = paths.iter().map(|path| (*path).to_owned()).collect::<Vec<_>>();
    let stats = run_scan_selected(root, options, &paths, &AtomicBool::new(false), |entry| {
        entries.push(entry);
        true
    }, |_| {}).unwrap();
    (entries, stats)
}

#[test]
fn selected_scan_reads_only_one_nested_movie_and_its_own_sidecars() {
    let dir = tempdir().unwrap();
    for (folder, title) in [("电影/合集/第一部", "First"), ("电影/合集/第二部", "Second")] {
        let parent = dir.path().join(folder);
        fs::create_dir_all(&parent).unwrap();
        fs::write(parent.join("正片.mp4"), b"unchanged source").unwrap();
        fs::write(parent.join("正片.nfo"), format!("<movie><title>{title}</title></movie>")).unwrap();
        fs::write(parent.join("poster.jpg"), b"local poster").unwrap();
    }
    let mut opts = options(MediaKind::Movie);
    let path = "电影/合集/第一部/正片.mp4";
    let (entries, stats) = selected(dir.path(), &opts, &[path, path]);
    assert_eq!(entries.len(), 1);
    assert_eq!(stats.visited_files, 1);
    assert_eq!(stats.candidate_count, 1);
    assert_eq!(stats.warnings, 0);
    assert_eq!(entries[0].parsed.title, "First");
    assert_eq!(entries[0].associated_files, vec!["电影/合集/第一部/poster.jpg", "电影/合集/第一部/正片.nfo"]);
    let mut full = Vec::new();
    run_scan(dir.path(), &opts, &AtomicBool::new(false), |entry| {
        full.push(entry);
        true
    }, |_| {}).unwrap();
    assert_eq!(full.len(), 2);
    assert_eq!(entries[0], *full.iter().find(|entry| entry.relative_path == path).unwrap());
    opts.known_files.insert(entries[0].normalized_relative_path.clone(), FileStamp {
        size_bytes: entries[0].size_bytes,
        modified_at: entries[0].modified_at,
    });
    let (unchanged, _) = selected(dir.path(), &opts, &[path]);
    assert!(unchanged[0].unchanged);
    assert_eq!(fs::read(dir.path().join(path)).unwrap(), b"unchanged source");
}

#[test]
fn selected_episode_keeps_parent_show_nfo_separate_from_its_episode_nfo() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("Show/Season 2")).unwrap();
    fs::write(dir.path().join("Show/tvshow.nfo"), "<tvshow><title>Local show</title></tvshow>").unwrap();
    let path = "Show/Season 2/Show.S02E01.mkv";
    fs::write(dir.path().join(path), b"episode").unwrap();
    fs::write(dir.path().join("Show/Season 2/Show.S02E01.nfo"), "<episodedetails><title>Local episode</title><season>2</season><episode>1</episode></episodedetails>").unwrap();
    let (entries, stats) = selected(dir.path(), &options(MediaKind::TvSeries), &[path]);
    assert_eq!(stats.candidate_count, 1);
    assert_eq!(entries[0].parsed.title, "Local episode");
    assert_eq!(entries[0].series_nfo.as_deref(), Some("Show/tvshow.nfo"));
    assert_eq!(entries[0].series_metadata.as_ref().unwrap().fields["title"], "Local show");
    assert_eq!(entries[0].associated_files.len(), 2);
}

#[test]
fn selected_scan_preserves_ancestor_exclusions_private_prefixes_and_sidecar_exclusions() {
    let dir = tempdir().unwrap();
    for folder in ["Visible", "Excluded", "Trailing", ".MeDiAaTlAs_staging", "Nested/.mediaatlas-private"] {
        fs::create_dir_all(dir.path().join(folder)).unwrap();
        fs::write(dir.path().join(folder).join("Film.mkv"), b"source").unwrap();
    }
    fs::write(dir.path().join("Visible/Film.nfo"), "<movie><title>Excluded metadata</title></movie>").unwrap();
    let mut opts = options(MediaKind::Movie);
    opts.exclude_patterns = vec!["Excluded".into(), "Trailing/".into(), "**/*.nfo".into()];
    let (entries, stats) = selected(dir.path(), &opts, &[
        "Excluded/Film.mkv", "Trailing/Film.mkv", ".MeDiAaTlAs_staging/Film.mkv",
        "Nested/.mediaatlas-private/Film.mkv", "Visible/Film.mkv",
    ]);
    assert_eq!(stats.candidate_count, 1);
    assert_eq!(stats.warnings, 0);
    assert_eq!(entries[0].relative_path, "Visible/Film.mkv");
    assert!(entries[0].associated_files.is_empty());
    assert!(entries[0].local_metadata.fields.is_empty());
}

#[test]
fn selected_scan_never_expands_directories_missing_files_or_unsafe_paths_to_a_full_scan() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("Folder")).unwrap();
    fs::write(dir.path().join("Folder/Nested.mkv"), b"do not discover").unwrap();
    fs::write(dir.path().join("Film.mkv"), b"source").unwrap();
    let absolute = dir.path().join("Film.mkv").to_string_lossy().into_owned();
    let (entries, stats) = selected(dir.path(), &options(MediaKind::Movie), &[
        "Folder", "Missing.mkv", "../Film.mkv", "./Film.mkv", "Folder/../Film.mkv",
        "Folder//Nested.mkv", "Folder\\Nested.mkv", &absolute, "Film.mkv",
    ]);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].relative_path, "Film.mkv");
    assert_eq!(stats.visited_files, 1);
    assert_eq!(stats.warnings, 7);
    let (entries, stats) = selected(&dir.path().join("not-a-root"), &options(MediaKind::Movie), &[]);
    assert!(entries.is_empty());
    assert_eq!(stats, ScanStats::default());
}

#[cfg(unix)]
#[test]
fn selected_posix_filename_with_a_literal_backslash_matches_full_scan() {
    let dir = tempdir().unwrap();
    let path = "Film\\Part.2020.mkv";
    fs::write(dir.path().join(path), b"source").unwrap();
    let opts = options(MediaKind::Movie);
    let (entries, stats) = selected(dir.path(), &opts, &[path]);
    assert_eq!(stats.candidate_count, 1);
    assert_eq!(entries[0].relative_path, path);
    let mut full = Vec::new();
    run_scan(dir.path(), &opts, &AtomicBool::new(false), |entry| { full.push(entry); true }, |_| {}).unwrap();
    assert_eq!(entries, full);
}

#[cfg(unix)]
#[test]
fn selected_scan_rejects_symlink_sources_and_ancestors_even_inside_the_root() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("Real")).unwrap();
    fs::write(dir.path().join("Real/Film.mkv"), b"source").unwrap();
    symlink(dir.path().join("Real"), dir.path().join("Linked")).unwrap();
    symlink(dir.path().join("Real/Film.mkv"), dir.path().join("Linked.mkv")).unwrap();
    let (entries, stats) = selected(dir.path(), &options(MediaKind::Movie), &["Linked/Film.mkv", "Linked.mkv"]);
    assert!(entries.is_empty());
    assert_eq!(stats.warnings, 2);
    assert_eq!(stats.visited_files, 0);
}

#[test]
fn selected_scan_honors_cancellation_and_consumer_stop_without_visiting_other_sources() {
    let dir = tempdir().unwrap();
    for file in ["First.mkv", "Second.mkv"] {
        fs::write(dir.path().join(file), b"source").unwrap();
    }
    let paths = vec!["First.mkv".into(), "Second.mkv".into()];
    let opts = options(MediaKind::Movie);
    let cancel = AtomicBool::new(true);
    let stats = run_scan_selected(dir.path(), &opts, &paths, &cancel, |_| panic!("cancelled scan emitted media"), |_| {}).unwrap();
    assert!(stats.cancelled);
    assert_eq!(stats.visited_files, 0);
    cancel.store(false, Ordering::Relaxed);
    let mut entries = 0;
    let stats = run_scan_selected(dir.path(), &opts, &paths, &cancel, |_| {
        entries += 1;
        cancel.store(true, Ordering::Relaxed);
        true
    }, |_| {}).unwrap();
    assert!(stats.cancelled);
    assert_eq!(entries, 1);
    assert_eq!(stats.visited_files, 1);
    cancel.store(false, Ordering::Relaxed);
    entries = 0;
    let stats = run_scan_selected(dir.path(), &opts, &paths, &cancel, |_| {
        entries += 1;
        false
    }, |_| {}).unwrap();
    assert!(stats.cancelled);
    assert_eq!(entries, 1);
    assert_eq!(stats.visited_files, 1);
}
