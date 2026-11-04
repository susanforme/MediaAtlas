use mediaatlas_domain::{MediaKind, PathStyle};
use mediaatlas_media_scanner::{
    parse_filename, read_audio_tags, read_nfo, run_scan, sidecar_fingerprint, FileStamp, ScanError,
    ScanOptions, ScannedFile,
};
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tempfile::tempdir;

fn options(kind: Option<MediaKind>) -> ScanOptions {
    ScanOptions {
        kind,
        path_style: if cfg!(windows) {
            PathStyle::Windows
        } else {
            PathStyle::Posix
        },
        exclude_patterns: vec![],
        known_files: HashMap::new(),
    }
}

fn collect(root: &Path, options: &ScanOptions) -> Vec<ScannedFile> {
    let mut result = Vec::new();
    run_scan(
        root,
        options,
        &AtomicBool::new(false),
        |entry| {
            result.push(entry);
            true
        },
        |_| {},
    )
    .unwrap();
    result
}

#[test]
fn parsers_preserve_type_specific_identity_and_part_grouping() {
    let first = parse_filename("Movies/Fixture Film (2024) CD1.mkv", Some(MediaKind::Movie));
    let second = parse_filename("Movies/Fixture Film (2024) CD2.mkv", Some(MediaKind::Movie));
    assert_eq!(first.title, "Fixture Film");
    assert_eq!(first.year, Some(2024));
    assert_eq!(first.part, Some(1));
    assert_eq!(second.part, Some(2));
    assert_eq!(first.group_key, second.group_key);
    assert_ne!(
        first.group_key,
        parse_filename("Other/Fixture Film (2024) CD1.mkv", Some(MediaKind::Movie)).group_key
    );
    let episode = parse_filename(
        "Show/Fixture.Show.S02E03.1080p.mkv",
        Some(MediaKind::TvSeries),
    );
    assert_eq!(episode.kind, MediaKind::TvEpisode);
    assert_eq!(episode.season_number, Some(2));
    assert_eq!(episode.episode_number, Some(3));
    let episode2 = parse_filename("Show/Fixture Show 2x04.mkv", Some(MediaKind::TvSeries));
    assert_eq!(episode2.episode_number, Some(4));
    assert_ne!(episode.group_key, episode2.group_key);
    let anime = parse_filename(
        "Anime/[FixtureSubs] Fiction - 012 [1080p].mkv",
        Some(MediaKind::Anime),
    );
    assert_eq!(anime.absolute_number, Some(12));
    assert!(anime.needs_review);
    let adult = parse_filename("Adult/ABP-123 S01E01-CD1.mp4", Some(MediaKind::AdultVideo));
    assert_eq!(adult.kind, MediaKind::AdultVideo);
    assert_eq!(adult.catalog_number.as_deref(), Some("ABP-123"));
    let fc2 = parse_filename("Adult/FC2-PPV-1234567.mp4", Some(MediaKind::AdultVideo));
    assert_eq!(fc2.catalog_number.as_deref(), Some("FC2-PPV-1234567"));
    assert!(parse_filename("Unknown.mkv", None).needs_review);
}

#[test]
fn mixed_catalog_inference_is_conservative_and_never_overrides_an_explicit_movie_root() {
    for filename in ["ABC-123.mkv", "abc_123-CD1.mkv", "FC2-PPV-1234567.mp4"] {
        let parsed = parse_filename(filename, None);
        assert_eq!(parsed.kind, MediaKind::AdultVideo, "{filename}");
        assert!(parsed.needs_review);
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning == "mixed_adult_inferred"));
        assert_eq!(
            parse_filename(filename, Some(MediaKind::Movie)).kind,
            MediaKind::Movie
        );
    }
    for filename in [
        "Movie 2024.mkv",
        "Movie-2024.mkv",
        "Unknown.mkv",
        "Fixture ABC-123.mkv",
        "Part-02.mkv",
    ] {
        let parsed = parse_filename(filename, None);
        assert_ne!(parsed.kind, MediaKind::AdultVideo, "{filename}");
        assert!(parsed.needs_review);
    }
}

