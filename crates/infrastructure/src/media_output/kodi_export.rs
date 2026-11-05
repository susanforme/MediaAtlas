//! Prepare Kodi artwork without granting write authority over the media directory.
use super::*;
use mediaatlas_domain::{ProviderConfig, VideoMetadata};

const MAX_EXPORT_BYTES: usize = 32 * 1024 * 1024;
// Leave room for actor and content-hash suffixes within a 255-byte filename.
const MAX_IMAGE_STEM_BYTES: usize = 200;

pub(super) struct PreparedImage {
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) struct PreparedExport {
    pub(super) artwork: Vec<(ArtworkKind, String)>,
    pub(super) actor_thumbs: Option<BTreeMap<String, String>>,
    pub(super) images: Vec<PreparedImage>,
}

struct Source {
    kind: ArtworkKind,
    reference: Option<String>,
    cache: Option<(String, String)>,
}

impl PreparedExport {
    fn push_image(&mut self, image: PreparedImage) -> Result<(), RepositoryError> {
        if let Some(existing) = self.images.iter().find(|existing| existing.name == image.name) {
            return if existing.bytes == image.bytes { Ok(()) } else { Err(RepositoryError::Conflict) };
        }
        if self.images.iter().map(|image| image.bytes.len()).sum::<usize>()
            .checked_add(image.bytes.len()).is_none_or(|size| size > MAX_EXPORT_BYTES)
        {
            return Err(RepositoryError::Provider(ProviderFailure::ResponseTooLarge));
        }
        self.images.push(image);
        Ok(())
    }
}

fn kind(value: &str) -> Option<ArtworkKind> {
    match value {
        "poster" | "thumb" => Some(ArtworkKind::Poster),
        "backdrop" | "fanart" => Some(ArtworkKind::Backdrop),
        _ => None,
    }
}

fn remote_provider(reference: &str) -> Result<&'static str, RepositoryError> {
    let url = reqwest::Url::parse(reference).map_err(|_| RepositoryError::InvalidInput)?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some()
        || url.port().is_some_and(|port| port != 443) || url.fragment().is_some()
        || url.query().is_some() || reference.contains(['\\', '%'])
        || reference.contains("..") || reference.chars().any(char::is_control)
    {
        return Err(RepositoryError::Provider(ProviderFailure::UnsafeTarget));
    }
    match url.host_str() {
        Some("image.tmdb.org") => Ok("tmdb"),
        Some("s4.anilist.co") => Ok("anilist"),
        Some("static.tvmaze.com") => Ok("tvmaze"),
        Some("lain.bgm.tv") => Ok("bangumi"),
        Some("img1.doubanio.com" | "img2.doubanio.com" | "img3.doubanio.com" | "img9.doubanio.com") => Ok("douban"),
        Some("m.media-amazon.com" | "images-na.ssl-images-amazon.com") => Ok("omdb"),
        _ => Err(RepositoryError::Provider(ProviderFailure::UnsafeTarget)),
    }
}

fn local_path(reference: &str) -> Result<PathBuf, RepositoryError> {
    if reference.contains([':', '?', '#', '%']) || reference.chars().any(char::is_control) {
        return Err(RepositoryError::InvalidInput);
    }
    checked_relative(reference)
}

fn remote(reference: &str) -> bool {
    reference.get(..6).is_some_and(|scheme| scheme.eq_ignore_ascii_case("https:"))
}

fn local_bytes(dir: &Dir, reference: &str) -> Result<Option<Vec<u8>>, RepositoryError> {
    let path = local_path(reference)?;
    let parent = descend(dir.try_clone().map_err(io_error)?, path.parent().unwrap_or_else(|| Path::new("")), false)
        .map_err(io_error)?;
    let bytes = read_optional(&parent, path.file_name().ok_or(RepositoryError::InvalidInput)?, MAX_IMAGE)?;
    if let Some(bytes) = &bytes { decode_image(bytes).map_err(RepositoryError::Provider)?; }
    Ok(bytes)
}

fn image_name(dir: &Dir, requested: &str, bytes: &[u8]) -> Result<String, RepositoryError> {
    if local_path(requested)?.components().count() != 1 { return Err(RepositoryError::InvalidInput); }
    let (stem, extension) = requested.rsplit_once('.').ok_or(RepositoryError::InvalidInput)?;
    let alternate = format!("{stem}-{}.{}", &hash(bytes)[..12], extension);
    for name in [requested.to_owned(), alternate] {
        match dir.symlink_metadata(&name) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(name),
            Err(error) => return Err(io_error(error)),
            Ok(info) if info.is_file() && !info.file_type().is_symlink() && info.len() == bytes.len() as u64 => {
                match read_optional(dir, &name, MAX_IMAGE)? {
                    None => return Ok(name),
                    Some(existing) if existing == bytes => return Ok(name),
                    Some(_) => {}
                }
            }
            Ok(_) => {}
        }
    }
    Err(RepositoryError::Conflict)
}

