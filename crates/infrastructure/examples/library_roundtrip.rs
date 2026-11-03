//! A disposable Fixture demonstration; never opens or scans a user's media library.

use chrono::Utc;
use mediaatlas_domain::{LibraryRoot, MediaFile, MediaItem, MediaKind, MetadataValue, PathStyle};
use mediaatlas_infrastructure::library::{
    LibraryRepository, ListMediaQuery, MetadataWriteMode, RepositoryError,
};
use serde_json::json;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("fixture.sqlite");
    let repo = LibraryRepository::open(&database).await?;
    let now = Utc::now();
    let root = LibraryRoot {
        id: Uuid::new_v4(),
        name: "Fixture root (disposable)".into(),
        path: directory
            .path()
            .to_str()
            .ok_or("Non-UTF8 fixture directory")?
            .to_owned(),
        path_style: if cfg!(windows) {
            PathStyle::Windows
        } else {
            PathStyle::Posix
        },
        kind: Some(MediaKind::Movie),
        enabled: true,
        created_at: now,
        updated_at: now,
    };
    repo.save_root(&root).await?;
    let item = MediaItem {
        id: Uuid::new_v4(),
        kind: MediaKind::Movie,
        title: "Fixture movie".into(),
        original_title: None,
        year: Some(2026),
        created_at: now,
        updated_at: now,
    };
    repo.save_media(&item).await?;
    repo.save_file(&MediaFile {
        id: Uuid::new_v4(),
        media_item_id: item.id,
        root_id: root.id,
        relative_path: "Fixture/movie.mkv".into(),
        size_bytes: 0,
        modified_at: None,
        part: None,
    })
    .await?;
    let mut field = MetadataValue {
        id: Uuid::new_v4(),
        media_item_id: item.id,
        field_key: "title".into(),
        locale: Some("en".into()),
        value: json!("User-selected fixture title"),
        provider_id: None,
        source_url: None,
        fetched_at: None,
        confidence: None,
        is_user_modified: true,
        is_locked: true,
    };
    repo.save_metadata_value(&field, MetadataWriteMode::User)
        .await?;
    field.value = json!("Provider fixture proposal");
    field.is_user_modified = false;
    field.is_locked = false;
    assert_eq!(
        repo.save_metadata_value(&field, MetadataWriteMode::Provider)
            .await,
        Err(RepositoryError::ProtectedMetadata)
    );
    repo.pool().close().await;
    let reopened = LibraryRepository::open(&database).await?;
    let page = reopened
        .list_media(&ListMediaQuery {
            root_id: Some(root.id),
            kind: Some(MediaKind::Movie),
            ..Default::default()
        })
        .await?;
    assert_eq!(page.items.len(), 1);
    assert!(reopened.list_metadata_values(item.id).await?[0].is_locked);
    println!("Fixture roundtrip passed: root, media, file, pagination, locked metadata and database reopen. No source media files were created or changed.");
    reopened.pool().close().await;
    Ok(())
}