#[test]
fn safe_nfo_reads_entities_as_text_but_rejects_external_entities_and_large_input() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("fixture.nfo");
    fs::write(&file,"<movie><title>Fixture &amp; film</title><year>2024</year><genre>Drama</genre><actor><name>Fixture Person</name></actor><uniqueid type=\"tmdb\">123</uniqueid><future>ignored</future></movie>").unwrap();
    let result = read_nfo(&file).unwrap();
    assert_eq!(result.fields["title"], "Fixture & film");
    assert_eq!(result.fields["year"], 2024);
    assert_eq!(result.fields["people"][0], "Fixture Person");
    assert_eq!(result.external_ids[0].provider_id, "tmdb");
    fs::write(&file,"<!DOCTYPE movie [<!ENTITY secret SYSTEM 'file:///etc/passwd'>]><movie><title>&secret;</title></movie>").unwrap();
    assert_eq!(read_nfo(&file), Err(ScanError::UnsafeXml));
    fs::write(&file, vec![b'x'; 2 * 1024 * 1024 + 1]).unwrap();
    assert_eq!(read_nfo(&file), Err(ScanError::MetadataTooLarge));
    fs::write(&file, "<movie><title>unterminated").unwrap();
    assert_eq!(read_nfo(&file), Err(ScanError::InvalidMetadata));
}

#[test]
fn sidecars_local_priority_and_incremental_signatures_are_read_only() {
    let dir = tempdir().unwrap();
    let video = dir.path().join("Fixture (2020).mkv");
    fs::write(&video, b"fixture-video").unwrap();
    fs::write(
        dir.path().join("movie.nfo"),
        "<movie><title>Directory title</title></movie>",
    )
    .unwrap();
    fs::write(
        dir.path().join("Fixture (2020).nfo"),
        "<movie><title>Specific title</title><year>2025</year></movie>",
    )
    .unwrap();
    fs::write(
        dir.path().join("Fixture (2020).en.srt"),
        b"fixture subtitles",
    )
    .unwrap();
    fs::write(
        dir.path().join("poster.jpg"),
        b"fixture-only-not-a-real-image",
    )
    .unwrap();
    let mut opts = options(Some(MediaKind::Movie));
    let entries = collect(dir.path(), &opts);
    assert_eq!(entries.len(), 1);
    let first = &entries[0];
    assert_eq!(first.parsed.title, "Specific title");
    assert_eq!(first.parsed.year, Some(2025));
    assert_eq!(first.artworks.len(), 1);
    assert_eq!(first.associated_files.len(), 4);
    assert_eq!(
        sidecar_fingerprint(dir.path(), first).unwrap(),
        first.sidecar_fingerprint
    );
    opts.known_files.insert(
        first.normalized_relative_path.clone(),
        FileStamp {
            size_bytes: first.size_bytes,
            modified_at: first.modified_at,
        },
    );
    assert!(collect(dir.path(), &opts)[0].unchanged);
    fs::write(dir.path().join("Fixture (2020).fr.srt"), b"new subtitles").unwrap();
    assert_ne!(
        sidecar_fingerprint(dir.path(), first).unwrap(),
        first.sidecar_fingerprint
    );
    assert_eq!(fs::read(video).unwrap(), b"fixture-video");
}

#[test]
fn exclusions_apply_to_media_directories_and_sidecar_reads() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("Excluded")).unwrap();
    fs::write(dir.path().join("Excluded/Hidden.mkv"), b"fixture").unwrap();
    fs::write(dir.path().join("Visible (2024).mkv"), b"fixture").unwrap();
    fs::write(
        dir.path().join("Visible (2024).nfo"),
        "<movie><title>Must not be read</title></movie>",
    )
    .unwrap();
    let mut opts = options(Some(MediaKind::Movie));
    opts.exclude_patterns = vec!["Excluded".into(), "*.nfo".into()];
    let entries = collect(dir.path(), &opts);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].parsed.title, "Visible");
    assert!(entries[0].associated_files.is_empty());
    assert!(entries[0].local_metadata.fields.is_empty());
    assert_eq!(
        sidecar_fingerprint(dir.path(), &entries[0]).unwrap(),
        entries[0].sidecar_fingerprint
    );
}