impl MediaOutputService {
    fn export_cached_bytes(&self, path: &str, digest: &str) -> Result<Option<Vec<u8>>, RepositoryError> {
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()) {
            return Err(RepositoryError::InvalidInput);
        }
        let path = Path::new(path);
        if path.parent() != Some(self.base_cache.as_path()) { return Err(RepositoryError::InvalidInput); }
        let cache = match open_absolute_dir(&self.base_cache, false) {
            Ok(cache) => cache,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error(error)),
        };
        let bytes = read_optional(&cache, path.file_name().ok_or(RepositoryError::InvalidInput)?, MAX_IMAGE)?;
        if let Some(bytes) = &bytes {
            if hash(bytes) != digest { return Err(RepositoryError::Conflict); }
            let (format, _, _) = decode_image(bytes).map_err(RepositoryError::Provider)?;
            let name = format!("{digest}.{}", extension(format).ok_or(RepositoryError::InvalidInput)?);
            if path.file_name().and_then(|name| name.to_str()) != Some(name.as_str()) {
                return Err(RepositoryError::InvalidInput);
            }
        }
        Ok(bytes)
    }

    async fn export_remote_bytes(&self, item: &MediaItem, reference: &str) -> Result<Vec<u8>, RepositoryError> {
        let provider = remote_provider(reference)?;
        match open_absolute_dir(&self.base_cache, false) {
            Ok(cache) => {
                if let Some(bytes) = read_optional(&cache, format!("url-{}.json", hash(reference.as_bytes())), 4096)? {
                    let cached: CachedArtwork = serde_json::from_slice(&bytes).map_err(|_| RepositoryError::CorruptData)?;
                    if let Some(bytes) = self.export_cached_bytes(&cached.cache_path, &cached.content_hash)? {
                        let format = image::guess_format(&bytes).map_err(|_| RepositoryError::InvalidInput)?;
                        if mime(format) != Some(cached.mime.as_str()) { return Err(RepositoryError::CorruptData); }
                        return Ok(bytes);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
        let network = self.network.as_ref().ok_or(RepositoryError::Provider(ProviderFailure::InvalidConfiguration))?;
        let saved: Option<String> = sqlx::query_scalar("SELECT config_json FROM provider_configs WHERE provider_id=?")
            .bind(provider).fetch_optional(self.repo.pool()).await?;
        let mut config: ProviderConfig = serde_json::from_str(&saved.ok_or(RepositoryError::Provider(ProviderFailure::Disabled))?)
            .map_err(|_| RepositoryError::CorruptData)?;
        if !config.enabled { return Err(RepositoryError::Provider(ProviderFailure::Disabled)); }
        // Public image hosts never receive the metadata provider's credential.
        config.credential_ref = None;
        let id = Uuid::new_v4();
        let job = ProviderJob {
            id, provider_id: provider.into(), media_item_id: Some(item.id), media_kind: Some(item.kind),
            file_identity: None, region: None, correlation_id: id, operation: "export_artwork".into(),
            query: String::new(), language: String::new(), config, created_at: Utc::now(),
        };
        // The caller holds its gate; use an independent cache gate for downloads.
        let downloader = Self::new(self.repo.clone(), self.base_cache.clone());
        let cached = downloader.cache_artwork(network, &job, reference, &CancellationToken::new()).await
            .map_err(RepositoryError::Provider)?;
        self.export_cached_bytes(&cached.cache_path, &cached.content_hash)?.ok_or(RepositoryError::NotFound)
    }

    pub(super) async fn prepare_kodi_images(
        &self, item: &MediaItem, metadata: &[MetadataValue], stem: &str, dir: &Dir, mode: NfoExportMode,
        root_id: Uuid, relative_parent: &Path,
    ) -> Result<PreparedExport, RepositoryError> {
        if self.channel != OutputChannel::Ordinary || item.kind == MediaKind::AdultVideo {
            return Err(RepositoryError::InvalidInput);
        }
        let stem_path = checked_relative(stem)?;
        if stem_path.components().count() != 1 { return Err(RepositoryError::InvalidInput); }
        let image_stem = if mode == NfoExportMode::Offline
            && (stem.len() > MAX_IMAGE_STEM_BYTES || stem.contains([':', '?', '#', '%']) || stem.chars().any(char::is_control))
        {
            format!("mediaatlas-{}", &hash(stem.as_bytes())[..12])
        } else { stem.to_owned() };
        let corrected_identity = self.had_identity_correction(item.id).await?;
        let mut sources = Vec::new();
        for artwork in self.repo.list_artworks(item.id).await? {
            // A local image may be a sidecar of the previously misidentified work.
            if corrected_identity && artwork.provider_id.is_none() { continue; }
            let Some(kind) = kind(artwork.kind.as_str()) else { continue; };
            let reference = if let Some(reference) = artwork.source_url {
                if let Some(library) = reference.strip_prefix("library://") {
                    if mode == NfoExportMode::Online { continue; }
                    let (root, relative) = library.split_once('/').ok_or(RepositoryError::InvalidInput)?;
                    let root = Uuid::parse_str(root).map_err(|_| RepositoryError::InvalidInput)?;
                    let relative = local_path(relative)?;
                    if root != root_id || relative.parent() != Some(relative_parent) {
                        continue;
                    }
                    Some(relative.file_name().and_then(|name| name.to_str()).ok_or(RepositoryError::InvalidInput)?.to_owned())
                } else { Some(reference) }
            } else { None };
            sources.push(Source { kind, reference, cache: artwork.cache_path.zip(artwork.content_hash) });
        }
        if let Some(value) = metadata.iter().filter(|value| value.field_key == "nfo_artwork").max_by_key(|value| value.is_protected()) {
            if let Some(images) = value.value.as_array() {
                if images.len() > 1000 { return Err(RepositoryError::Provider(ProviderFailure::ResponseTooLarge)); }
                for image in images {
                    if let (Some(kind), Some(reference)) = (image["kind"].as_str().and_then(kind), image["url"].as_str()) {
                        sources.push(Source { kind, reference: Some(reference.into()), cache: None });
                    }
                }
            }
        }
        if [ArtworkKind::Poster, ArtworkKind::Backdrop].iter().any(|target| {
            !sources.iter().any(|source| source.kind == *target && if mode == NfoExportMode::Online {
                source.reference.as_deref().is_some_and(remote)
            } else { source.reference.is_some() || source.cache.is_some() })
        }) {
            let confirmed: Option<String> = sqlx::query_scalar("SELECT detail_json FROM video_merge_previews WHERE media_item_id=? AND state='succeeded' ORDER BY created_at DESC,id DESC LIMIT 1")
                .bind(item.id.to_string()).fetch_optional(self.repo.pool()).await?;
            if let Some(confirmed) = confirmed {
                let detail: VideoMetadata = serde_json::from_str(&confirmed).map_err(|_| RepositoryError::CorruptData)?;
                if !matches!((&detail, item.kind),
                    (VideoMetadata::Movie(_), MediaKind::Movie) | (VideoMetadata::Tv(_), MediaKind::TvSeries | MediaKind::TvSeason | MediaKind::TvEpisode)
                    | (VideoMetadata::Anime(_), MediaKind::Anime))
                { return Err(RepositoryError::CorruptData); }
                for image in detail.artwork() {
                    if let Some(kind) = kind(&image.kind) {
                        sources.push(Source { kind, reference: Some(image.url.clone()), cache: None });
                    }
                }
            }
        }
        let actors = crate::kodi_nfo::actor_thumbnail_urls(metadata);
        if mode == NfoExportMode::Offline && actors.len() > 200 {
            return Err(RepositoryError::Provider(ProviderFailure::ResponseTooLarge));
        }
        let mut output = PreparedExport {
            artwork: Vec::new(), actor_thumbs: (mode == NfoExportMode::Offline).then(BTreeMap::new), images: Vec::new(),
        };
        for target in [ArtworkKind::Poster, ArtworkKind::Backdrop] {
            for source in sources.iter().filter(|source| source.kind == target) {
                if mode == NfoExportMode::Online {
                    let Some(reference) = &source.reference else { continue; };
                    if !remote(reference) { continue; }
                    remote_provider(reference)?;
                    output.artwork.push((target, reference.clone()));
                    break;
                }
                let bytes = if let Some((path, digest)) = &source.cache {
                    self.export_cached_bytes(path, digest)?
                } else { None };
                let bytes = if let Some(bytes) = bytes { bytes } else {
                    let Some(reference) = &source.reference else { continue; };
                    if remote(reference) { self.export_remote_bytes(item, reference).await? }
                    else { let Some(bytes) = local_bytes(dir, reference)? else { continue; }; bytes }
                };
                let (format, _, _) = decode_image(&bytes).map_err(RepositoryError::Provider)?;
                let image_kind = if target == ArtworkKind::Poster { "poster" } else { "fanart" };
                let name = image_name(dir, &format!("{image_stem}-{image_kind}.{}", extension(format).ok_or(RepositoryError::InvalidInput)?), &bytes)?;
                output.artwork.push((target, name.clone()));
                output.push_image(PreparedImage { kind: image_kind.into(), name, bytes })?;
                break;
            }
        }
        for reference in actors {
            if mode == NfoExportMode::Online {
                if !remote(&reference) { continue; }
                remote_provider(&reference)?;
                continue;
            }
            if remote(&reference) { remote_provider(&reference)?; }
            let bytes = if remote(&reference) { self.export_remote_bytes(item, &reference).await? }
                else { local_bytes(dir, &reference)?.ok_or(RepositoryError::NotFound)? };
            let (format, _, _) = decode_image(&bytes).map_err(RepositoryError::Provider)?;
            let name = image_name(dir, &format!("{image_stem}-actor-{}.{}", &hash(reference.as_bytes())[..12], extension(format).ok_or(RepositoryError::InvalidInput)?), &bytes)?;
            output.actor_thumbs.as_mut().ok_or(RepositoryError::InvalidInput)?.insert(reference, name.clone());
            output.push_image(PreparedImage { kind: "actor_thumb".into(), name, bytes })?;
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    async fn fixture() -> (tempfile::TempDir, MediaOutputService, MediaItem, Dir) {
        let temp = crate::test_support::tempdir().unwrap();
        let parent = temp.path().join("电影/无敌破坏王/无敌破坏1");
        fs::create_dir_all(&parent).unwrap();
        fs::write(parent.join("无敌破坏1.mp4"), b"readonly source video").unwrap();
        let repo = LibraryRepository::open(&temp.path().join("library.sqlite")).await.unwrap();
        let now = Utc::now();
        let item = MediaItem { id: Uuid::new_v4(), kind: MediaKind::Movie, title: "无敌破坏1".into(),
            original_title: None, year: None, created_at: now, updated_at: now };
        repo.save_media(&item).await.unwrap();
        let service = MediaOutputService::new(repo, temp.path().join("cache"));
        let dir = open_absolute_dir(&parent, false).unwrap();
        (temp, service, item, dir)
    }

    fn value(item: &MediaItem, key: &str, value: serde_json::Value) -> MetadataValue {
        MetadataValue { id: Uuid::new_v4(), media_item_id: item.id, field_key: key.into(), locale: None,
            value, provider_id: Some("local".into()), source_url: None, fetched_at: None,
            confidence: None, is_user_modified: false, is_locked: false }
    }

    fn png() -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(1, 1).write_to(&mut buffer, ImageFormat::Png).unwrap();
        buffer.into_inner()
    }

    #[tokio::test]
    async fn online_mode_uses_https_references_without_downloads_or_media_writes() {
        let (temp, service, item, dir) = fixture().await;
        let values = [value(&item, "nfo_artwork", json!([
            {"kind":"poster","url":"https://image.tmdb.org/t/p/original/poster.jpg"},
            {"kind":"backdrop","url":"https://image.tmdb.org/t/p/original/fanart.jpg"}
        ])), value(&item, "cast", json!([{"name":"Actor","thumb":"https://image.tmdb.org/t/p/original/person.jpg"}]))];
        let prepared = service.prepare_kodi_images(&item, &values, "无敌破坏1", &dir, NfoExportMode::Online,
            Uuid::nil(), Path::new("电影/无敌破坏王/无敌破坏1")).await.unwrap();
        assert_eq!(prepared.artwork.len(), 2);
        assert!(prepared.artwork.iter().all(|(_, reference)| reference.starts_with("https://")));
        assert!(prepared.images.is_empty());
        assert!(prepared.actor_thumbs.is_none());
        assert!(!temp.path().join("cache").exists());
        assert_eq!(fs::read_dir(temp.path().join("电影/无敌破坏王/无敌破坏1")).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn reserved_characters_in_video_names_do_not_break_either_export_mode() {
        let (temp,service,item,dir)=fixture().await;
        let parent=Path::new("电影/无敌破坏王/无敌破坏1");
        let stem="100%中文#成功";
        let video=temp.path().join(parent).join(format!("{stem}.mp4"));
        fs::write(&video,b"readonly reserved-name video").unwrap();
        let remote_values=[value(&item,"nfo_artwork",json!([{"kind":"poster","url":"https://image.tmdb.org/t/p/original/poster.jpg"}]))];
        let online=service.prepare_kodi_images(&item,&remote_values,stem,&dir,NfoExportMode::Online,Uuid::nil(),parent).await.unwrap();
        assert_eq!(online.artwork,vec![(ArtworkKind::Poster,"https://image.tmdb.org/t/p/original/poster.jpg".into())]);
        assert!(online.images.is_empty());
        for path in ["poster.png","actor.png"] { fs::write(temp.path().join(parent).join(path),png()).unwrap(); }
        let local_values=[value(&item,"nfo_artwork",json!([{"kind":"poster","url":"poster.png"}])),
            value(&item,"cast",json!([{"name":"Actor","thumb":"actor.png"}]))];
        let offline=service.prepare_kodi_images(&item,&local_values,stem,&dir,NfoExportMode::Offline,Uuid::nil(),parent).await.unwrap();
        let prefix=format!("mediaatlas-{}",&hash(stem.as_bytes())[..12]);
        assert_eq!(offline.artwork,vec![(ArtworkKind::Poster,format!("{prefix}-poster.png"))]);
        assert_eq!(offline.images.len(),2);
        assert!(offline.images.iter().all(|image|image.name.starts_with(&prefix)&&!image.name.contains(['%','#'])));
        let nfo=crate::kodi_nfo::render_nfo(&item,&local_values,&[],&offline.artwork,offline.actor_thumbs.as_ref()).unwrap();
        assert!(nfo.contains(&format!("<thumb aspect=\"poster\">{prefix}-poster.png</thumb>")));
        assert!(nfo.contains(&format!("<thumb>{prefix}-actor-")));
        assert_eq!(fs::read(&video).unwrap(),b"readonly reserved-name video");
    }

    #[tokio::test]
    async fn online_mode_omits_local_images_and_uses_confirmed_https_fallback() {
        let (_temp, service, item, dir) = fixture().await;
        let values = [value(&item,"nfo_artwork",json!([
            {"kind":"poster","url":"poster.jpg"},{"kind":"backdrop","url":"fanart.jpg"}
        ])),value(&item,"cast",json!([{"name":"Local actor","thumb":".actors/person.jpg"}]))];
        let task = Uuid::new_v4();
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES(?,'search_providers','succeeded','{}',1,1)")
            .bind(task.to_string()).execute(service.repo.pool()).await.unwrap();
        sqlx::query("INSERT INTO video_scrape_sessions(task_id,query_json,jobs_json,created_at) VALUES(?,'{}','{}',1)")
            .bind(task.to_string()).execute(service.repo.pool()).await.unwrap();
        let detail = VideoMetadata::Movie(mediaatlas_domain::MovieDetail {
            identity: mediaatlas_domain::CandidateData { provider_id:"tmdb".into(),remote_id:"1".into(),title:item.title.clone(),
                original_title:None,year:None,aliases:Vec::new(),external_ids:Vec::new(),source_url:"https://www.themoviedb.org/movie/1".into(),language:"en-US".into() },
            fields:Vec::new(),runtime_minutes:None,artwork:vec![
                mediaatlas_domain::ArtworkCandidate {url:"https://image.tmdb.org/t/p/original/poster.jpg".into(),source_provider:"tmdb".into(),kind:"poster".into(),language:None},
                mediaatlas_domain::ArtworkCandidate {url:"https://image.tmdb.org/t/p/original/fanart.jpg".into(),source_provider:"tmdb".into(),kind:"backdrop".into(),language:None}
            ]
        });
        sqlx::query("INSERT INTO video_merge_previews(id,task_id,media_item_id,preview_json,detail_json,selected_artwork,expected_media_json,state,created_at) VALUES(?,?,?,'{}',?,'[]',?,'succeeded',1)")
            .bind(Uuid::new_v4().to_string()).bind(task.to_string()).bind(item.id.to_string())
            .bind(serde_json::to_string(&detail).unwrap()).bind(serde_json::to_string(&item).unwrap())
            .execute(service.repo.pool()).await.unwrap();
        let prepared=service.prepare_kodi_images(&item,&values,"Film",&dir,NfoExportMode::Online,
            Uuid::nil(),Path::new("电影/无敌破坏王/无敌破坏1")).await.unwrap();
        assert_eq!(prepared.artwork.len(),2);
        assert!(prepared.artwork.iter().all(|(_,reference)|reference.starts_with("https://image.tmdb.org/")));
        assert!(prepared.actor_thumbs.is_none());
        assert!(prepared.images.is_empty());
        let nfo=crate::kodi_nfo::render_nfo(&item,&values,&[],&prepared.artwork,prepared.actor_thumbs.as_ref()).unwrap();
        assert!(!nfo.contains(".actors/person.jpg"));
        assert!(nfo.contains("<thumb aspect=\"poster\">https://image.tmdb.org/"));
    }

    #[tokio::test]
    async fn offline_mode_prepares_real_format_relative_images_and_actor_portraits() {
        let (temp, service, item, dir) = fixture().await;
        let parent = temp.path().join("电影/无敌破坏王/无敌破坏1");
        fs::create_dir(parent.join(".actors")).unwrap();
        let bytes = png();
        for path in ["poster.jpg", "fanart.png", ".actors/person.png"] { fs::write(parent.join(path), &bytes).unwrap(); }
        let values = [value(&item, "nfo_artwork", json!([
            {"kind":"poster","url":"poster.jpg"},{"kind":"backdrop","url":"fanart.png"}
        ])), value(&item, "cast", json!([{"name":"Actor","thumb":".actors/person.png"}]))];
        let prepared = service.prepare_kodi_images(&item, &values, "无敌破坏1", &dir, NfoExportMode::Offline,
            Uuid::nil(), Path::new("电影/无敌破坏王/无敌破坏1")).await.unwrap();
        assert_eq!(prepared.artwork, vec![(ArtworkKind::Poster,"无敌破坏1-poster.png".into()),(ArtworkKind::Backdrop,"无敌破坏1-fanart.png".into())]);
        assert_eq!(prepared.images.len(), 3);
        assert!(prepared.images.iter().all(|image| image.bytes == bytes && image.name.ends_with(".png")));
        let actor = prepared.images.iter().find(|image| image.kind == "actor_thumb").unwrap();
        assert_eq!(prepared.actor_thumbs.as_ref().unwrap().get(".actors/person.png"), Some(&actor.name));
        assert!(prepared.images.iter().all(|image| !parent.join(&image.name).exists()));
        assert_eq!(fs::read(parent.join("无敌破坏1.mp4")).unwrap(), b"readonly source video");
    }

    #[tokio::test]
    async fn changed_offline_images_get_new_references_without_overwriting_existing_images() {
        let (temp,service,item,dir)=fixture().await;
        let parent=Path::new("电影/无敌破坏王/无敌破坏1");
        let bytes=png();
        let mut old_buffer=Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2,1).write_to(&mut old_buffer,ImageFormat::Png).unwrap();
        let old=old_buffer.into_inner();
        for name in ["new-poster.png","new-fanart.png","new-actor.png"] { fs::write(temp.path().join(parent).join(name),&bytes).unwrap(); }
        let actor_name=format!("Film-actor-{}.png",&hash(b"new-actor.png")[..12]);
        let existing=["Film-poster.png".to_owned(),"Film-fanart.png".to_owned(),actor_name];
        for name in &existing { fs::write(temp.path().join(parent).join(name),&old).unwrap(); }
        let values=[value(&item,"nfo_artwork",json!([
            {"kind":"poster","url":"new-poster.png"},{"kind":"backdrop","url":"new-fanart.png"}
        ])),value(&item,"cast",json!([{"name":"Actor","thumb":"new-actor.png"}]))];
        let prepared=service.prepare_kodi_images(&item,&values,"Film",&dir,NfoExportMode::Offline,Uuid::nil(),parent).await.unwrap();
        let suffix=format!("-{}.png",&hash(&bytes)[..12]);
        assert_eq!(prepared.images.len(),3);
        assert!(prepared.images.iter().all(|image|image.name.ends_with(&suffix)&&image.bytes==bytes));
        assert_eq!(prepared.artwork,vec![(ArtworkKind::Poster,format!("Film-poster{suffix}")),(ArtworkKind::Backdrop,format!("Film-fanart{suffix}"))]);
        let actor=prepared.images.iter().find(|image|image.kind=="actor_thumb").unwrap();
        assert_eq!(prepared.actor_thumbs.as_ref().unwrap().get("new-actor.png"),Some(&actor.name));
        let nfo=crate::kodi_nfo::render_nfo(&item,&values,&[],&prepared.artwork,prepared.actor_thumbs.as_ref()).unwrap();
        assert!(nfo.contains(&format!("<thumb aspect=\"poster\">Film-poster{suffix}</thumb>")));
        assert!(nfo.contains(&format!("<thumb>{}</thumb>",actor.name)));
        for name in &existing { assert_eq!(fs::read(temp.path().join(parent).join(name)).unwrap(),old); }
        for image in &prepared.images { fs::write(temp.path().join(parent).join(&image.name),&image.bytes).unwrap(); }
        let repeated=service.prepare_kodi_images(&item,&values,"Film",&dir,NfoExportMode::Offline,Uuid::nil(),parent).await.unwrap();
        assert_eq!(prepared.artwork,repeated.artwork);
        assert_eq!(prepared.actor_thumbs,repeated.actor_thumbs);
        assert_eq!(prepared.images.iter().map(|image|&image.name).collect::<Vec<_>>(),repeated.images.iter().map(|image|&image.name).collect::<Vec<_>>());
        fs::write(temp.path().join(parent).join(format!("Film-poster{suffix}")),&old).unwrap();
        assert!(matches!(service.prepare_kodi_images(&item,&values,"Film",&dir,NfoExportMode::Offline,Uuid::nil(),parent).await,Err(RepositoryError::Conflict)));
        assert_eq!(fs::read(temp.path().join(parent).join("Film-poster.png")).unwrap(),old);
    }

    #[tokio::test]
    async fn same_offline_image_reuses_its_normal_name_and_long_utf8_stems_get_bounded_aliases() {
        let (temp,service,item,dir)=fixture().await;
        let parent=Path::new("电影/无敌破坏王/无敌破坏1");
        let bytes=png();
        for name in ["poster.png","Film-poster.png"] { fs::write(temp.path().join(parent).join(name),&bytes).unwrap(); }
        let values=[value(&item,"nfo_artwork",json!([{"kind":"poster","url":"poster.png"}])),
            value(&item,"cast",json!([{"name":"Actor","thumb":"poster.png"}]))];
        let normal=service.prepare_kodi_images(&item,&values,"Film",&dir,NfoExportMode::Offline,Uuid::nil(),parent).await.unwrap();
        assert_eq!(normal.artwork,vec![(ArtworkKind::Poster,"Film-poster.png".into())]);
        let stem="中文".repeat(40);
        assert_eq!(stem.len(),240);
        let long=service.prepare_kodi_images(&item,&values,&stem,&dir,NfoExportMode::Offline,Uuid::nil(),parent).await.unwrap();
        let prefix=format!("mediaatlas-{}",&hash(stem.as_bytes())[..12]);
        assert!(long.images.iter().all(|image|image.name.starts_with(&prefix)&&image.name.len()<=255));
        assert_eq!(long.artwork,vec![(ArtworkKind::Poster,format!("{prefix}-poster.png"))]);
    }

    #[tokio::test]
    async fn offline_mode_reuses_valid_cache_without_network_or_enabled_provider() {
        let (temp, service, item, dir) = fixture().await;
        fs::create_dir(temp.path().join("cache")).unwrap();
        let bytes = png();
        let digest = hash(&bytes);
        let image = temp.path().join("cache").join(format!("{digest}.png"));
        fs::write(&image, &bytes).unwrap();
        let url = "https://image.tmdb.org/t/p/original/person.png";
        let cached = CachedArtwork { cache_path: image.to_string_lossy().into_owned(),
            thumbnail_path: temp.path().join("cache").join(format!("{digest}.thumb.png")).to_string_lossy().into_owned(),
            content_hash: digest, width: 1, height: 1, mime: "image/png".into() };
        fs::write(temp.path().join("cache").join(format!("url-{}.json",hash(url.as_bytes()))), serde_json::to_vec(&cached).unwrap()).unwrap();
        let values = [value(&item,"nfo_artwork",json!([{"kind":"poster","url":url}])), value(&item,"cast",json!([{"name":"Actor","thumb":url}]))];
        let prepared = service.prepare_kodi_images(&item,&values,"Film",&dir,NfoExportMode::Offline,
            Uuid::nil(),Path::new("电影/无敌破坏王/无敌破坏1")).await.unwrap();
        assert_eq!(prepared.images.len(),2);
        assert!(prepared.images.iter().all(|image| image.bytes == bytes));
        fs::write(&image,b"corrupted cache").unwrap();
        assert!(matches!(service.prepare_kodi_images(&item,&values,"Film",&dir,NfoExportMode::Offline,
            Uuid::nil(),Path::new("电影/无敌破坏王/无敌破坏1")).await,Err(RepositoryError::Conflict)));
    }

    #[tokio::test]
    async fn offline_uncached_remote_images_fail_without_a_network_service() {
        let (_temp,service,item,dir)=fixture().await;
        let values=[value(&item,"nfo_artwork",json!([{"kind":"poster","url":"https://image.tmdb.org/t/p/original/missing.png"}]))];
        assert!(matches!(service.prepare_kodi_images(&item,&values,"Film",&dir,NfoExportMode::Offline,
            Uuid::nil(),Path::new("电影/无敌破坏王/无敌破坏1")).await,
            Err(RepositoryError::Provider(ProviderFailure::InvalidConfiguration))));
    }

    #[test]
    fn only_supported_https_hosts_and_confined_relative_paths_are_accepted() {
        for url in ["http://image.tmdb.org/image.jpg","https://127.0.0.1/image.jpg","https://example.com/image.jpg",
            "https://user:secret@image.tmdb.org/image.jpg","https://image.tmdb.org:444/image.jpg",
            "https://image.tmdb.org/image.jpg?token=secret","https://image.tmdb.org/image.jpg#fragment"] {
            assert!(remote_provider(url).is_err(),"{url}");
        }
        for reference in ["../outside.jpg","/outside.jpg","C:/outside.jpg","a\\b.jpg","a/%2e%2e/b.jpg"] {
            assert!(local_path(reference).is_err(),"{reference}");
        }
        assert_eq!(remote_provider("https://static.tvmaze.com/image.jpg").unwrap(),"tvmaze");
        assert_eq!(remote_provider("https://s4.anilist.co/image.jpg").unwrap(),"anilist");
        assert_eq!(remote_provider("https://lain.bgm.tv/pic/cover/l/image.jpg").unwrap(),"bangumi");
        assert_eq!(remote_provider("https://img9.doubanio.com/view/photo/l/public/image.jpg").unwrap(),"douban");
        assert_eq!(remote_provider("https://m.media-amazon.com/images/M/image.jpg").unwrap(),"omdb");
        assert!(remote_provider("https://img9.doubanio.com.evil.test/image.jpg").is_err());
        assert!(local_path(".actors/person.jpg").is_ok());
    }

    #[tokio::test]
    async fn actor_image_limits_reject_instead_of_silently_truncating() {
        let (_temp,service,item,dir)=fixture().await;
        let people=(0..201).map(|index|json!({"name":format!("Actor {index}"),"thumb":format!("https://image.tmdb.org/t/p/original/{index}.jpg")})).collect::<Vec<_>>();
        let values=[value(&item,"cast",json!(people))];
        assert!(matches!(service.prepare_kodi_images(&item,&values,"Film",&dir,NfoExportMode::Offline,
            Uuid::nil(),Path::new("电影/无敌破坏王/无敌破坏1")).await,
            Err(RepositoryError::Provider(ProviderFailure::ResponseTooLarge))));
        let online=service.prepare_kodi_images(&item,&values,"Film",&dir,NfoExportMode::Online,
            Uuid::nil(),Path::new("电影/无敌破坏王/无敌破坏1")).await.unwrap();
        assert!(online.actor_thumbs.is_none());
        assert!(online.images.is_empty());
        let nfo=crate::kodi_nfo::render_nfo(&item,&values,&[],&online.artwork,online.actor_thumbs.as_ref()).unwrap();
        assert_eq!(nfo.matches("<actor>").count(),201);
        assert_eq!(nfo.matches("<thumb>https://image.tmdb.org/").count(),201);
    }

    #[tokio::test]
    async fn library_artwork_must_match_the_current_source_root_and_parent() {
        let (temp,service,item,dir)=fixture().await;
        let root=Uuid::new_v4();
        let parent=Path::new("电影/无敌破坏王/无敌破坏1");
        fs::write(temp.path().join(parent).join("poster.png"),png()).unwrap();
        let mut artwork=mediaatlas_domain::Artwork {id:Uuid::new_v4(),media_item_id:item.id,kind:ArtworkKind::Poster,
            source_url:None,cache_path:None,content_hash:None,provider_id:Some("local".into()),width:None,height:None,created_at:Utc::now()};
        for source in [format!("library://{root}/电影/无敌破坏王/另一部/poster.png"),
            format!("library://{}/电影/无敌破坏王/无敌破坏1/poster.png",Uuid::new_v4())] {
            artwork.source_url=Some(source);
            service.repo.save_artwork(&artwork).await.unwrap();
            let prepared=service.prepare_kodi_images(&item,&[],"Film",&dir,NfoExportMode::Offline,root,parent).await.unwrap();
            assert!(prepared.artwork.is_empty());
            assert!(prepared.images.is_empty());
        }
        artwork.source_url=Some(format!("library://{root}/电影/无敌破坏王/无敌破坏1/poster.png"));
        service.repo.save_artwork(&artwork).await.unwrap();
        let prepared=service.prepare_kodi_images(&item,&[],"Film",&dir,NfoExportMode::Offline,root,parent).await.unwrap();
        assert_eq!(prepared.artwork,vec![(ArtworkKind::Poster,"Film-poster.png".into())]);
        assert_eq!(prepared.images.len(),1);
    }

    #[tokio::test]
    async fn corrected_identity_does_not_reuse_the_previous_movies_local_sidecars() {
        let (temp,service,item,dir)=fixture().await;
        let root=Uuid::new_v4();
        let parent=Path::new("电影/无敌破坏王/无敌破坏1");
        let mut old_buffer=Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2,1).write_to(&mut old_buffer,ImageFormat::Png).unwrap();
        let old=old_buffer.into_inner();
        for (kind,name) in [(ArtworkKind::Poster,"wrong-poster.png"),(ArtworkKind::Backdrop,"wrong-fanart.png")] {
            fs::write(temp.path().join(parent).join(name),&old).unwrap();
            service.repo.save_artwork(&mediaatlas_domain::Artwork {
                id:Uuid::new_v4(),media_item_id:item.id,kind,provider_id:None,
                source_url:Some(format!("library://{root}/{}/{name}",parent.to_str().unwrap())),
                cache_path:None,content_hash:None,width:None,height:None,created_at:Utc::now(),
            }).await.unwrap();
        }
        let urls=["https://image.tmdb.org/t/p/original/correct-poster.png","https://image.tmdb.org/t/p/original/correct-fanart.png"];
        let detail=VideoMetadata::Movie(mediaatlas_domain::MovieDetail {
            identity:mediaatlas_domain::CandidateData {provider_id:"tmdb".into(),remote_id:"2".into(),title:item.title.clone(),
                original_title:None,year:None,aliases:Vec::new(),external_ids:vec![mediaatlas_domain::RemoteId {namespace:"tmdb".into(),value:"2".into()}],
                source_url:"https://www.themoviedb.org/movie/2".into(),language:"zh-CN".into()},
            fields:Vec::new(),runtime_minutes:None,
            artwork:vec![
                mediaatlas_domain::ArtworkCandidate {url:urls[0].into(),source_provider:"tmdb".into(),kind:"poster".into(),language:None},
                mediaatlas_domain::ArtworkCandidate {url:urls[1].into(),source_provider:"tmdb".into(),kind:"backdrop".into(),language:None},
            ],
        });
        let task=Uuid::new_v4();
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES(?,'search_providers','succeeded','{}',1,1)")
            .bind(task.to_string()).execute(service.repo.pool()).await.unwrap();
        sqlx::query("INSERT INTO video_scrape_sessions(task_id,query_json,jobs_json,created_at) VALUES(?,'{}','{}',1)")
            .bind(task.to_string()).execute(service.repo.pool()).await.unwrap();
        let preview=json!({"external_ids":[{"namespace":"tmdb","old":["1"],"new":["2"]}]});
        sqlx::query("INSERT INTO video_merge_previews(id,task_id,media_item_id,preview_json,detail_json,selected_artwork,expected_media_json,state,created_at) VALUES(?,?,?,?,?,'[]',?,'succeeded',1)")
            .bind(Uuid::new_v4().to_string()).bind(task.to_string()).bind(item.id.to_string()).bind(preview.to_string())
            .bind(serde_json::to_string(&detail).unwrap()).bind(serde_json::to_string(&item).unwrap())
            .execute(service.repo.pool()).await.unwrap();
        let bytes=png();
        let digest=hash(&bytes);
        let cache=temp.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let image=cache.join(format!("{digest}.png"));
        fs::write(&image,&bytes).unwrap();
        let cached=CachedArtwork {cache_path:image.to_string_lossy().into_owned(),
            thumbnail_path:cache.join(format!("{digest}.thumb.png")).to_string_lossy().into_owned(),
            content_hash:digest,width:1,height:1,mime:"image/png".into()};
        for url in urls {
            fs::write(cache.join(format!("url-{}.json",hash(url.as_bytes()))),serde_json::to_vec(&cached).unwrap()).unwrap();
        }
        let online=service.prepare_kodi_images(&item,&[],"Film",&dir,NfoExportMode::Online,root,parent).await.unwrap();
        assert_eq!(online.artwork,vec![(ArtworkKind::Poster,urls[0].into()),(ArtworkKind::Backdrop,urls[1].into())]);
        assert!(online.images.is_empty());
        let offline=service.prepare_kodi_images(&item,&[],"Film",&dir,NfoExportMode::Offline,root,parent).await.unwrap();
        assert_eq!(offline.artwork,vec![(ArtworkKind::Poster,"Film-poster.png".into()),(ArtworkKind::Backdrop,"Film-fanart.png".into())]);
        assert_eq!(offline.images.len(),2);
        assert!(offline.images.iter().all(|image|image.bytes==bytes));
        let nfo=crate::kodi_nfo::render_nfo(&item,&[],&[],&offline.artwork,offline.actor_thumbs.as_ref()).unwrap();
        assert!(!nfo.contains("wrong-"));
        for name in ["wrong-poster.png","wrong-fanart.png"] {
            assert_eq!(fs::read(temp.path().join(parent).join(name)).unwrap(),old);
        }
        assert!(!temp.path().join(parent).join("Film-poster.png").exists());
    }

    #[test]
    fn total_image_payload_is_bounded() {
        let mut prepared=PreparedExport {artwork:Vec::new(),actor_thumbs:None,images:Vec::new()};
        for index in 0..4 { prepared.push_image(PreparedImage {kind:"poster".into(),name:format!("{index}.png"),bytes:vec![0;MAX_IMAGE]}).unwrap(); }
        assert!(matches!(prepared.push_image(PreparedImage {kind:"actor_thumb".into(),name:"excess.png".into(),bytes:vec![0]}),
            Err(RepositoryError::Provider(ProviderFailure::ResponseTooLarge))));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_image_links_cannot_escape_the_source_directory() {
        let (temp,_service,_item,dir)=fixture().await;
        let outside=temp.path().join("outside.png");
        fs::write(&outside,png()).unwrap();
        std::os::unix::fs::symlink(&outside,temp.path().join("电影/无敌破坏王/无敌破坏1/linked.png")).unwrap();
        assert!(local_bytes(&dir,"linked.png").is_err());
        assert!(local_bytes(&dir,"../outside.png").is_err());
    }
}
