//! Read approved local artwork and return a bounded PNG thumbnail; never fetch a URL.

use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{ImageFormat, ImageReader, Limits};
use mediaatlas_domain::{normalize_relative_path, ArtworkKind};
use std::{
    hash::{Hash, Hasher},
    io::{Cursor, Read},
    path::Path,
};

impl BrowseService {
    pub(super) async fn poster(&self, media: &MediaItem) -> Result<Option<String>> {
        let isolated: bool =
            sqlx::query_scalar("SELECT adult_isolated FROM media_items WHERE id=?")
                .bind(media.id.to_string())
                .fetch_one(self.repo.pool())
                .await?;
        if !self.render_posters {
            return Ok(None);
        }
        let adult = media.kind == MediaKind::AdultVideo || isolated;
        if adult {
            let Some(access) = &self.adult_access else {
                return Ok(None);
            };
            access.ensure()?;
        }
        let artworks = self.repo.list_artworks(media.id).await?;
        for art in artworks {
            {
                if let (Some(cache), Some(hash), Some(path)) = (
                    &self.approved_artwork_cache,
                    &art.content_hash,
                    &art.cache_path,
                ) {
                    if matches!(art.kind, ArtworkKind::Poster | ArtworkKind::Cover)
                        && hash.len() == 64
                        && hash.bytes().all(|b| b.is_ascii_hexdigit())
                        && Path::new(path).parent() == Some(cache.as_path())
                    {
                        let cached = crate::media_output::CachedArtwork {
                            cache_path: path.clone(),
                            thumbnail_path: cache
                                .join(format!("{hash}.thumb.png"))
                                .to_string_lossy()
                                .into_owned(),
                            content_hash: hash.clone(),
                            width: art.width.unwrap_or(0),
                            height: art.height.unwrap_or(0),
                            mime: String::new(),
                        };
                        let service = if adult {
                            crate::media_output::MediaOutputService::new_adult(
                                self.repo.clone(),
                                cache.clone(),
                                self.adult_access
                                    .clone()
                                    .ok_or(RepositoryError::ProtectedMetadata)?,
                            )
                        } else {
                            crate::media_output::MediaOutputService::new(
                                self.repo.clone(),
                                cache.clone(),
                            )
                        };
                        if let Ok(bytes) = service.thumbnail_bytes(&cached) {
                            if let Some(access) = &self.adult_access {
                                access.ensure()?;
                            }
                            return Ok(Some(format!(
                                "data:image/png;base64,{}",
                                STANDARD.encode(bytes)
                            )));
                        }
                    }
                }
            }
            if !matches!(
                art.kind,
                ArtworkKind::Poster | ArtworkKind::Cover | ArtworkKind::AlbumCover
            ) {
                continue;
            }
            let Some(source) = art
                .source_url
                .as_deref()
                .and_then(|v| v.strip_prefix("library://"))
            else {
                continue;
            };
            let Some((id, relative)) = source.split_once('/') else {
                continue;
            };
            let Ok(root_id) = Uuid::parse_str(id) else {
                continue;
            };
            let Some(root) = self.repo.get_root(root_id).await? else {
                continue;
            };
            // Adult cache material must never be rendered in an ordinary card.
            if root.kind == Some(MediaKind::AdultVideo)
                && media.kind != MediaKind::AdultVideo
                && !isolated
            {
                continue;
            }
            if normalize_relative_path(relative, root.path_style).is_err() {
                continue;
            }
            let root_path = PathBuf::from(root.path);
            let source = root_path.join(relative);
            let folder = self
                .cache_dir
                .join(if media.kind == MediaKind::AdultVideo || isolated {
                    "adult"
                } else {
                    "general"
                });
            let art_id = art.id;
            let result = tokio::task::spawn_blocking(move || {
                thumbnail(&root_path, &source, &folder, art_id)
            })
            .await;
            if let Ok(Ok(Some(bytes))) = result {
                if let Some(access) = &self.adult_access {
                    access.ensure()?;
                }
                return Ok(Some(format!(
                    "data:image/png;base64,{}",
                    STANDARD.encode(bytes)
                )));
            }
        }
        Ok(None)
    }
}

fn thumbnail(
    root: &Path,
    source: &Path,
    cache: &Path,
    id: Uuid,
) -> std::io::Result<Option<Vec<u8>>> {
    let root = std::fs::canonicalize(root)?;
    let source = std::fs::canonicalize(source)?;
    if !source.starts_with(&root) || !source.is_file() {
        return Ok(None);
    }
    let metadata = std::fs::metadata(&source)?;
    const MAX: u64 = 16 * 1024 * 1024;
    if metadata.len() > MAX {
        return Ok(None);
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hash);
    metadata.len().hash(&mut hash);
    metadata.modified()?.hash(&mut hash);
    id.hash(&mut hash);
    std::fs::create_dir_all(cache)?;
    if std::fs::symlink_metadata(cache)?.file_type().is_symlink() {
        return Ok(None);
    }
    let file = cache.join(format!("{:016x}.png", hash.finish()));
    if let Ok(meta) = std::fs::symlink_metadata(&file) {
        if meta.file_type().is_symlink() {
            return Ok(None);
        }
        if meta.is_file() && meta.len() < 1024 * 1024 {
            return std::fs::read(file).map(Some);
        }
    }
    let mut data = Vec::new();
    std::fs::File::open(source)?
        .take(MAX + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX {
        return Ok(None);
    }
    let Ok(mut reader) = ImageReader::new(Cursor::new(data)).with_guessed_format() else {
        return Ok(None);
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let Ok(image) = reader.decode() else {
        return Ok(None);
    };
    let image = image.thumbnail(240, 360);
    let mut png = Cursor::new(Vec::new());
    if image.write_to(&mut png, ImageFormat::Png).is_err() {
        return Ok(None);
    }
    let bytes = png.into_inner();
    std::fs::write(file, &bytes)?;
    Ok(Some(bytes))
}