#[test]
fn callback_and_atomic_cancellation_stop_without_collecting_remaining_candidates() {
    let dir = tempdir().unwrap();
    for number in 0..100 {
        fs::write(dir.path().join(format!("Fixture {number}.mkv")), []).unwrap();
    }
    let mut count = 0;
    let result = run_scan(
        dir.path(),
        &options(Some(MediaKind::Movie)),
        &AtomicBool::new(false),
        |_| {
            count += 1;
            count < 3
        },
        |_| {},
    )
    .unwrap();
    assert!(result.cancelled);
    assert_eq!(count, 3);
    let result = run_scan(
        dir.path(),
        &options(Some(MediaKind::Movie)),
        &AtomicBool::new(true),
        |_| panic!("cancelled scan emitted candidate"),
        |_| {},
    )
    .unwrap();
    assert!(result.cancelled);
    assert_eq!(result.candidate_count, 0);
}

#[test]
fn ten_thousand_same_directory_files_stream_with_linear_sidecar_discovery() {
    let dir = tempdir().unwrap();
    for number in 0..10_000 {
        fs::write(dir.path().join(format!("Fixture {number:05}.mkv")), []).unwrap();
        fs::write(
            dir.path().join(format!("Fixture {number:05}.nfo")),
            "<movie><title>Fixture only</title></movie>",
        )
        .unwrap();
    }
    let mut count = 0;
    let started = Instant::now();
    let stats = run_scan(
        dir.path(),
        &options(Some(MediaKind::Movie)),
        &AtomicBool::new(false),
        |entry| {
            assert_eq!(entry.associated_files.len(), 1);
            count += 1;
            true
        },
        |_| {},
    )
    .unwrap();
    println!(
        "10,000 video + 10,000 NFO fixture scan: {:?}",
        started.elapsed()
    );
    assert_eq!(count, 10_000);
    assert_eq!(stats.candidate_count, 10_000);
    assert_eq!(stats.visited_files, 20_000);
    assert!(!stats.cancelled);
    let cancel = AtomicBool::new(false);
    let mut count = 0;
    let stats = run_scan(
        dir.path(),
        &options(Some(MediaKind::Movie)),
        &cancel,
        |_| {
            count += 1;
            if count == 5 {
                cancel.store(true, Ordering::Relaxed);
            }
            true
        },
        |_| {},
    )
    .unwrap();
    assert!(stats.cancelled);
    assert_eq!(count, 5);
}

#[cfg(unix)]
#[test]
fn symbolic_links_never_import_media_or_metadata_outside_root() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("Private.mkv"), b"private").unwrap();
    fs::write(
        outside.path().join("Private.nfo"),
        "<movie><title>private</title></movie>",
    )
    .unwrap();
    symlink(
        outside.path().join("Private.mkv"),
        dir.path().join("Private.mkv"),
    )
    .unwrap();
    symlink(
        outside.path().join("Private.nfo"),
        dir.path().join("Visible.nfo"),
    )
    .unwrap();
    fs::write(dir.path().join("Visible.mkv"), b"fixture").unwrap();
    let entries = collect(dir.path(), &options(Some(MediaKind::Movie)));
    assert_eq!(entries.len(), 1);
    assert!(entries[0].associated_files.is_empty());
    assert_eq!(entries[0].parsed.title, "Visible");
}

