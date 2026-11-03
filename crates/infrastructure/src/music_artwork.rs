//! Explicit tool-side cover selection and network-free library artwork reads.
use crate::{
    library::{LibraryRepository, RepositoryError},
    media_output::{CachedArtwork, MediaOutputService},
    music_library::{ensure_graph_access, load_graph, MusicLibraryService},
    music_providers::MusicProviderHub,
    network::CancellationToken,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use mediaatlas_domain::{MusicIdentifierKind, MusicReleaseGraph};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

const COVER_FIELD: &str = "music_selected_cover";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicCoverPreview {
    pub thumbnail: String,
    pub source_url: String,
    pub license: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SelectedCover {
    release_id: Uuid,
    mbid: Uuid,
    image: CachedArtwork,
    source_url: String,
    license: String,
}
#[derive(Clone)]
pub struct MusicArtworkService {
    repo: LibraryRepository,
    hub: MusicProviderHub,
    output: MediaOutputService,
}
impl MusicArtworkService {
    pub fn new(repo: LibraryRepository, hub: MusicProviderHub, output: MediaOutputService) -> Self {
        Self { repo, hub, output }
    }
    /// Explicit action from Music Tools. The caller can select only an index of
    /// the stored concrete release's backend-fetched cover candidates.
    pub async fn select(
        &self,
        release_id: Uuid,
        mbid: &str,
        cover_index: usize,
        cancel: &CancellationToken,
    ) -> Result<MusicCoverPreview, RepositoryError> {
        let mbid = Uuid::parse_str(mbid).map_err(|_| RepositoryError::InvalidInput)?;
        let graph = MusicLibraryService::new(self.repo.clone())
            .album_tools(release_id)
            .await?;
        ensure_release_mbid(&graph, mbid)?;
        if cancel.is_cancelled() {
            return Err(RepositoryError::Provider(
                mediaatlas_domain::ProviderFailure::Cancelled,
            ));
        }
        let covers = self
            .hub
            .cover(&mbid.to_string(), cancel)
            .await
            .map_err(RepositoryError::Provider)?;
        let cover = covers
            .get(cover_index)
            .ok_or(RepositoryError::InvalidInput)?;
        let image = self
            .hub
            .cache_cover(&self.output, cover, cancel)
            .await
            .map_err(RepositoryError::Provider)?;
        let selected = SelectedCover {
            release_id,
            mbid,
            image,
            source_url: cover.source_url.clone(),
            license: cover.license.clone(),
        };
        self.commit_selection(selected, cancel).await
    }
    async fn commit_selection(
        &self,
        selected: SelectedCover,
        cancel: &CancellationToken,
    ) -> Result<MusicCoverPreview, RepositoryError> {
        let preview = self.render(&selected)?;
        let mut tx = self.repo.pool().begin().await?;
        // Reserve SQLite's writer before loading the current graph, closing the
        // isolation/reclassification race between provider fetch and selection.
        sqlx::query("UPDATE media_items SET updated_at=updated_at WHERE id=?")
            .bind(selected.release_id.to_string())
            .execute(&mut *tx)
            .await?;
        let graph = load_graph(&mut tx, selected.release_id).await?;
        ensure_graph_access(&mut tx, &graph).await?;
        ensure_release_mbid(&graph, selected.mbid)?;
        let locked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM metadata_values WHERE media_item_id=? AND field_key=? AND is_locked=1)").bind(selected.release_id.to_string()).bind(COVER_FIELD).fetch_one(&mut *tx).await?;
        if locked {
            return Err(RepositoryError::Conflict);
        }
        if cancel.is_cancelled() {
            return Err(RepositoryError::Provider(
                mediaatlas_domain::ProviderFailure::Cancelled,
            ));
        }
        let snapshot =
            serde_json::to_string(&selected).map_err(|_| RepositoryError::InvalidInput)?;
        let now = Utc::now().timestamp_millis();
        sqlx::query("INSERT INTO metadata_values(id,media_item_id,field_key,locale,value_json,provider_id,source_url,fetched_at,confidence,is_user_modified,is_locked) VALUES(?,?,?,'',?,'coverartarchive',?,?,1.0,1,0) ON CONFLICT(media_item_id,field_key,locale) DO UPDATE SET value_json=excluded.value_json,provider_id=excluded.provider_id,source_url=excluded.source_url,fetched_at=excluded.fetched_at,confidence=excluded.confidence,is_user_modified=1 WHERE metadata_values.is_locked=0")
            .bind(Uuid::new_v4().to_string()).bind(selected.release_id.to_string()).bind(COVER_FIELD).bind(snapshot).bind(&selected.source_url).bind(now).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM artworks WHERE media_item_id=? AND provider_id='coverartarchive' AND kind='album_cover'").bind(selected.release_id.to_string()).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO artworks(id,media_item_id,kind,cache_path,source_url,provider_id,width,height,content_hash,created_at) VALUES(?,?,'album_cover',?,?,'coverartarchive',?,?,?,?)")
            .bind(Uuid::new_v4().to_string()).bind(selected.release_id.to_string()).bind(&selected.image.cache_path).bind(&selected.source_url).bind(selected.image.width).bind(selected.image.height).bind(&selected.image.content_hash).bind(now).execute(&mut *tx).await?;
        ensure_graph_access(&mut tx, &graph).await?;
        if cancel.is_cancelled() {
            return Err(RepositoryError::Provider(
                mediaatlas_domain::ProviderFailure::Cancelled,
            ));
        }
        tx.commit().await?;
        Ok(preview)
    }
    /// Consumes saved results only. Provider enablement and network availability
    /// have no role in rendering an already selected library cover.
    pub async fn cached(
        &self,
        release_id: Uuid,
    ) -> Result<Option<MusicCoverPreview>, RepositoryError> {
        let mut tx = self.repo.pool().begin().await?;
        let graph = load_graph(&mut tx, release_id).await?;
        ensure_graph_access(&mut tx, &graph).await?;
        let row=sqlx::query("SELECT value_json,provider_id,source_url FROM metadata_values WHERE media_item_id=? AND field_key=? AND locale=''").bind(release_id.to_string()).bind(COVER_FIELD).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let value: String = row.try_get("value_json")?;
        if value.len() > 32768 {
            return Err(RepositoryError::CorruptData);
        }
        let selected: SelectedCover =
            serde_json::from_str(&value).map_err(|_| RepositoryError::CorruptData)?;
        if selected.release_id != release_id
            || row.try_get::<Option<String>, _>("provider_id")?.as_deref()
                != Some("coverartarchive")
            || row.try_get::<Option<String>, _>("source_url")?.as_deref()
                != Some(selected.source_url.as_str())
        {
            return Err(RepositoryError::CorruptData);
        }
        ensure_release_mbid(&graph, selected.mbid)?;
        let preview = self.render(&selected)?;
        ensure_graph_access(&mut tx, &graph).await?;
        tx.commit().await?;
        Ok(Some(preview))
    }
    fn render(&self, selected: &SelectedCover) -> Result<MusicCoverPreview, RepositoryError> {
        if selected.mbid.is_nil()
            || selected.source_url
                != format!("https://coverartarchive.org/release/{}", selected.mbid)
            || selected.license.is_empty()
            || selected.license.len() > 4096
        {
            return Err(RepositoryError::CorruptData);
        }
        let bytes = self.output.validated_cached_thumbnail(&selected.image)?;
        Ok(MusicCoverPreview {
            thumbnail: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
            source_url: selected.source_url.clone(),
            license: selected.license.clone(),
        })
    }
}
fn ensure_release_mbid(graph: &MusicReleaseGraph, mbid: Uuid) -> Result<(), RepositoryError> {
    if !mbid.is_nil()
        && graph.identifiers.iter().any(|id| {
            id.media_item_id == graph.release.media_item_id
                && id.kind == MusicIdentifierKind::MusicBrainz
                && id.namespace == "release"
                && Uuid::parse_str(&id.value).ok() == Some(mbid)
        })
    {
        Ok(())
    } else {
        Err(RepositoryError::InvalidInput)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::NetworkEngine;
    use mediaatlas_domain::*;
    use sha2::{Digest, Sha256};
    use std::{io::Cursor, sync::Arc};
    async fn setup() -> (
        tempfile::TempDir,
        LibraryRepository,
        MusicArtworkService,
        MusicReleaseGraph,
        SelectedCover,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("art.sqlite"))
            .await
            .unwrap();
        let artist = MusicArtist {
            media_item_id: Uuid::new_v4(),
            name: "Synthetic artist".into(),
            sort_name: None,
            aliases: vec![],
        };
        let credit = ArtistCredit {
            artist_id: artist.media_item_id,
            credited_name: artist.name.clone(),
            join_phrase: String::new(),
        };
        let group = MusicReleaseGroup {
            media_item_id: Uuid::new_v4(),
            title: "Synthetic album".into(),
            artist_credits: vec![credit.clone()],
            primary_type: Some("Album".into()),
            secondary_types: vec![],
            first_release_date: None,
        };
        let release = MusicRelease {
            media_item_id: Uuid::new_v4(),
            release_group_id: group.media_item_id,
            title: group.title.clone(),
            artist_credits: vec![credit],
            country: None,
            release_date: None,
            status: None,
            labels: vec![],
        };
        let release_id = release.media_item_id;
        let graph = MusicReleaseGraph {
            artists: vec![artist],
            release_group: group,
            release,
            media: vec![],
            tracks: vec![],
            recordings: vec![],
            identifiers: vec![MusicIdentifier {
                media_item_id: release_id,
                kind: MusicIdentifierKind::MusicBrainz,
                namespace: "release".into(),
                value: release_id.to_string(),
            }],
            source_url: format!("https://musicbrainz.org/release/{release_id}"),
            fetched_at: Utc::now(),
            locale: None,
            release_date_text: None,
            release_disambiguation: None,
            group_first_release_date_text: None,
            files: vec![],
            field_sources: vec![],
        };
        MusicLibraryService::new(repo.clone())
            .import_release(graph.clone())
            .await
            .unwrap();
        // Only generated pixels in a temporary backend cache. No source audio or
        // user files, and no Provider call is required to consume a saved result.
        let cache = dir.path().join("cache");
        std::fs::create_dir(&cache).unwrap();
        let mut data = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(4, 4)
            .write_to(&mut data, image::ImageFormat::Png)
            .unwrap();
        let bytes = data.into_inner();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let source = cache.join(format!("{hash}.png"));
        let thumb = cache.join(format!("{hash}.thumb.png"));
        std::fs::write(&source, &bytes).unwrap();
        std::fs::write(&thumb, &bytes).unwrap();
        let image = CachedArtwork {
            cache_path: source.to_str().unwrap().into(),
            thumbnail_path: thumb.to_str().unwrap().into(),
            content_hash: hash,
            width: 4,
            height: 4,
            mime: "image/png".into(),
        };
        let selected = SelectedCover {
            release_id,
            mbid: release_id,
            image,
            source_url: format!("https://coverartarchive.org/release/{release_id}"),
            license: "Individual image copyright retained; no blanket reuse license".into(),
        };
        let hub = MusicProviderHub::new(Arc::new(NetworkEngine::new(4).unwrap()));
        let output = MediaOutputService::new(repo.clone(), cache);
        let service = MusicArtworkService::new(repo.clone(), hub, output);
        (dir, repo, service, graph, selected)
    }
    #[tokio::test]
    async fn selected_artwork_and_rights_survive_recreation_with_network_free_library_reads() {
        let (dir, repo, service, graph, selected) = setup().await;
        let release_id = graph.release.media_item_id;
        assert_eq!(service.cached(release_id).await.unwrap(), None);
        let expected = service
            .commit_selection(selected, &CancellationToken::new())
            .await
            .unwrap();
        assert!(expected.thumbnail.starts_with("data:image/png;base64,"));
        let public_graph = MusicLibraryService::new(repo.clone())
            .album_tools(release_id)
            .await
            .unwrap();
        let provenance = public_graph
            .field_sources
            .iter()
            .find(|source| source.field_key == COVER_FIELD)
            .unwrap();
        assert_eq!(provenance.value["license"], expected.license);
        assert!(provenance.fetched_at.is_some());
        assert!(provenance.value.get("image").is_none());
        assert!(!serde_json::to_string(&public_graph)
            .unwrap()
            .contains("cache_path"));
        let artworks = repo.list_artworks(release_id).await.unwrap();
        assert_eq!(artworks.len(), 1);
        assert_eq!(artworks[0].kind, ArtworkKind::AlbumCover);
        assert_eq!(artworks[0].provider_id.as_deref(), Some("coverartarchive"));
        let hub = MusicProviderHub::new(Arc::new(NetworkEngine::new(4).unwrap()));
        let service = MusicArtworkService::new(
            repo,
            hub,
            MediaOutputService::new(service.repo.clone(), dir.path().join("cache")),
        );
        assert_eq!(service.cached(release_id).await.unwrap(), Some(expected));
    }
    #[tokio::test]
    async fn graph_isolation_stale_mbid_and_cancellation_never_commit_a_selection() {
        let (_dir, repo, service, graph, selected) = setup().await;
        let id = graph.release.media_item_id;
        let mut wrong = selected.clone();
        wrong.mbid = Uuid::new_v4();
        wrong.source_url = format!("https://coverartarchive.org/release/{}", wrong.mbid);
        assert!(service
            .commit_selection(wrong, &CancellationToken::new())
            .await
            .is_err());
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(service
            .commit_selection(selected.clone(), &cancel)
            .await
            .is_err());
        assert_eq!(service.cached(id).await.unwrap(), None);
        sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
            .bind(graph.artists[0].media_item_id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        assert!(service
            .commit_selection(selected, &CancellationToken::new())
            .await
            .is_err());
        assert!(service.cached(id).await.is_err());
        assert!(repo.list_artworks(id).await.unwrap().is_empty());
    }
    #[tokio::test]
    async fn corrupt_cache_snapshot_and_locked_selection_fail_closed() {
        let (_dir, repo, service, graph, selected) = setup().await;
        let id = graph.release.media_item_id;
        service
            .commit_selection(selected.clone(), &CancellationToken::new())
            .await
            .unwrap();
        sqlx::query("UPDATE metadata_values SET is_locked=1 WHERE media_item_id=? AND field_key=?")
            .bind(id.to_string())
            .bind(COVER_FIELD)
            .execute(repo.pool())
            .await
            .unwrap();
        assert!(service
            .commit_selection(selected.clone(), &CancellationToken::new())
            .await
            .is_err());
        std::fs::write(&selected.image.cache_path, b"changed cached bytes").unwrap();
        assert!(service.cached(id).await.is_err());
        sqlx::query(
            "UPDATE metadata_values SET value_json='{}' WHERE media_item_id=? AND field_key=?",
        )
        .bind(id.to_string())
        .bind(COVER_FIELD)
        .execute(repo.pool())
        .await
        .unwrap();
        assert_eq!(service.cached(id).await, Err(RepositoryError::CorruptData));
    }
}
