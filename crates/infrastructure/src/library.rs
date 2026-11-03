//! SQLite repositories for the local library. No method performs media-file I/O.

use chrono::{DateTime, Utc};
use mediaatlas_domain::{
    normalize_locale, normalize_relative_path, normalize_root_path, Artwork, ExternalId,
    LibraryRoot, MediaFile, MediaItem, MediaKind, MetadataValue,
};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow},
    ConnectOptions, QueryBuilder, Row, Sqlite, SqlitePool,
};
use std::{fmt, path::Path, time::Duration};
use uuid::Uuid;

/// Errors deliberately omit raw SQL, database paths and user values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryError {
    InvalidInput,
    NotFound,
    Conflict,
    ProtectedMetadata,
    Database,
    Migration,
    CorruptData,
    Provider(mediaatlas_domain::ProviderFailure),
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "Repository input is invalid.",
            Self::NotFound => "The requested library record was not found.",
            Self::Conflict => "The change conflicts with an existing library record.",
            Self::ProtectedMetadata => "The metadata field is protected from provider updates.",
            Self::Database => "The library database operation failed.",
            Self::Migration => "The library database migration failed.",
            Self::CorruptData => "The library database contains invalid data.",
            Self::Provider(_) => "The provider request could not be completed.",
        })
    }
}

impl std::error::Error for RepositoryError {}