#[test]
fn audio_info_tags_are_read_without_rewriting_the_fixture() {
    fn chunk(name: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut chunk = name.to_vec();
        chunk.extend((data.len() as u32).to_le_bytes());
        chunk.extend(data);
        if data.len() % 2 == 1 {
            chunk.push(0);
        }
        chunk
    }
    let dir = tempdir().unwrap();
    let path = dir.path().join("Fixture.wav");
    let mut fmt = Vec::new();
    fmt.extend(1u16.to_le_bytes());
    fmt.extend(1u16.to_le_bytes());
    fmt.extend(8000u32.to_le_bytes());
    fmt.extend(16000u32.to_le_bytes());
    fmt.extend(2u16.to_le_bytes());
    fmt.extend(16u16.to_le_bytes());
    let mut list = b"INFO".to_vec();
    list.extend(chunk(b"INAM", b"Fixture song\0"));
    list.extend(chunk(b"IART", b"Fixture artist\0"));
    let mut body = b"WAVE".to_vec();
    body.extend(chunk(b"fmt ", &fmt));
    body.extend(chunk(b"LIST", &list));
    body.extend(chunk(b"data", &[0, 0, 0, 0]));
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((body.len() as u32).to_le_bytes());
    bytes.extend(body);
    fs::write(&path, &bytes).unwrap();
    let tags = read_audio_tags(&path).unwrap();
    assert_eq!(tags.fields["title"], "Fixture song");
    assert_eq!(tags.fields["artist"], "Fixture artist");
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn third_party_nfo_identifier_spellings_are_normalized_without_producer_checks() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("external.nfo");
    let xml = r#"<?xml version="1.0"?><movie><title>External producer</title><tmdbid>42</tmdbid><imdbid>tt0042</imdbid><tvdbid>99</tvdbid><uniqueid type="tmdb">42</uniqueid><generator>Some other tool</generator></movie>"#;
    fs::write(&path, xml).unwrap();
    let parsed = read_nfo(&path).unwrap();
    assert_eq!(parsed.fields["title"], "External producer");
    assert_eq!(parsed.external_ids.len(), 3);
    for (namespace, value) in [("tmdb", "42"), ("imdb", "tt0042"), ("tvdb", "99")] {
        assert!(parsed
            .external_ids
            .iter()
            .any(|id| id.namespace == namespace && id.value == value));
    }
    assert_eq!(fs::read_to_string(path).unwrap(), xml);
}

#[test]
fn show_sidecar_is_separate_from_episode_metadata_and_part_of_the_source_fingerprint() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("Show/Season 10")).unwrap();
    fs::write(
        dir.path().join("Show/Season 10/Filename.Show.S10E01.mkv"),
        b"video",
    )
    .unwrap();
    let show_nfo = dir.path().join("Show/tvshow.nfo");
    fs::write(&show_nfo, "<tvshow><title>External Show</title><year>2023</year><uniqueid type=\"tmdb\">show-id</uniqueid></tvshow>").unwrap();
    fs::write(dir.path().join("Show/Season 10/Filename.Show.S10E01.nfo"), "<episodedetails><title>Episode One</title><uniqueid type=\"tmdb\">episode-id</uniqueid></episodedetails>").unwrap();
    let entries = collect(dir.path(), &options(Some(MediaKind::TvSeries)));
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.parsed.title, "Episode One");
    assert_eq!(entry.parsed.series_title.as_deref(), Some("Filename Show"));
    assert_eq!(entry.local_metadata.external_ids[0].value, "episode-id");
    assert_eq!(
        entry.series_metadata.as_ref().unwrap().external_ids[0].value,
        "show-id"
    );
    assert_eq!(entry.series_nfo.as_deref(), Some("Show/tvshow.nfo"));
    assert_eq!(
        sidecar_fingerprint(dir.path(), entry).unwrap(),
        entry.sidecar_fingerprint
    );
    fs::write(
        &show_nfo,
        "<tvshow><title>Changed external Show title</title></tvshow>",
    )
    .unwrap();
    assert_ne!(
        sidecar_fingerprint(dir.path(), entry).unwrap(),
        entry.sidecar_fingerprint
    );
    let mut excluded = options(Some(MediaKind::TvSeries));
    excluded.exclude_patterns = vec!["**/tvshow.nfo".into()];
    assert!(collect(dir.path(), &excluded)[0].series_metadata.is_none());
}
