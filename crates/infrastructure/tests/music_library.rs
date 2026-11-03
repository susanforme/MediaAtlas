use chrono::Utc;
use mediaatlas_domain::*;
use mediaatlas_infrastructure::{
    library::{LibraryRepository, MetadataWriteMode, RepositoryError},
    music_library::{ensure_music_file, MusicLibraryService},
};
use uuid::Uuid;

fn graph() -> MusicReleaseGraph {
    let artist = MusicArtist {
        media_item_id: Uuid::new_v4(),
        name: "Artist".into(),
        sort_name: Some("Artist, The".into()),
        aliases: vec!["別名".into()],
    };
    let credit = ArtistCredit {
        artist_id: artist.media_item_id,
        credited_name: "Artist".into(),
        join_phrase: " feat. ".into(),
    };
    let group = MusicReleaseGroup {
        media_item_id: Uuid::new_v4(),
        title: "Album".into(),
        artist_credits: vec![credit.clone()],
        primary_type: Some("Album".into()),
        secondary_types: vec![],
        first_release_date: None,
    };
    let release = MusicRelease {
        media_item_id: Uuid::new_v4(),
        release_group_id: group.media_item_id,
        title: "Album".into(),
        artist_credits: vec![credit.clone()],
        country: Some("JP".into()),
        release_date: None,
        status: Some("Official".into()),
        labels: vec![],
    };
    let mut media = vec![];
    let mut tracks = vec![];
    let mut recordings = vec![];
    for disc in 1..=2 {
        let medium = MusicMedium {
            id: Uuid::new_v4(),
            release_id: release.media_item_id,
            position: disc,
            title: Some(format!("Disc {disc}")),
            format: Some("CD".into()),
        };
        let recording = MusicRecording {
            media_item_id: Uuid::new_v4(),
            title: format!("Recording {disc}"),
            length_ms: Some(1500),
            artist_credits: vec![credit.clone()],
        };
        let track = MusicTrack {
            media_item_id: Uuid::new_v4(),
            medium_id: medium.id,
            recording_id: recording.media_item_id,
            position: 1,
            number: "1".into(),
            title: format!("Track {disc}"),
            length_ms: Some(1500),
            artist_credits: vec![credit.clone()],
        };
        media.push(medium);
        tracks.push(track);
        recordings.push(recording);
    }
    let identifiers = vec![
        MusicIdentifier {
            media_item_id: release.media_item_id,
            kind: MusicIdentifierKind::MusicBrainz,
            namespace: "release".into(),
            value: release.media_item_id.to_string(),
        },
        MusicIdentifier {
            media_item_id: recordings[0].media_item_id,
            kind: MusicIdentifierKind::Isrc,
            namespace: "isrc".into(),
            value: "USABC9912345".into(),
        },
        MusicIdentifier {
            media_item_id: release.media_item_id,
            kind: MusicIdentifierKind::Ean,
            namespace: "barcode".into(),
            value: "1234567890123".into(),
        },
        MusicIdentifier {
            media_item_id: release.media_item_id,
            kind: MusicIdentifierKind::CatalogNumber,
            namespace: "label".into(),
            value: "ABC-123".into(),
        },
    ];
    MusicReleaseGraph {
        artists: vec![artist],
        release_group: group,
        release,
        media,
        tracks,
        recordings,
        identifiers,
        source_url: "https://musicbrainz.org/release/test".into(),
        fetched_at: Utc::now(),
        locale: Some("en-US".into()),
        release_date_text: Some("1999-02".into()),
        group_first_release_date_text: Some("1999".into()),
        release_disambiguation: Some("Deluxe remaster".into()),
        files: vec![],
        field_sources: vec![],
    }
}
async fn database() -> (tempfile::TempDir, LibraryRepository, MusicLibraryService) {
    let dir = tempfile::tempdir().unwrap();
    let repo = LibraryRepository::open(&dir.path().join("music.sqlite"))
        .await
        .unwrap();
    let music = MusicLibraryService::new(repo.clone());
    (dir, repo, music)
}
async fn local_file(repo: &LibraryRepository, dir: &tempfile::TempDir) -> MediaFile {
    let root = LibraryRoot {
        id: Uuid::new_v4(),
        name: "Audio".into(),
        path: dir.path().to_str().unwrap().into(),
        path_style: PathStyle::Posix,
        kind: Some(MediaKind::MusicTrack),
        enabled: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    repo.save_root(&root).await.unwrap();
    let item = MediaItem {
        id: Uuid::new_v4(),
        kind: MediaKind::MusicTrack,
        title: "Local track".into(),
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
        relative_path: "song.flac".into(),
        size_bytes: 0,
        modified_at: None,
        part: None,
    };
    repo.save_file(&file).await.unwrap();
    file
}
#[tokio::test]
async fn graph_roundtrip_has_distinct_discs_recordings_aliases_credits_and_identifiers() {
    let (_dir, repo, music) = database().await;
    let graph = graph();
    let imported = music.import_release(graph.clone()).await.unwrap();
    assert_eq!(imported.media, graph.media);
    assert_eq!(imported.tracks, graph.tracks);
    assert_eq!(imported.artists, graph.artists);
    assert_eq!(imported.release_date_text.as_deref(), Some("1999-02"));
    assert_eq!(
        imported.release_disambiguation.as_deref(),
        Some("Deluxe remaster")
    );
    assert_eq!(music.list_tools().await.unwrap()[0].track_count, 2);
    assert_eq!(
        music
            .artist_tools(graph.artists[0].media_item_id)
            .await
            .unwrap()
            .releases
            .len(),
        1
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM music_artist_aliases")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM music_artist_credits")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(count, 6);
    assert_eq!(imported.identifiers.len(), 4);
    assert!(imported
        .field_sources
        .iter()
        .any(|f| f.media_item_id == graph.release.media_item_id
            && f.field_key == "country"
            && f.provider_id.as_deref() == Some("musicbrainz")
            && f.source_url.as_deref() == Some(graph.source_url.as_str())));
    assert!(imported
        .field_sources
        .iter()
        .any(|f| f.field_key == "release_date_text" && f.value == "1999-02"));
}
#[tokio::test]
async fn equal_titles_group_and_barcode_never_merge_releases_or_rebind_file() {
    let (dir, repo, music) = database().await;
    let first = graph();
    let mut second = graph();
    second.artists = first.artists.clone();
    second.release_group = first.release_group.clone();
    second.release.release_group_id = first.release_group.media_item_id;
    second.release.country = Some("US".into());
    second.release.artist_credits[0].artist_id = first.artists[0].media_item_id;
    for recording in &mut second.recordings {
        recording.artist_credits[0].artist_id = first.artists[0].media_item_id;
    }
    for track in &mut second.tracks {
        track.artist_credits[0].artist_id = first.artists[0].media_item_id;
    }
    music.import_release(first.clone()).await.unwrap();
    music.import_release(second.clone()).await.unwrap();
    assert_eq!(music.list_tools().await.unwrap().len(), 2);
    let file = local_file(&repo, &dir).await;
    music
        .link_track(first.tracks[0].media_item_id, file.id)
        .await
        .unwrap();
    music
        .link_track(first.tracks[0].media_item_id, file.id)
        .await
        .unwrap();
    assert_eq!(
        music
            .link_track(second.tracks[0].media_item_id, file.id)
            .await,
        Err(RepositoryError::Conflict)
    );
    assert_eq!(
        music
            .album_tools(first.release.media_item_id)
            .await
            .unwrap()
            .files[0]
            .file_id,
        file.id
    );
}
#[tokio::test]
async fn protected_local_fields_override_refresh_and_all_partial_dates_survive() {
    let (_dir, repo, music) = database().await;
    let mut graph = graph();
    music.import_release(graph.clone()).await.unwrap();
    let field = MetadataValue {
        id: Uuid::new_v4(),
        media_item_id: graph.release.media_item_id,
        field_key: "title".into(),
        locale: Some("zh-CN".into()),
        value: serde_json::json!("我的专辑"),
        provider_id: None,
        source_url: None,
        fetched_at: None,
        confidence: Some(1.0),
        is_user_modified: true,
        is_locked: true,
    };
    repo.save_metadata_value(&field, MetadataWriteMode::User)
        .await
        .unwrap();
    for date in ["1999", "1999-02", "1999-02-23"] {
        graph.release_date_text = Some(date.into());
        let loaded = music.import_release(graph.clone()).await.unwrap();
        assert_eq!(loaded.release.title, "我的专辑");
        assert_eq!(loaded.release_date_text.as_deref(), Some(date));
    }
    assert!(repo
        .list_metadata_values(graph.release.media_item_id)
        .await
        .unwrap()
        .iter()
        .any(|m| m.value == "我的专辑" && m.is_locked));
}
#[tokio::test]
async fn invalid_graph_and_identity_collisions_are_atomic() {
    let (_dir, repo, music) = database().await;
    let graph = graph();
    let mut invalid = graph.clone();
    invalid.tracks[1].medium_id = Uuid::new_v4();
    assert_eq!(
        music.import_release(invalid).await,
        Err(RepositoryError::InvalidInput)
    );
    assert!(music.list_tools().await.unwrap().is_empty());
    music.import_release(graph.clone()).await.unwrap();
    let mut other = graph.clone();
    other.release.media_item_id = Uuid::new_v4();
    for medium in &mut other.media {
        medium.release_id = other.release.media_item_id;
    }
    for identifier in &mut other.identifiers {
        if identifier.media_item_id == graph.release.media_item_id {
            identifier.media_item_id = other.release.media_item_id;
        }
    }
    assert_eq!(
        music.import_release(other).await,
        Err(RepositoryError::Conflict)
    );
    assert_eq!(music.list_tools().await.unwrap().len(), 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM music_releases")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
}
#[tokio::test]
async fn linked_files_recheck_adult_root_and_persistent_isolation_after_reclassification() {
    let (dir, repo, music) = database().await;
    let graph = graph();
    music.import_release(graph.clone()).await.unwrap();
    let file = local_file(&repo, &dir).await;
    music
        .link_track(graph.tracks[0].media_item_id, file.id)
        .await
        .unwrap();
    assert_eq!(music.available_files().await.unwrap().len(), 1);
    sqlx::query("UPDATE library_roots SET kind='adult_video' WHERE id=?")
        .bind(file.root_id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(
        ensure_music_file(&repo, file.id).await,
        Err(RepositoryError::InvalidInput)
    );
    assert!(music.list_tools().await.unwrap().is_empty());
    assert!(music.available_files().await.unwrap().is_empty());
    assert_eq!(
        music.album_tools(graph.release.media_item_id).await,
        Err(RepositoryError::InvalidInput)
    );
    sqlx::query("UPDATE library_roots SET kind='music_track' WHERE id=?")
        .bind(file.root_id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
        .bind(file.media_item_id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(
        ensure_music_file(&repo, file.id).await,
        Err(RepositoryError::InvalidInput)
    );
    assert_eq!(
        music.import_release(graph).await,
        Err(RepositoryError::InvalidInput)
    );
}

#[tokio::test]
async fn protected_artist_title_and_entity_specific_fields_survive_refresh() {
    let (_dir, repo, music) = database().await;
    let graph = graph();
    music.import_release(graph.clone()).await.unwrap();
    for (id, key, value) in [
        (
            graph.artists[0].media_item_id,
            "title",
            serde_json::json!("Local artist"),
        ),
        (
            graph.release.media_item_id,
            "country",
            serde_json::json!("DE"),
        ),
        (
            graph.recordings[0].media_item_id,
            "length_ms",
            serde_json::json!(9999),
        ),
        (
            graph.release.media_item_id,
            "artist_credits",
            serde_json::json!([ArtistCredit {
                artist_id: graph.artists[0].media_item_id,
                credited_name: "Local credit".into(),
                join_phrase: "".into()
            }]),
        ),
    ] {
        repo.save_metadata_value(
            &MetadataValue {
                id: Uuid::new_v4(),
                media_item_id: id,
                field_key: key.into(),
                locale: Some("zh-CN".into()),
                value,
                provider_id: None,
                source_url: None,
                fetched_at: None,
                confidence: None,
                is_user_modified: true,
                is_locked: true,
            },
            MetadataWriteMode::User,
        )
        .await
        .unwrap();
    }
    let imported = music.import_release(graph.clone()).await.unwrap();
    assert_eq!(imported.artists[0].name, "Local artist");
    assert_eq!(
        repo.get_media(graph.artists[0].media_item_id)
            .await
            .unwrap()
            .unwrap()
            .title,
        "Local artist"
    );
    assert_eq!(imported.release.country.as_deref(), Some("DE"));
    assert_eq!(
        imported
            .recordings
            .iter()
            .find(|r| r.media_item_id == graph.recordings[0].media_item_id)
            .unwrap()
            .length_ms,
        Some(9999)
    );
    assert_eq!(
        imported.release.artist_credits[0].credited_name,
        "Local credit"
    );
}

#[tokio::test]
async fn release_mbid_cannot_be_imported_under_a_different_local_identity() {
    let (_dir, _repo, music) = database().await;
    let first = graph();
    music.import_release(first.clone()).await.unwrap();
    let mut second = graph();
    second.identifiers[0].value = first.identifiers[0].value.clone();
    assert_eq!(
        music.import_release(second).await,
        Err(RepositoryError::Conflict)
    );
    assert_eq!(music.list_tools().await.unwrap().len(), 1);
}
#[tokio::test]
async fn artist_hides_when_associated_album_is_reclassified_and_bad_payload_is_reported() {
    let (dir, repo, music) = database().await;
    let graph = graph();
    music.import_release(graph.clone()).await.unwrap();
    let file = local_file(&repo, &dir).await;
    music
        .link_track(graph.tracks[0].media_item_id, file.id)
        .await
        .unwrap();
    sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
        .bind(file.media_item_id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(
        music.artist_tools(graph.artists[0].media_item_id).await,
        Err(RepositoryError::InvalidInput)
    );
    sqlx::query("UPDATE media_items SET adult_isolated=0 WHERE id=?")
        .bind(file.media_item_id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE music_releases SET entity_json='{}' WHERE media_item_id=?")
        .bind(graph.release.media_item_id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(music.list_tools().await, Err(RepositoryError::CorruptData));
    assert_eq!(
        music.available_files().await,
        Err(RepositoryError::CorruptData)
    );
}
#[tokio::test]
async fn protected_release_date_and_disambiguation_keep_raw_and_typed_values_consistent() {
    let (_dir, repo, music) = database().await;
    let graph = graph();
    music.import_release(graph.clone()).await.unwrap();
    for (key, value) in [
        ("release_date", serde_json::json!("2001-02-03")),
        ("release_disambiguation", serde_json::json!("Local edition")),
    ] {
        repo.save_metadata_value(
            &MetadataValue {
                id: Uuid::new_v4(),
                media_item_id: graph.release.media_item_id,
                field_key: key.into(),
                locale: Some("zh-CN".into()),
                value,
                provider_id: None,
                source_url: None,
                fetched_at: None,
                confidence: None,
                is_user_modified: true,
                is_locked: true,
            },
            MetadataWriteMode::User,
        )
        .await
        .unwrap();
    }
    let loaded = music.import_release(graph).await.unwrap();
    assert_eq!(loaded.release_date_text.as_deref(), Some("2001-02-03"));
    assert_eq!(
        loaded.release.release_date.unwrap().to_string(),
        "2001-02-03"
    );
    assert_eq!(
        loaded.release_disambiguation.as_deref(),
        Some("Local edition")
    );
}

#[tokio::test]
async fn remote_saved_graphs_stay_in_tools_until_a_current_authorized_local_file_is_linked() {
    let (dir, repo, music) = database().await;
    let graph = graph();
    music.import_release(graph.clone()).await.unwrap();
    assert!(music.list().await.unwrap().is_empty());
    assert_eq!(music.list_tools().await.unwrap().len(), 1);
    assert_eq!(
        music.album(graph.release.media_item_id).await,
        Err(RepositoryError::NotFound)
    );
    assert_eq!(
        music.artist(graph.artists[0].media_item_id).await,
        Err(RepositoryError::NotFound)
    );
    let file = local_file(&repo, &dir).await;
    std::fs::write(dir.path().join("song.flac"), b"temporary fixture bytes").unwrap();
    music
        .link_track(graph.tracks[0].media_item_id, file.id)
        .await
        .unwrap();
    assert_eq!(music.list().await.unwrap().len(), 1);
    assert_eq!(
        music
            .album(graph.release.media_item_id)
            .await
            .unwrap()
            .files[0]
            .file_id,
        file.id
    );
    assert_eq!(
        music
            .artist(graph.artists[0].media_item_id)
            .await
            .unwrap()
            .releases
            .len(),
        1
    );
    sqlx::query("UPDATE library_roots SET enabled=0")
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(music.list().await.unwrap().is_empty());
}