impl From<sqlx::Error> for RepositoryError {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::Database(ref db)
                if db.is_unique_violation() || db.is_foreign_key_violation() =>
            {
                Self::Conflict
            }
            sqlx::Error::RowNotFound => Self::NotFound,
            _ => Self::Database,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataWriteMode {
    User,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaCursor {
    pub updated_at: DateTime<Utc>,
    pub id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ListMediaQuery {
    pub kind: Option<MediaKind>,
    pub root_id: Option<Uuid>,
    pub query: Option<String>,
    pub cursor: Option<MediaCursor>,
    pub limit: u32,
}

impl Default for ListMediaQuery {
    fn default() -> Self {
        Self {
            kind: None,
            root_id: None,
            query: None,
            cursor: None,
            limit: 50,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MediaPage {
    pub items: Vec<MediaItem>,
    pub next_cursor: Option<MediaCursor>,
}

#[derive(Clone)]
pub struct LibraryRepository {
    pool: SqlitePool,
}

impl LibraryRepository {
    pub async fn open(path: &Path) -> Result<Self, RepositoryError> {
        if path.as_os_str().is_empty() {
            return Err(RepositoryError::InvalidInput);
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5))
            .disable_statement_logging();
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        if crate::migration_compat::normalize_current_database(&pool, path)
            .await
            .is_err()
        {
            pool.close().await;
            return Err(RepositoryError::Migration);
        }
        crate::migration_compat::canonical_migrator()
            .run(&pool)
            .await
            .map_err(|_| RepositoryError::Migration)?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn save_root(&self, root: &LibraryRoot) -> Result<(), RepositoryError> {
        nonempty(&root.name)?;
        validate_times(root.created_at, root.updated_at)?;
        let normalized = normalize_root_path(&root.path, root.path_style)
            .map_err(|_| RepositoryError::InvalidInput)?;
        let mut tx = self.pool.begin().await?;
        // Reserve the SQLite writer before taking a Root before-image. A
        // deferred read snapshot cannot safely be upgraded after another
        // connection commits, including an asynchronously dropped failed write.
        sqlx::query("UPDATE library_roots SET enabled=enabled WHERE id=?")
            .bind(root.id.to_string())
            .execute(&mut *tx)
            .await?;
        let previous = sqlx::query("SELECT path_style FROM library_roots WHERE id = ?")
            .bind(root.id.to_string())
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(previous) = previous {
            if column::<String>(&previous, "path_style")? != enum_string(&root.path_style)? {
                let count: i64 =
                    sqlx::query_scalar("SELECT count(*) FROM media_files WHERE root_id = ?")
                        .bind(root.id.to_string())
                        .fetch_one(&mut *tx)
                        .await?;
                if count > 0 {
                    tx.rollback().await?;
                    return Err(RepositoryError::Conflict);
                }
            }
        }
        let saved = sqlx::query("INSERT INTO library_roots (id,name,path,normalized_path,path_style,kind,enabled,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,path=excluded.path,normalized_path=excluded.normalized_path,path_style=excluded.path_style,kind=excluded.kind,enabled=excluded.enabled,updated_at=excluded.updated_at")
            .bind(root.id.to_string()).bind(&root.name).bind(&root.path).bind(normalized)
            .bind(enum_string(&root.path_style)?).bind(optional_enum(root.kind.as_ref())?)
            .bind(root.enabled).bind(root.created_at.timestamp_millis()).bind(root.updated_at.timestamp_millis())
            .execute(&mut *tx).await;
        if let Err(error) = saved {
            // Await rollback before returning a constraint conflict so the next
            // request does not inherit an outstanding writer reservation.
            tx.rollback().await?;
            return Err(error.into());
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_root(&self, id: Uuid) -> Result<Option<LibraryRoot>, RepositoryError> {
        sqlx::query("SELECT * FROM library_roots WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(root_from_row)
            .transpose()
    }

    pub async fn list_roots(&self) -> Result<Vec<LibraryRoot>, RepositoryError> {
        sqlx::query("SELECT * FROM library_roots ORDER BY id")
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(root_from_row)
            .collect()
    }

    pub async fn remove_root(&self, id: Uuid) -> Result<bool, RepositoryError> {
        Ok(sqlx::query("DELETE FROM library_roots WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }

    pub async fn save_media(&self, item: &MediaItem) -> Result<(), RepositoryError> {
        nonempty(&item.title)?;
        validate_times(item.created_at, item.updated_at)?;
        sqlx::query("INSERT INTO media_items (id,kind,title,original_title,year,created_at,updated_at) VALUES (?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET kind=excluded.kind,title=excluded.title,original_title=excluded.original_title,year=excluded.year,updated_at=excluded.updated_at")
            .bind(item.id.to_string()).bind(enum_string(&item.kind)?).bind(&item.title)
            .bind(&item.original_title).bind(item.year).bind(item.created_at.timestamp_millis()).bind(item.updated_at.timestamp_millis())
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_media(&self, id: Uuid) -> Result<Option<MediaItem>, RepositoryError> {
        sqlx::query("SELECT * FROM media_items WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(media_from_row)
            .transpose()
    }

    pub async fn remove_media(&self, id: Uuid) -> Result<bool, RepositoryError> {
        Ok(sqlx::query("DELETE FROM media_items WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }

    pub async fn list_media(&self, query: &ListMediaQuery) -> Result<MediaPage, RepositoryError> {
        if !(1..=200).contains(&query.limit) {
            return Err(RepositoryError::InvalidInput);
        }
        let mut sql = QueryBuilder::<Sqlite>::new("SELECT m.* FROM media_items m WHERE 1=1");
        if let Some(kind) = &query.kind {
            sql.push(" AND m.kind = ").push_bind(enum_string(kind)?);
        }
        if let Some(root) = query.root_id {
            sql.push(" AND EXISTS (SELECT 1 FROM media_files f WHERE f.media_item_id=m.id AND f.root_id=")
                .push_bind(root.to_string()).push(")");
        }
        if let Some(text) = query
            .query
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            let pattern = format!(
                "%{}%",
                text.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            sql.push(" AND (m.title LIKE ")
                .push_bind(pattern.clone())
                .push(" ESCAPE '\\' OR m.original_title LIKE ")
                .push_bind(pattern)
                .push(" ESCAPE '\\')");
        }
        if let Some(cursor) = &query.cursor {
            sql.push(" AND (m.updated_at < ")
                .push_bind(cursor.updated_at.timestamp_millis())
                .push(" OR (m.updated_at = ")
                .push_bind(cursor.updated_at.timestamp_millis())
                .push(" AND m.id > ")
                .push_bind(cursor.id.to_string())
                .push("))");
        }
        sql.push(" ORDER BY m.updated_at DESC,m.id ASC LIMIT ")
            .push_bind(i64::from(query.limit) + 1);
        let rows = sql.build().fetch_all(&self.pool).await?;
        let mut items: Vec<MediaItem> =
            rows.iter().map(media_from_row).collect::<Result<_, _>>()?;
        let has_more = items.len() > query.limit as usize;
        items.truncate(query.limit as usize);
        let next_cursor = if has_more {
            items.last().map(|item| MediaCursor {
                updated_at: item.updated_at,
                id: item.id,
            })
        } else {
            None
        };
        Ok(MediaPage { items, next_cursor })
    }

    pub async fn save_file(&self, file: &MediaFile) -> Result<(), RepositoryError> {
        // Reserve the writer before reading the Root. A deferred read snapshot
        // cannot wait safely when another connection already owns the writer.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let saved = async {
            let root = sqlx::query("SELECT * FROM library_roots WHERE id=?")
                .bind(file.root_id.to_string())
                .fetch_optional(&mut *tx)
                .await?
                .as_ref()
                .map(root_from_row)
                .transpose()?
                .ok_or(RepositoryError::NotFound)?;
            let normalized = normalize_relative_path(&file.relative_path, root.path_style)
                .map_err(|_| RepositoryError::InvalidInput)?;
            let size = i64::try_from(file.size_bytes).map_err(|_| RepositoryError::InvalidInput)?;
            let result = sqlx::query("INSERT INTO media_files (id,media_item_id,root_id,relative_path,normalized_relative_path,size_bytes,modified_at,part) VALUES (?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET relative_path=excluded.relative_path,normalized_relative_path=excluded.normalized_relative_path,size_bytes=excluded.size_bytes,modified_at=excluded.modified_at,part=excluded.part WHERE media_files.media_item_id=excluded.media_item_id AND media_files.root_id=excluded.root_id")
                .bind(file.id.to_string()).bind(file.media_item_id.to_string()).bind(file.root_id.to_string())
                .bind(&file.relative_path).bind(normalized).bind(size).bind(file.modified_at.map(|t|t.timestamp_millis()))
                .bind(file.part.map(i64::from)).execute(&mut *tx).await?;
            changed(result.rows_affected())
        }
        .await;
        match saved {
            Ok(()) => {
                tx.commit().await?;
                Ok(())
            }
            Err(error) => {
                // A rejected identity or invalid input must release its writer
                // before the next request starts; Drop only queues rollback.
                tx.rollback().await?;
                Err(error)
            }
        }
    }

    pub async fn list_files(&self, media_id: Uuid) -> Result<Vec<MediaFile>, RepositoryError> {
        sqlx::query("SELECT * FROM media_files WHERE media_item_id = ? ORDER BY id")
            .bind(media_id.to_string())
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(file_from_row)
            .collect()
    }

    pub async fn save_artwork(&self, art: &Artwork) -> Result<(), RepositoryError> {
        if art.width == Some(0) || art.height == Some(0) {
            return Err(RepositoryError::InvalidInput);
        }
        let result = sqlx::query("INSERT INTO artworks (id,media_item_id,kind,cache_path,source_url,provider_id,width,height,content_hash,created_at) VALUES (?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET kind=excluded.kind,cache_path=excluded.cache_path,source_url=excluded.source_url,provider_id=excluded.provider_id,width=excluded.width,height=excluded.height,content_hash=excluded.content_hash WHERE artworks.media_item_id=excluded.media_item_id")
            .bind(art.id.to_string()).bind(art.media_item_id.to_string()).bind(enum_string(&art.kind)?)
            .bind(&art.cache_path).bind(&art.source_url).bind(&art.provider_id).bind(art.width.map(i64::from))
            .bind(art.height.map(i64::from)).bind(&art.content_hash).bind(art.created_at.timestamp_millis())
            .execute(&self.pool).await?;
        changed(result.rows_affected())
    }

    pub async fn list_artworks(&self, media_id: Uuid) -> Result<Vec<Artwork>, RepositoryError> {
        sqlx::query("SELECT * FROM artworks WHERE media_item_id = ? ORDER BY id")
            .bind(media_id.to_string())
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(artwork_from_row)
            .collect()
    }

    pub async fn save_external_id(&self, external: &ExternalId) -> Result<(), RepositoryError> {
        nonempty(&external.provider_id)?;
        nonempty(&external.namespace)?;
        nonempty(&external.external_id)?;
        let result = sqlx::query("INSERT INTO external_ids (id,media_item_id,provider_id,namespace,external_id,is_primary,created_at) VALUES (?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET provider_id=excluded.provider_id,namespace=excluded.namespace,external_id=excluded.external_id,is_primary=excluded.is_primary WHERE external_ids.media_item_id=excluded.media_item_id")
            .bind(external.id.to_string()).bind(external.media_item_id.to_string()).bind(&external.provider_id)
            .bind(&external.namespace).bind(&external.external_id).bind(external.is_primary)
            .bind(external.created_at.timestamp_millis()).execute(&self.pool).await?;
        changed(result.rows_affected())
    }

    pub async fn list_external_ids(
        &self,
        media_id: Uuid,
    ) -> Result<Vec<ExternalId>, RepositoryError> {
        sqlx::query("SELECT * FROM external_ids WHERE media_item_id = ? ORDER BY id")
            .bind(media_id.to_string())
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(external_from_row)
            .collect()
    }

    pub async fn save_metadata_value(
        &self,
        value: &MetadataValue,
        mode: MetadataWriteMode,
    ) -> Result<(), RepositoryError> {
        value
            .validate()
            .map_err(|_| RepositoryError::InvalidInput)?;
        let normalized_locale =
            normalize_locale(value.locale.as_deref()).map_err(|_| RepositoryError::InvalidInput)?;
        if mode == MetadataWriteMode::Provider && (value.is_user_modified || value.is_locked) {
            return Err(RepositoryError::InvalidInput);
        }
        let json =
            serde_json::to_string(&value.value).map_err(|_| RepositoryError::InvalidInput)?;
        let mut tx = self.pool.begin().await?;
        let locale = normalized_locale.as_deref().unwrap_or("");
        let existing = sqlx::query("SELECT id,media_item_id,field_key,locale,is_locked,is_user_modified FROM metadata_values WHERE id=? OR (media_item_id=? AND field_key=? AND locale=?)")
            .bind(value.id.to_string()).bind(value.media_item_id.to_string()).bind(&value.field_key).bind(locale)
            .fetch_all(&mut *tx).await?;
        for row in &existing {
            if column::<String>(row, "media_item_id")? != value.media_item_id.to_string() {
                return Err(RepositoryError::Conflict);
            }
            if mode == MetadataWriteMode::Provider
                && (column::<bool>(row, "is_locked")? || column::<bool>(row, "is_user_modified")?)
            {
                return Err(RepositoryError::ProtectedMetadata);
            }
            if column::<String>(row, "id")? != value.id.to_string() {
                return Err(RepositoryError::Conflict);
            }
            if column::<String>(row, "field_key")? != value.field_key
                || column::<String>(row, "locale")? != locale
            {
                return Err(RepositoryError::Conflict);
            }
        }
        let result = sqlx::query("INSERT INTO metadata_values (id,media_item_id,field_key,locale,value_json,provider_id,source_url,fetched_at,confidence,is_user_modified,is_locked) VALUES (?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,provider_id=excluded.provider_id,source_url=excluded.source_url,fetched_at=excluded.fetched_at,confidence=excluded.confidence,is_user_modified=excluded.is_user_modified,is_locked=excluded.is_locked WHERE metadata_values.media_item_id=excluded.media_item_id AND metadata_values.field_key=excluded.field_key AND metadata_values.locale=excluded.locale AND (? OR (metadata_values.is_locked=0 AND metadata_values.is_user_modified=0))")
            .bind(value.id.to_string()).bind(value.media_item_id.to_string()).bind(&value.field_key).bind(locale)
            .bind(json).bind(&value.provider_id).bind(&value.source_url).bind(value.fetched_at.map(|t|t.timestamp_millis()))
            .bind(value.confidence).bind(mode == MetadataWriteMode::User).bind(value.is_locked).bind(mode == MetadataWriteMode::User)
            .execute(&mut *tx).await?;
        if result.rows_affected() == 0 {
            return Err(if mode == MetadataWriteMode::Provider {
                RepositoryError::ProtectedMetadata
            } else {
                RepositoryError::Conflict
            });
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_metadata_values(
        &self,
        media_id: Uuid,
    ) -> Result<Vec<MetadataValue>, RepositoryError> {
        sqlx::query("SELECT * FROM metadata_values WHERE media_item_id = ? ORDER BY field_key,COALESCE(locale,''),id")
            .bind(media_id.to_string()).fetch_all(&self.pool).await?.iter().map(metadata_from_row).collect()
    }
}

fn nonempty(text: &str) -> Result<(), RepositoryError> {
    if text.trim().is_empty() || text.contains('\0') {
        Err(RepositoryError::InvalidInput)
    } else {
        Ok(())
    }
}

fn validate_times(created: DateTime<Utc>, updated: DateTime<Utc>) -> Result<(), RepositoryError> {
    if updated < created {
        Err(RepositoryError::InvalidInput)
    } else {
        Ok(())
    }
}

fn changed(rows: u64) -> Result<(), RepositoryError> {
    if rows == 0 {
        Err(RepositoryError::Conflict)
    } else {
        Ok(())
    }
}

fn enum_string<T: Serialize>(value: &T) -> Result<String, RepositoryError> {
    match serde_json::to_value(value).map_err(|_| RepositoryError::InvalidInput)? {
        serde_json::Value::String(value) => Ok(value),
        _ => Err(RepositoryError::InvalidInput),
    }
}

fn optional_enum<T: Serialize>(value: Option<&T>) -> Result<Option<String>, RepositoryError> {
    value.map(enum_string).transpose()
}

fn column<T>(row: &SqliteRow, key: &str) -> Result<T, RepositoryError>
where
    T: for<'r> sqlx::Decode<'r, Sqlite> + sqlx::Type<Sqlite>,
{
    row.try_get(key).map_err(|_| RepositoryError::CorruptData)
}

fn id(row: &SqliteRow, key: &str) -> Result<Uuid, RepositoryError> {
    Uuid::parse_str(&column::<String>(row, key)?).map_err(|_| RepositoryError::CorruptData)
}

fn date(row: &SqliteRow, key: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::from_timestamp_millis(column(row, key)?).ok_or(RepositoryError::CorruptData)
}

fn optional_date(row: &SqliteRow, key: &str) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    column::<Option<i64>>(row, key)?
        .map(|millis| DateTime::from_timestamp_millis(millis).ok_or(RepositoryError::CorruptData))
        .transpose()
}

fn decode_enum<T: DeserializeOwned>(row: &SqliteRow, key: &str) -> Result<T, RepositoryError> {
    serde_json::from_value(serde_json::Value::String(column(row, key)?))
        .map_err(|_| RepositoryError::CorruptData)
}

fn root_from_row(row: &SqliteRow) -> Result<LibraryRoot, RepositoryError> {
    Ok(LibraryRoot {
        id: id(row, "id")?,
        name: column(row, "name")?,
        path: column(row, "path")?,
        path_style: decode_enum(row, "path_style")?,
        kind: column::<Option<String>>(row, "kind")?
            .map(|kind| {
                serde_json::from_value(serde_json::Value::String(kind))
                    .map_err(|_| RepositoryError::CorruptData)
            })
            .transpose()?,
        enabled: column(row, "enabled")?,
        created_at: date(row, "created_at")?,
        updated_at: date(row, "updated_at")?,
    })
}

fn media_from_row(row: &SqliteRow) -> Result<MediaItem, RepositoryError> {
    Ok(MediaItem {
        id: id(row, "id")?,
        kind: decode_enum(row, "kind")?,
        title: column(row, "title")?,
        original_title: column(row, "original_title")?,
        year: column(row, "year")?,
        created_at: date(row, "created_at")?,
        updated_at: date(row, "updated_at")?,
    })
}

fn file_from_row(row: &SqliteRow) -> Result<MediaFile, RepositoryError> {
    Ok(MediaFile {
        id: id(row, "id")?,
        media_item_id: id(row, "media_item_id")?,
        root_id: id(row, "root_id")?,
        relative_path: column(row, "relative_path")?,
        size_bytes: u64::try_from(column::<i64>(row, "size_bytes")?)
            .map_err(|_| RepositoryError::CorruptData)?,
        modified_at: optional_date(row, "modified_at")?,
        part: column(row, "part")?,
    })
}

fn artwork_from_row(row: &SqliteRow) -> Result<Artwork, RepositoryError> {
    Ok(Artwork {
        id: id(row, "id")?,
        media_item_id: id(row, "media_item_id")?,
        kind: decode_enum(row, "kind")?,
        cache_path: column(row, "cache_path")?,
        source_url: column(row, "source_url")?,
        provider_id: column(row, "provider_id")?,
        width: column(row, "width")?,
        height: column(row, "height")?,
        content_hash: column(row, "content_hash")?,
        created_at: date(row, "created_at")?,
    })
}

fn external_from_row(row: &SqliteRow) -> Result<ExternalId, RepositoryError> {
    Ok(ExternalId {
        id: id(row, "id")?,
        media_item_id: id(row, "media_item_id")?,
        provider_id: column(row, "provider_id")?,
        namespace: column(row, "namespace")?,
        external_id: column(row, "external_id")?,
        is_primary: column(row, "is_primary")?,
        created_at: date(row, "created_at")?,
    })
}

fn metadata_from_row(row: &SqliteRow) -> Result<MetadataValue, RepositoryError> {
    Ok(MetadataValue {
        id: id(row, "id")?,
        media_item_id: id(row, "media_item_id")?,
        field_key: column(row, "field_key")?,
        locale: Some(column::<String>(row, "locale")?).filter(|locale| !locale.is_empty()),
        value: serde_json::from_str(&column::<String>(row, "value_json")?)
            .map_err(|_| RepositoryError::CorruptData)?,
        provider_id: column(row, "provider_id")?,
        source_url: column(row, "source_url")?,
        fetched_at: optional_date(row, "fetched_at")?,
        confidence: column(row, "confidence")?,
        is_user_modified: column(row, "is_user_modified")?,
        is_locked: column(row, "is_locked")?,
    })
}
