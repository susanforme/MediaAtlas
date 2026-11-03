//! Transactional persistence and ordinary-lane access for the independent music graph.
use crate::library::{LibraryRepository, RepositoryError};
use chrono::DateTime;
use mediaatlas_domain::{
    normalize_locale, ArtistCredit, LibraryRoot, MediaFile, MediaKind, MetadataValue,
    MusicAlbumView, MusicArtist, MusicArtistView, MusicIdentifier, MusicIdentifierKind,
    MusicLocalFile, MusicMedium, MusicRecording, MusicRelease, MusicReleaseGraph,
    MusicReleaseGroup, MusicTrack, MusicTrackFile,
};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{Row, Sqlite, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone)]
pub struct MusicLibraryService {
    repo: LibraryRepository,
}
impl MusicLibraryService {
    pub fn new(repo: LibraryRepository) -> Self {
        Self { repo }
    }

    pub async fn list(&self) -> Result<Vec<MusicAlbumView>, RepositoryError> {
        let ids: Vec<String> =
            sqlx::query_scalar("SELECT media_item_id FROM music_releases ORDER BY media_item_id")
                .fetch_all(self.repo.pool())
                .await?;
        let mut albums = Vec::new();
        for id in ids {
            let id = uuid(&id)?;
            match self.album(id).await {
                Ok(graph) => albums.push(album_view(&graph)),
                Err(RepositoryError::InvalidInput | RepositoryError::NotFound) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(albums)
    }
    pub async fn album(&self, release_id: Uuid) -> Result<MusicReleaseGraph, RepositoryError> {
        let mut tx = self.repo.pool().begin().await?;
        let graph = load_graph(&mut tx, release_id).await?;
        ensure_graph_access(&mut tx, &graph).await?;
        tx.commit().await?;
        Ok(graph)
    }
    pub async fn artist(&self, artist_id: Uuid) -> Result<MusicArtistView, RepositoryError> {
        let mut tx = self.repo.pool().begin().await?;
        ensure_item(&mut tx, artist_id, MediaKind::MusicArtist).await?;
        let artist = load_entity(&mut tx, "music_artists", artist_id).await?;
        let associated:Vec<String>=sqlx::query_scalar("SELECT DISTINCT m.release_id FROM music_artist_credits c JOIN music_tracks t ON t.media_item_id=c.media_item_id OR t.recording_id=c.media_item_id JOIN music_media m ON m.id=t.medium_id WHERE c.artist_id=? UNION SELECT DISTINCT r.media_item_id FROM music_artist_credits c JOIN music_releases r ON r.media_item_id=c.media_item_id OR r.release_group_id=c.media_item_id WHERE c.artist_id=?").bind(artist_id.to_string()).bind(artist_id.to_string()).fetch_all(&mut *tx).await?;
        for release_id in associated {
            let graph = load_graph(&mut tx, uuid(&release_id)?).await?;
            ensure_graph_access(&mut tx, &graph).await?;
        }
        tx.commit().await?;
        let mut releases = Vec::new();
        for album in self.list().await? {
            let graph = self.album(album.release_id).await?;
            if graph
                .artists
                .iter()
                .any(|artist| artist.media_item_id == artist_id)
            {
                releases.push(album);
            }
        }
        Ok(MusicArtistView { artist, releases })
    }
    pub async fn import_release(
        &self,
        mut graph: MusicReleaseGraph,
    ) -> Result<MusicReleaseGraph, RepositoryError> {
        validate_graph(&graph)?;
        graph.locale =
            normalize_locale(graph.locale.as_deref()).map_err(|_| RepositoryError::InvalidInput)?;
        let mut tx = self.repo.pool().begin().await?;
        // Acquire SQLite's writer reservation before checking existing isolation.
        sqlx::query("UPDATE media_items SET updated_at=updated_at WHERE id=?")
            .bind(graph.release.media_item_id.to_string())
            .execute(&mut *tx)
            .await?;
        for artist in &mut graph.artists {
            protect(
                &mut tx,
                artist.media_item_id,
                artist,
                graph.locale.as_deref(),
            )
            .await?;
        }
        protect(
            &mut tx,
            graph.release_group.media_item_id,
            &mut graph.release_group,
            graph.locale.as_deref(),
        )
        .await?;
        protect(
            &mut tx,
            graph.release.media_item_id,
            &mut graph.release,
            graph.locale.as_deref(),
        )
        .await?;
        for recording in &mut graph.recordings {
            protect(
                &mut tx,
                recording.media_item_id,
                recording,
                graph.locale.as_deref(),
            )
            .await?;
        }
        for track in &mut graph.tracks {
            protect(&mut tx, track.media_item_id, track, graph.locale.as_deref()).await?;
        }
        protect_envelope(&mut tx, &mut graph).await?;
        validate_graph(&graph)?;
        if let Some(previous) = sqlx::query_scalar::<_, String>(
            "SELECT media_item_id FROM music_releases WHERE media_item_id=?",
        )
        .bind(graph.release.media_item_id.to_string())
        .fetch_optional(&mut *tx)
        .await?
        {
            let old = load_graph(&mut tx, uuid(&previous)?).await?;
            ensure_graph_access(&mut tx, &old).await?;
            let before: HashSet<_> = old.tracks.iter().map(|t| t.media_item_id).collect();
            let after: HashSet<_> = graph.tracks.iter().map(|t| t.media_item_id).collect();
            if before != after {
                return Err(RepositoryError::Conflict);
            }
        }
        for artist in &graph.artists {
            save_item(
                &mut tx,
                artist.media_item_id,
                MediaKind::MusicArtist,
                &artist.name,
                &graph,
            )
            .await?;
            save_entity(&mut tx, "music_artists", artist.media_item_id, artist).await?;
            sqlx::query("DELETE FROM music_artist_aliases WHERE artist_id=?")
                .bind(artist.media_item_id.to_string())
                .execute(&mut *tx)
                .await?;
            for (position, alias) in artist.aliases.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO music_artist_aliases(artist_id,position,alias) VALUES(?,?,?)",
                )
                .bind(artist.media_item_id.to_string())
                .bind(position as i64)
                .bind(alias)
                .execute(&mut *tx)
                .await?;
            }
        }
        let group = &graph.release_group;
        save_item(
            &mut tx,
            group.media_item_id,
            MediaKind::MusicReleaseGroup,
            &group.title,
            &graph,
        )
        .await?;
        save_entity(&mut tx, "music_release_groups", group.media_item_id, group).await?;
        let release = &graph.release;
        save_item(
            &mut tx,
            release.media_item_id,
            MediaKind::MusicRelease,
            &release.title,
            &graph,
        )
        .await?;
        let affected = sqlx::query("INSERT INTO music_releases(media_item_id,release_group_id,entity_json,source_url,fetched_at,locale,release_date_text,group_first_release_date_text,release_disambiguation) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(media_item_id) DO UPDATE SET entity_json=excluded.entity_json,source_url=excluded.source_url,fetched_at=excluded.fetched_at,locale=excluded.locale,release_date_text=excluded.release_date_text,group_first_release_date_text=excluded.group_first_release_date_text,release_disambiguation=excluded.release_disambiguation WHERE music_releases.release_group_id=excluded.release_group_id")
            .bind(release.media_item_id.to_string()).bind(release.release_group_id.to_string()).bind(json(release)?).bind(&graph.source_url).bind(graph.fetched_at.timestamp_millis()).bind(&graph.locale).bind(&graph.release_date_text).bind(&graph.group_first_release_date_text).bind(&graph.release_disambiguation).execute(&mut *tx).await?.rows_affected();
        if affected != 1 {
            return Err(RepositoryError::Conflict);
        }
        for medium in &graph.media {
            let affected=sqlx::query("INSERT INTO music_media(id,release_id,position,entity_json) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET position=excluded.position,entity_json=excluded.entity_json WHERE music_media.release_id=excluded.release_id")
                .bind(medium.id.to_string()).bind(medium.release_id.to_string()).bind(medium.position as i64).bind(json(medium)?).execute(&mut *tx).await?.rows_affected();
            if affected != 1 {
                return Err(RepositoryError::Conflict);
            }
        }
        for recording in &graph.recordings {
            save_item(
                &mut tx,
                recording.media_item_id,
                MediaKind::MusicRecording,
                &recording.title,
                &graph,
            )
            .await?;
            save_entity(
                &mut tx,
                "music_recordings",
                recording.media_item_id,
                recording,
            )
            .await?;
        }
        for track in &graph.tracks {
            save_item(
                &mut tx,
                track.media_item_id,
                MediaKind::MusicTrack,
                &track.title,
                &graph,
            )
            .await?;
            let affected=sqlx::query("INSERT INTO music_tracks(media_item_id,medium_id,recording_id,position,entity_json) VALUES(?,?,?,?,?) ON CONFLICT(media_item_id) DO UPDATE SET position=excluded.position,entity_json=excluded.entity_json WHERE music_tracks.medium_id=excluded.medium_id AND music_tracks.recording_id=excluded.recording_id")
                .bind(track.media_item_id.to_string()).bind(track.medium_id.to_string()).bind(track.recording_id.to_string()).bind(track.position as i64).bind(json(track)?).execute(&mut *tx).await?.rows_affected();
            if affected != 1 {
                return Err(RepositoryError::Conflict);
            }
        }
        save_credits(&mut tx, group.media_item_id, &group.artist_credits).await?;
        save_credits(&mut tx, release.media_item_id, &release.artist_credits).await?;
        for recording in &graph.recordings {
            save_credits(&mut tx, recording.media_item_id, &recording.artist_credits).await?;
        }
        for track in &graph.tracks {
            save_credits(&mut tx, track.media_item_id, &track.artist_credits).await?;
        }
        for identifier in &graph.identifiers {
            sqlx::query("INSERT OR IGNORE INTO music_identifiers(media_item_id,kind,namespace,value) VALUES(?,?,?,?)").bind(identifier.media_item_id.to_string()).bind(identifier_kind(identifier.kind)).bind(&identifier.namespace).bind(&identifier.value).execute(&mut *tx).await?;
            // OR IGNORE must never silently redirect a conflicting MBID.
            if identifier.kind == MusicIdentifierKind::MusicBrainz {
                let owner:String=sqlx::query_scalar("SELECT media_item_id FROM music_identifiers WHERE kind='music_brainz' AND namespace=? AND value=?").bind(&identifier.namespace).bind(&identifier.value).fetch_one(&mut *tx).await?;
                if owner != identifier.media_item_id.to_string() {
                    return Err(RepositoryError::Conflict);
                }
            }
        }
        for artist in &graph.artists {
            save_sources(&mut tx, artist.media_item_id, artist, &graph).await?;
        }
        save_sources(&mut tx, group.media_item_id, group, &graph).await?;
        save_sources(&mut tx, release.media_item_id, release, &graph).await?;
        for track in &graph.tracks {
            save_sources(&mut tx, track.media_item_id, track, &graph).await?;
        }
        for recording in &graph.recordings {
            save_sources(&mut tx, recording.media_item_id, recording, &graph).await?;
        }
        save_sources(&mut tx,release.media_item_id,&serde_json::json!({"release_date_text":graph.release_date_text,"release_disambiguation":graph.release_disambiguation}),&graph).await?;
        save_sources(&mut tx,group.media_item_id,&serde_json::json!({"group_first_release_date_text":graph.group_first_release_date_text}),&graph).await?;
        let graph = load_graph(&mut tx, release.media_item_id).await?;
        ensure_graph_access(&mut tx, &graph).await?;
        tx.commit().await?;
        Ok(graph)
    }
    pub async fn available_files(&self) -> Result<Vec<MusicLocalFile>, RepositoryError> {
        let rows=sqlx::query("SELECT f.id,f.media_item_id,f.relative_path FROM media_files f JOIN media_items m ON m.id=f.media_item_id WHERE m.kind IN('music_track','music_recording','music_release') ORDER BY f.relative_path,f.id").fetch_all(self.repo.pool()).await?;
        let mut files = Vec::new();
        for row in rows {
            let file_id = uuid(row.try_get("id")?)?;
            match ensure_music_file(&self.repo, file_id).await {
                Ok(_) => files.push(MusicLocalFile {
                    file_id,
                    media_item_id: uuid(row.try_get("media_item_id")?)?,
                    relative_path: row.try_get("relative_path")?,
                }),
                Err(RepositoryError::InvalidInput | RepositoryError::NotFound) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(files)
    }
    pub async fn link_track(&self, track_id: Uuid, file_id: Uuid) -> Result<(), RepositoryError> {
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("UPDATE media_items SET updated_at=updated_at WHERE id=?")
            .bind(track_id.to_string())
            .execute(&mut *tx)
            .await?;
        ensure_item(&mut tx, track_id, MediaKind::MusicTrack).await?;
        let release_id:String=sqlx::query_scalar("SELECT m.release_id FROM music_tracks t JOIN music_media m ON m.id=t.medium_id WHERE t.media_item_id=?").bind(track_id.to_string()).fetch_one(&mut *tx).await?;
        let graph = load_graph(&mut tx, uuid(&release_id)?).await?;
        ensure_graph_access(&mut tx, &graph).await?;
        ensure_file_tx(&mut tx, file_id).await?;
        sqlx::query("INSERT INTO music_track_files(track_id,file_id) VALUES(?,?) ON CONFLICT(track_id,file_id) DO NOTHING").bind(track_id.to_string()).bind(file_id.to_string()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

fn album_view(graph: &MusicReleaseGraph) -> MusicAlbumView {
    MusicAlbumView {
        release_id: graph.release.media_item_id,
        title: graph.release.title.clone(),
        artist: graph
            .release
            .artist_credits
            .iter()
            .map(|c| format!("{}{}", c.credited_name, c.join_phrase))
            .collect(),
        country: graph.release.country.clone(),
        date: graph
            .release_date_text
            .clone()
            .or_else(|| graph.release.release_date.map(|d| d.to_string())),
        medium_count: graph.media.len() as u32,
        track_count: graph.tracks.len() as u32,
    }
}
fn uuid(value: &str) -> Result<Uuid, RepositoryError> {
    Uuid::parse_str(value).map_err(|_| RepositoryError::CorruptData)
}
fn json(value: &impl Serialize) -> Result<String, RepositoryError> {
    serde_json::to_string(value).map_err(|_| RepositoryError::InvalidInput)
}
fn decode<T: DeserializeOwned>(value: &str) -> Result<T, RepositoryError> {
    serde_json::from_str(value).map_err(|_| RepositoryError::CorruptData)
}
fn identifier_kind(kind: MusicIdentifierKind) -> &'static str {
    match kind {
        MusicIdentifierKind::MusicBrainz => "music_brainz",
        MusicIdentifierKind::Isrc => "isrc",
        MusicIdentifierKind::Upc => "upc",
        MusicIdentifierKind::Ean => "ean",
        MusicIdentifierKind::CatalogNumber => "catalog_number",
    }
}

async fn load_entity<T: DeserializeOwned>(
    tx: &mut Transaction<'_, Sqlite>,
    table: &str,
    id: Uuid,
) -> Result<T, RepositoryError> {
    let value: String = sqlx::query_scalar(&format!(
        "SELECT entity_json FROM {table} WHERE media_item_id=?"
    ))
    .bind(id.to_string())
    .fetch_one(&mut **tx)
    .await?;
    decode(&value)
}
async fn load_many<T: DeserializeOwned>(
    tx: &mut Transaction<'_, Sqlite>,
    query: &str,
    id: Uuid,
) -> Result<Vec<T>, RepositoryError> {
    let rows: Vec<String> = sqlx::query_scalar(query)
        .bind(id.to_string())
        .fetch_all(&mut **tx)
        .await?;
    rows.iter().map(|s| decode(s)).collect()
}
pub(crate) async fn load_graph(
    tx: &mut Transaction<'_, Sqlite>,
    id: Uuid,
) -> Result<MusicReleaseGraph, RepositoryError> {
    let row = sqlx::query("SELECT * FROM music_releases WHERE media_item_id=?")
        .bind(id.to_string())
        .fetch_one(&mut **tx)
        .await?;
    let release: MusicRelease = decode(row.try_get("entity_json")?)?;
    let release_group: MusicReleaseGroup =
        load_entity(tx, "music_release_groups", release.release_group_id).await?;
    let media: Vec<MusicMedium> = load_many(
        tx,
        "SELECT entity_json FROM music_media WHERE release_id=? ORDER BY position",
        id,
    )
    .await?;
    let tracks:Vec<MusicTrack>=load_many(tx,"SELECT t.entity_json FROM music_tracks t JOIN music_media m ON m.id=t.medium_id WHERE m.release_id=? ORDER BY m.position,t.position",id).await?;
    let recordings:Vec<MusicRecording>=load_many(tx,"SELECT DISTINCT r.entity_json FROM music_recordings r JOIN music_tracks t ON t.recording_id=r.media_item_id JOIN music_media m ON m.id=t.medium_id WHERE m.release_id=? ORDER BY r.media_item_id",id).await?;
    let mut ids: HashSet<Uuid> = HashSet::new();
    for credit in release_group
        .artist_credits
        .iter()
        .chain(release.artist_credits.iter())
        .chain(tracks.iter().flat_map(|t| t.artist_credits.iter()))
        .chain(recordings.iter().flat_map(|r| r.artist_credits.iter()))
    {
        ids.insert(credit.artist_id);
    }
    let mut artists: Vec<MusicArtist> = Vec::new();
    for id in ids {
        artists.push(load_entity(tx, "music_artists", id).await?);
    }
    artists.sort_by_key(|a| a.media_item_id);
    let mut entity_ids: Vec<Uuid> = artists
        .iter()
        .map(|a| a.media_item_id)
        .chain(tracks.iter().map(|a| a.media_item_id))
        .chain(recordings.iter().map(|a| a.media_item_id))
        .collect();
    entity_ids.extend([id, release_group.media_item_id]);
    let mut identifiers = Vec::new();
    let mut field_sources = Vec::new();
    for entity_id in entity_ids {
        for row in sqlx::query(
            "SELECT * FROM metadata_values WHERE media_item_id=? ORDER BY field_key,locale,id",
        )
        .bind(entity_id.to_string())
        .fetch_all(&mut **tx)
        .await?
        {
            let fetched_at: Option<i64> = row.try_get("fetched_at")?;
            let locale: String = row.try_get("locale")?;
            let field_key: String = row.try_get("field_key")?;
            let mut public_value: serde_json::Value = decode(row.try_get("value_json")?)?;
            if field_key == "music_selected_cover" {
                // Backend cache identities/paths are not library metadata DTOs.
                public_value = serde_json::json!({"source_url":public_value.get("source_url").and_then(serde_json::Value::as_str),"license":public_value.get("license").and_then(serde_json::Value::as_str)});
            }
            field_sources.push(MetadataValue {
                id: uuid(row.try_get("id")?)?,
                media_item_id: entity_id,
                field_key,
                locale: if locale.is_empty() {
                    None
                } else {
                    Some(locale)
                },
                value: public_value,
                provider_id: row.try_get("provider_id")?,
                source_url: row.try_get("source_url")?,
                fetched_at: fetched_at
                    .map(|t| DateTime::from_timestamp_millis(t).ok_or(RepositoryError::CorruptData))
                    .transpose()?,
                confidence: row.try_get("confidence")?,
                is_user_modified: row.try_get("is_user_modified")?,
                is_locked: row.try_get("is_locked")?,
            });
        }
        for row in sqlx::query("SELECT kind,namespace,value FROM music_identifiers WHERE media_item_id=? ORDER BY kind,namespace,value").bind(entity_id.to_string()).fetch_all(&mut **tx).await? {
            let kind:String=row.try_get("kind")?;
            identifiers.push(MusicIdentifier{media_item_id:entity_id,kind:serde_json::from_value(serde_json::Value::String(kind)).map_err(|_|RepositoryError::CorruptData)?,namespace:row.try_get("namespace")?,value:row.try_get("value")?});
        }
    }
    let files=sqlx::query("SELECT tf.track_id,tf.file_id,f.relative_path FROM music_track_files tf JOIN music_tracks t ON t.media_item_id=tf.track_id JOIN music_media m ON m.id=t.medium_id JOIN media_files f ON f.id=tf.file_id WHERE m.release_id=? ORDER BY m.position,t.position,f.id").bind(id.to_string()).fetch_all(&mut **tx).await?.iter().map(|r|Ok(MusicTrackFile{track_id:uuid(r.try_get("track_id")?)?,file_id:uuid(r.try_get("file_id")?)?,relative_path:r.try_get("relative_path")?})).collect::<Result<Vec<_>,RepositoryError>>()?;
    let graph = MusicReleaseGraph {
        artists,
        release_group,
        release,
        media,
        tracks,
        recordings,
        identifiers,
        files,
        field_sources,
        source_url: row.try_get("source_url")?,
        fetched_at: DateTime::from_timestamp_millis(row.try_get("fetched_at")?)
            .ok_or(RepositoryError::CorruptData)?,
        locale: row.try_get("locale")?,
        release_date_text: row.try_get("release_date_text")?,
        group_first_release_date_text: row.try_get("group_first_release_date_text")?,
        release_disambiguation: row.try_get("release_disambiguation")?,
    };
    validate_graph(&graph).map_err(|_| RepositoryError::CorruptData)?;
    Ok(graph)
}

fn text(value: &str) -> Result<(), RepositoryError> {
    if value.trim().is_empty() || value.contains('\0') || value.len() > 16384 {
        Err(RepositoryError::InvalidInput)
    } else {
        Ok(())
    }
}
fn validate_graph(graph: &MusicReleaseGraph) -> Result<(), RepositoryError> {
    let url = reqwest::Url::parse(&graph.source_url).map_err(|_| RepositoryError::InvalidInput)?;
    if url.scheme() != "https"
        || url.host_str() != Some("musicbrainz.org")
        || !url.username().is_empty()
        || url.password().is_some()
        || graph.artists.len() > 1000
        || graph.tracks.len() > 10000
        || graph.media.len() > 100
        || graph.recordings.len() > 10000
    {
        return Err(RepositoryError::InvalidInput);
    }
    let mut entity_ids = HashSet::new();
    let mut insert = |id: Uuid| {
        if id.is_nil() || !entity_ids.insert(id) {
            Err(RepositoryError::InvalidInput)
        } else {
            Ok(())
        }
    };
    insert(graph.release_group.media_item_id)?;
    insert(graph.release.media_item_id)?;
    text(&graph.release_group.title)?;
    text(&graph.release.title)?;
    if graph.release.release_group_id != graph.release_group.media_item_id {
        return Err(RepositoryError::InvalidInput);
    }
    let mut artists = HashSet::new();
    for artist in &graph.artists {
        insert(artist.media_item_id)?;
        artists.insert(artist.media_item_id);
        text(&artist.name)?;
        for alias in &artist.aliases {
            text(alias)?;
        }
    }
    let mut media = HashSet::new();
    let mut medium_positions = HashSet::new();
    for medium in &graph.media {
        if medium.id.is_nil()
            || !media.insert(medium.id)
            || medium.release_id != graph.release.media_item_id
            || medium.position == 0
            || !medium_positions.insert(medium.position)
        {
            return Err(RepositoryError::InvalidInput);
        }
    }
    let mut recordings = HashSet::new();
    for recording in &graph.recordings {
        insert(recording.media_item_id)?;
        text(&recording.title)?;
        recordings.insert(recording.media_item_id);
    }
    let mut positions = HashSet::new();
    for track in &graph.tracks {
        insert(track.media_item_id)?;
        text(&track.title)?;
        text(&track.number)?;
        if !media.contains(&track.medium_id)
            || !recordings.contains(&track.recording_id)
            || track.position == 0
            || !positions.insert((track.medium_id, track.position))
        {
            return Err(RepositoryError::InvalidInput);
        }
    }
    for credit in graph
        .release_group
        .artist_credits
        .iter()
        .chain(graph.release.artist_credits.iter())
        .chain(graph.tracks.iter().flat_map(|t| t.artist_credits.iter()))
        .chain(
            graph
                .recordings
                .iter()
                .flat_map(|r| r.artist_credits.iter()),
        )
    {
        if !artists.contains(&credit.artist_id) {
            return Err(RepositoryError::InvalidInput);
        }
        text(&credit.credited_name)?;
    }
    for identifier in &graph.identifiers {
        if !entity_ids.contains(&identifier.media_item_id) {
            return Err(RepositoryError::InvalidInput);
        }
        text(&identifier.namespace)?;
        text(&identifier.value)?;
        if identifier.kind == MusicIdentifierKind::MusicBrainz
            && Uuid::parse_str(&identifier.value).is_err()
        {
            return Err(RepositoryError::InvalidInput);
        }
    }
    if !graph.identifiers.iter().any(|i| {
        i.media_item_id == graph.release.media_item_id
            && i.kind == MusicIdentifierKind::MusicBrainz
            && i.namespace == "release"
    }) {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(())
}
async fn protect<T: Serialize + DeserializeOwned>(
    tx: &mut Transaction<'_, Sqlite>,
    id: Uuid,
    entity: &mut T,
    locale: Option<&str>,
) -> Result<(), RepositoryError> {
    let mut value = serde_json::to_value(&*entity).map_err(|_| RepositoryError::InvalidInput)?;
    // A protected value in any locale must never be erased by an incoming provider.
    let fields=sqlx::query("SELECT field_key,value_json FROM metadata_values WHERE media_item_id=? AND (is_locked=1 OR is_user_modified=1) ORDER BY CASE WHEN locale=? THEN 2 WHEN locale='' THEN 1 ELSE 0 END, id").bind(id.to_string()).bind(locale.unwrap_or("")).fetch_all(&mut **tx).await?;
    for field in fields {
        let key: String = field.try_get("field_key")?;
        if let Some(map) = value.as_object_mut() {
            let key = if key == "title" && !map.contains_key("title") && map.contains_key("name") {
                "name".to_owned()
            } else {
                key
            };
            // Protected identity fields cannot authorize changing the graph topology.
            if matches!(
                key.as_str(),
                "media_item_id" | "id" | "release_group_id" | "medium_id" | "recording_id"
            ) {
                return Err(RepositoryError::ProtectedMetadata);
            }
            if map.contains_key(&key) {
                map.insert(key, decode(field.try_get("value_json")?)?);
            }
        }
    }
    *entity = serde_json::from_value(value).map_err(|_| RepositoryError::ProtectedMetadata)?;
    Ok(())
}
async fn save_item(
    tx: &mut Transaction<'_, Sqlite>,
    id: Uuid,
    kind: MediaKind,
    title: &str,
    graph: &MusicReleaseGraph,
) -> Result<(), RepositoryError> {
    if sqlx::query_scalar::<_, String>("SELECT id FROM media_items WHERE id=?")
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await?
        .is_some()
    {
        ensure_item(tx, id, kind).await?;
    }
    sqlx::query("INSERT INTO media_items(id,kind,title,created_at,updated_at) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title,updated_at=MAX(media_items.updated_at,excluded.updated_at) WHERE media_items.kind=excluded.kind AND media_items.adult_isolated=0")
        .bind(id.to_string()).bind(kind.as_str()).bind(title).bind(graph.fetched_at.timestamp_millis()).bind(graph.fetched_at.timestamp_millis()).execute(&mut **tx).await?;
    Ok(())
}
async fn save_entity(
    tx: &mut Transaction<'_, Sqlite>,
    table: &str,
    id: Uuid,
    entity: &impl Serialize,
) -> Result<(), RepositoryError> {
    sqlx::query(&format!("INSERT INTO {table}(media_item_id,entity_json) VALUES(?,?) ON CONFLICT(media_item_id) DO UPDATE SET entity_json=excluded.entity_json")).bind(id.to_string()).bind(json(entity)?).execute(&mut **tx).await?;
    Ok(())
}
async fn save_credits(
    tx: &mut Transaction<'_, Sqlite>,
    id: Uuid,
    credits: &[ArtistCredit],
) -> Result<(), RepositoryError> {
    sqlx::query("DELETE FROM music_artist_credits WHERE media_item_id=?")
        .bind(id.to_string())
        .execute(&mut **tx)
        .await?;
    for (position, credit) in credits.iter().enumerate() {
        sqlx::query("INSERT INTO music_artist_credits(media_item_id,position,artist_id,credited_name,join_phrase) VALUES(?,?,?,?,?)").bind(id.to_string()).bind(position as i64).bind(credit.artist_id.to_string()).bind(&credit.credited_name).bind(&credit.join_phrase).execute(&mut **tx).await?;
    }
    Ok(())
}
async fn ensure_item(
    tx: &mut Transaction<'_, Sqlite>,
    id: Uuid,
    kind: MediaKind,
) -> Result<(), RepositoryError> {
    let valid:bool=sqlx::query_scalar("SELECT kind=? AND adult_isolated=0 AND NOT EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=media_items.id AND (r.kind='adult_video' OR r.enabled=0)) FROM media_items WHERE id=?").bind(kind.as_str()).bind(id.to_string()).fetch_one(&mut **tx).await?;
    if !valid {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(())
}
async fn ensure_file_tx(tx: &mut Transaction<'_, Sqlite>, id: Uuid) -> Result<(), RepositoryError> {
    let valid:bool=sqlx::query_scalar("SELECT m.kind IN('music_artist','music_release_group','music_release','music_track','music_recording') AND m.adult_isolated=0 AND r.enabled=1 AND (r.kind IS NULL OR r.kind IN('music_artist','music_release_group','music_release','music_track','music_recording')) AND NOT EXISTS(SELECT 1 FROM media_files af JOIN library_roots ar ON ar.id=af.root_id WHERE af.media_item_id=m.id AND (ar.kind='adult_video' OR ar.enabled=0)) FROM media_files f JOIN media_items m ON m.id=f.media_item_id JOIN library_roots r ON r.id=f.root_id WHERE f.id=?").bind(id.to_string()).fetch_one(&mut **tx).await?;
    if !valid {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(())
}
pub(crate) async fn ensure_graph_access(
    tx: &mut Transaction<'_, Sqlite>,
    graph: &MusicReleaseGraph,
) -> Result<(), RepositoryError> {
    ensure_item(tx, graph.release.media_item_id, MediaKind::MusicRelease).await?;
    ensure_item(
        tx,
        graph.release_group.media_item_id,
        MediaKind::MusicReleaseGroup,
    )
    .await?;
    for artist in &graph.artists {
        ensure_item(tx, artist.media_item_id, MediaKind::MusicArtist).await?;
    }
    for recording in &graph.recordings {
        ensure_item(tx, recording.media_item_id, MediaKind::MusicRecording).await?;
    }
    for track in &graph.tracks {
        ensure_item(tx, track.media_item_id, MediaKind::MusicTrack).await?;
    }
    for file in &graph.files {
        ensure_file_tx(tx, file.file_id).await?;
    }
    Ok(())
}

/// Resolve a scanned audio file only after checking current ordinary-lane authority.
/// All associated file roots are checked, so reclassification cannot bypass isolation.
pub async fn ensure_music_file(
    repo: &LibraryRepository,
    file_id: Uuid,
) -> Result<(MediaFile, LibraryRoot), RepositoryError> {
    let mut tx = repo.pool().begin().await?;
    let result = ensure_music_file_tx(&mut tx, file_id).await?;
    tx.commit().await?;
    Ok(result)
}

pub(crate) async fn ensure_music_file_tx(
    tx: &mut Transaction<'_, Sqlite>,
    file_id: Uuid,
) -> Result<(MediaFile, LibraryRoot), RepositoryError> {
    ensure_file_tx(tx, file_id).await?;
    let row = sqlx::query("SELECT * FROM media_files WHERE id=?")
        .bind(file_id.to_string())
        .fetch_one(&mut **tx)
        .await?;
    let media_item_id = uuid(row.try_get("media_item_id")?)?;
    let root_id = uuid(row.try_get("root_id")?)?;
    // A linked imported graph must retain its ordinary authority as well.
    let releases:Vec<String>=sqlx::query_scalar("SELECT m.release_id FROM music_track_files tf JOIN music_tracks t ON t.media_item_id=tf.track_id JOIN music_media m ON m.id=t.medium_id WHERE tf.file_id=?").bind(file_id.to_string()).fetch_all(&mut **tx).await?;
    for release in releases {
        let graph = load_graph(tx, uuid(&release)?).await?;
        ensure_graph_access(tx, &graph).await?;
    }
    let modified_at: Option<i64> = row.try_get("modified_at")?;
    let file = MediaFile {
        id: file_id,
        media_item_id,
        root_id,
        relative_path: row.try_get("relative_path")?,
        size_bytes: u64::try_from(row.try_get::<i64, _>("size_bytes")?)
            .map_err(|_| RepositoryError::CorruptData)?,
        modified_at: modified_at
            .map(|t| DateTime::from_timestamp_millis(t).ok_or(RepositoryError::CorruptData))
            .transpose()?,
        part: row
            .try_get::<Option<i64>, _>("part")?
            .map(|p| u32::try_from(p).map_err(|_| RepositoryError::CorruptData))
            .transpose()?,
    };
    let root_row = sqlx::query("SELECT * FROM library_roots WHERE id=?")
        .bind(root_id.to_string())
        .fetch_one(&mut **tx)
        .await?;
    let kind: Option<String> = root_row.try_get("kind")?;
    let root = LibraryRoot {
        id: root_id,
        name: root_row.try_get("name")?,
        path: root_row.try_get("path")?,
        path_style: serde_json::from_value(serde_json::Value::String(
            root_row.try_get("path_style")?,
        ))
        .map_err(|_| RepositoryError::CorruptData)?,
        kind: kind
            .map(|k| {
                serde_json::from_value(serde_json::Value::String(k))
                    .map_err(|_| RepositoryError::CorruptData)
            })
            .transpose()?,
        enabled: root_row.try_get("enabled")?,
        created_at: DateTime::from_timestamp_millis(root_row.try_get("created_at")?)
            .ok_or(RepositoryError::CorruptData)?,
        updated_at: DateTime::from_timestamp_millis(root_row.try_get("updated_at")?)
            .ok_or(RepositoryError::CorruptData)?,
    };
    Ok((file, root))
}

async fn protect_envelope(
    tx: &mut Transaction<'_, Sqlite>,
    graph: &mut MusicReleaseGraph,
) -> Result<(), RepositoryError> {
    for (id, is_group) in [
        (graph.release.media_item_id, false),
        (graph.release_group.media_item_id, true),
    ] {
        let rows=sqlx::query("SELECT field_key,value_json FROM metadata_values WHERE media_item_id=? AND (is_locked=1 OR is_user_modified=1) ORDER BY CASE WHEN locale=? THEN 2 WHEN locale='' THEN 1 ELSE 0 END,id").bind(id.to_string()).bind(graph.locale.as_deref().unwrap_or("")).fetch_all(&mut **tx).await?;
        for row in rows {
            let key: String = row.try_get("field_key")?;
            let value: serde_json::Value = decode(row.try_get("value_json")?)?;
            let date_field = if is_group {
                "first_release_date"
            } else {
                "release_date"
            };
            let raw_field = if is_group {
                "group_first_release_date_text"
            } else {
                "release_date_text"
            };
            if key == date_field || key == raw_field {
                let date: Option<String> = serde_json::from_value(value)
                    .map_err(|_| RepositoryError::ProtectedMetadata)?;
                if is_group {
                    graph.group_first_release_date_text = date.clone();
                    graph.release_group.first_release_date =
                        date.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());
                } else {
                    graph.release_date_text = date.clone();
                    graph.release.release_date =
                        date.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());
                }
            } else if !is_group && key == "release_disambiguation" {
                graph.release_disambiguation = serde_json::from_value(value)
                    .map_err(|_| RepositoryError::ProtectedMetadata)?;
            }
        }
    }
    Ok(())
}

async fn save_sources(
    tx: &mut Transaction<'_, Sqlite>,
    id: Uuid,
    entity: &impl Serialize,
    graph: &MusicReleaseGraph,
) -> Result<(), RepositoryError> {
    let mut value = serde_json::to_value(entity).map_err(|_| RepositoryError::InvalidInput)?;
    let fields = value.as_object_mut().ok_or(RepositoryError::InvalidInput)?;
    let is_artist = fields.contains_key("name");
    if let Some(name) = fields.get("name").cloned() {
        fields.insert("title".into(), name);
    }
    for (key, value) in fields {
        if matches!(
            key.as_str(),
            "media_item_id" | "id" | "release_group_id" | "medium_id" | "recording_id"
        ) {
            continue;
        }
        let alternate = if key == "name" {
            "title"
        } else if key == "title" && is_artist {
            "name"
        } else if key == "release_date_text" {
            "release_date"
        } else if key == "release_date" {
            "release_date_text"
        } else if key == "group_first_release_date_text" {
            "first_release_date"
        } else if key == "first_release_date" {
            "group_first_release_date_text"
        } else {
            key.as_str()
        };
        sqlx::query("INSERT INTO metadata_values(id,media_item_id,field_key,locale,value_json,provider_id,source_url,fetched_at,confidence,is_user_modified,is_locked) SELECT ?,?,?,?,?,'musicbrainz',?,?,1,0,0 WHERE NOT EXISTS(SELECT 1 FROM metadata_values WHERE media_item_id=? AND field_key IN(?,?) AND (is_user_modified=1 OR is_locked=1)) ON CONFLICT(media_item_id,field_key,locale) DO UPDATE SET value_json=excluded.value_json,provider_id=excluded.provider_id,source_url=excluded.source_url,fetched_at=excluded.fetched_at WHERE metadata_values.is_user_modified=0 AND metadata_values.is_locked=0")
            .bind(Uuid::new_v4().to_string()).bind(id.to_string()).bind(key).bind(graph.locale.as_deref().unwrap_or("")).bind(json(value)?).bind(&graph.source_url).bind(graph.fetched_at.timestamp_millis()).bind(id.to_string()).bind(key).bind(alternate).execute(&mut **tx).await?;
    }
    Ok(())
}
