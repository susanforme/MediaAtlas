//! Explicitly selected NFO files and bounded artwork are previewed without library or writes.
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use mediaatlas_media_scanner::{parse_nfo, LocalMetadata};
use mediaatlas_domain::{ProviderConfig, ProviderJob};
use crate::network::{CancellationToken, EngineKind, NetworkEngine, RequestSpec};
use serde::Serialize;
use std::{io::Read, path::{Component, Path, PathBuf}};

const MAX_NFO_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ARTWORK_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct NfoDocument {
    pub name: String,
    pub metadata: LocalMetadata,
    pub source: String,
    pub format: &'static str,
    pub poster_uri: Option<String>,
    pub backdrop_uri: Option<String>,
}

pub fn is_nfo(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("nfo"))
}

pub fn read_preview(path: &Path) -> Result<NfoDocument, &'static str> {
    read_selected(path).map(|(document, _)| document)
}

fn read_selected(path: &Path) -> Result<(NfoDocument, Dir), &'static str> {
    if !path.is_absolute() || !is_nfo(path) {
        return Err("invalid_input");
    }
    let parent = path.parent().ok_or("invalid_input")?;
    let name = path.file_name().ok_or("invalid_input")?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|_| "local_file_unreadable")?;
    let selected = dir
        .symlink_metadata(name)
        .map_err(|_| "local_file_unreadable")?;
    if !selected.is_file() || selected.file_type().is_symlink() {
        return Err("invalid_input");
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        // A file replaced by a FIFO between metadata and open must not block the viewer.
        options.custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32);
    }
    let file = dir
        .open_with(name, &options)
        .map_err(|_| "local_file_unreadable")?;
    let info = file.metadata().map_err(|_| "local_file_unreadable")?;
    if !info.is_file() {
        return Err("invalid_input");
    }
    if info.len() > MAX_NFO_BYTES {
        return Err("local_metadata_too_large");
    }
    let mut bytes = Vec::new();
    file.take(MAX_NFO_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "local_file_unreadable")?;
    if bytes.len() as u64 > MAX_NFO_BYTES {
        return Err("local_metadata_too_large");
    }
    let source = String::from_utf8(bytes).map_err(|_| "invalid_local_metadata")?;
    let xml = source
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with('<');
    let metadata = if xml {
        parse_nfo(source.trim_start_matches('\u{feff}')).map_err(|error| error.code())?
    } else {
        LocalMetadata::default()
    };
    Ok((NfoDocument {
        name: name.to_string_lossy().into_owned(),
        metadata,
        source,
        format: if xml { "xml" } else { "text" },
        poster_uri: None,
        backdrop_uri: None,
    }, dir))
}

/// Only OS-selected paths reach this entry point; it exposes no arbitrary-path IPC.
pub async fn read_preview_with_artwork(path: PathBuf) -> Result<NfoDocument, &'static str> {
    let (mut document, directory) = tokio::task::spawn_blocking(move || read_selected(&path))
        .await.map_err(|_| "local_file_unreadable")??;
    let references = |kind: &str| -> Vec<String> {
        document.metadata.fields.get("nfo_artwork").and_then(|value| value.as_array())
            .into_iter().flatten().filter(|image| image["kind"].as_str() == Some(kind))
            .filter_map(|image| image["url"].as_str().map(str::to_owned)).take(3).collect()
    };
    let posters = references("poster");
    let backdrops = references("backdrop");
    if posters.is_empty() && backdrops.is_empty() { return Ok(document); }
    let engine = NetworkEngine::new(2).ok();
    let (poster, backdrop) = tokio::join!(
        artwork(&directory, &posters, engine.as_ref()),
        artwork(&directory, &backdrops, engine.as_ref()),
    );
    document.poster_uri = poster;
    document.backdrop_uri = backdrop;
    Ok(document)
}

async fn artwork(directory: &Dir, references: &[String], engine: Option<&NetworkEngine>) -> Option<String> {
    // A failed or slow image must not prevent the NFO text from opening.
    tokio::time::timeout(std::time::Duration::from_secs(6), async {
        for reference in references {
            let bytes = if reqwest::Url::parse(reference).ok().is_some_and(|url| url.scheme() == "https") {
                let Some(engine) = engine else { continue; };
                let job = ProviderJob {
                    id: uuid::Uuid::new_v4(), provider_id: "nfo-preview".into(),
                    media_item_id: None, media_kind: None, file_identity: None, region: None,
                    correlation_id: uuid::Uuid::new_v4(), operation: "image_preview".into(),
                    query: String::new(), language: String::new(), created_at: chrono::Utc::now(),
                    config: ProviderConfig { enabled: true, timeout_ms: 2500, max_retries: 0,
                        max_response_bytes: MAX_ARTWORK_BYTES as usize, ..ProviderConfig::default() },
                };
                let Ok(response) = engine.execute(&job, RequestSpec::get(reference, EngineKind::Image), &CancellationToken::new()).await else { continue; };
                response.body
            } else {
                let parent = directory.try_clone().ok()?;
                let relative = reference.clone();
                let Ok(Some(bytes)) = tokio::task::spawn_blocking(move || local_artwork(&parent, &relative)).await else { continue; };
                bytes
            };
            if let Ok(uri) = crate::media_output::preview_image(bytes).await { return Some(uri); }
        }
        None
    }).await.ok().flatten()
}

fn local_artwork(parent: &Dir, reference: &str) -> Option<Vec<u8>> {
    // Reject other schemes, drive/UNC paths, traversal and links at every component.
    if reference.contains([':', '\\', '\0']) { return None; }
    let relative = Path::new(reference);
    if relative.components().any(|part| !matches!(part, Component::Normal(_) | Component::CurDir)) { return None; }
    let mut dir = parent.try_clone().ok()?;
    for component in relative.parent()?.components() {
        if component == Component::CurDir { continue; }
        let Component::Normal(name) = component else { return None; };
        dir = dir.open_dir_nofollow(name).ok()?;
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32);
    }
    let file = dir.open_with(relative.file_name()?, &options).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_ARTWORK_BYTES { return None; }
    let mut bytes = Vec::new();
    file.take(MAX_ARTWORK_BYTES + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= MAX_ARTWORK_BYTES).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn displays_referenced_local_poster_and_backdrop_without_writes() {
        let directory = tempfile::tempdir().unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(24, 16).write_to(&mut png, image::ImageFormat::Png).unwrap();
        std::fs::write(directory.path().join("poster.png"), png.get_ref()).unwrap();
        std::fs::write(directory.path().join("fanart.png"), png.get_ref()).unwrap();
        let path = directory.path().join("movie.nfo");
        let source = "<movie><title>Atlas</title><thumb aspect=\"poster\">./poster.png</thumb><fanart><thumb>fanart.png</thumb></fanart></movie>";
        std::fs::write(&path, source).unwrap();
        let document = read_preview_with_artwork(path.clone()).await.unwrap();
        assert!(document.poster_uri.unwrap().starts_with("data:image/png;base64,"));
        assert!(document.backdrop_uri.unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), source);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 3);
    }

    #[tokio::test]
    async fn bad_images_and_private_urls_do_not_hide_the_document() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("movie.nfo");
        std::fs::write(directory.path().join("bad.png"), b"<svg><script/></svg>").unwrap();
        std::fs::write(&path, "<movie><title>Atlas</title><thumb>bad.png</thumb><fanart><thumb>https://127.0.0.1/private.png</thumb></fanart></movie>").unwrap();
        let document = read_preview_with_artwork(path).await.unwrap();
        assert_eq!(document.metadata.fields["title"], "Atlas");
        assert!(document.poster_uri.is_none());
        assert!(document.backdrop_uri.is_none());
    }

    #[test]
    fn image_paths_stay_inside_the_selected_directory_and_reject_links() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("media");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(directory.path().join("private.png"), b"private").unwrap();
        let dir = Dir::open_ambient_dir(&root, ambient_authority()).unwrap();
        for reference in ["../private.png", "/private.png", "file:///private.png", "C:\\private.png", "//server/private.png", "data:image/png;base64,abc"] {
            assert!(local_artwork(&dir, reference).is_none(), "{reference}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(directory.path().join("private.png"), root.join("link.png")).unwrap();
            std::os::unix::fs::symlink(directory.path(), root.join("linked")).unwrap();
            assert!(local_artwork(&dir, "link.png").is_none());
            assert!(local_artwork(&dir, "linked/private.png").is_none());
        }
        let file = std::fs::File::create(root.join("large.png")).unwrap();
        file.set_len(MAX_ARTWORK_BYTES + 1).unwrap();
        assert!(local_artwork(&dir, "large.png").is_none());
    }

    #[test]
    fn renders_only_selected_file_without_importing_or_rewriting() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("movie.nfo");
        let content = b"<movie><title>Atlas</title><year>2026</year><plot>&lt;b&gt;Text&lt;/b&gt;</plot></movie>";
        std::fs::write(&file, content).unwrap();
        let preview = read_preview(&file).unwrap();
        assert_eq!(preview.metadata.fields["title"], "Atlas");
        assert_eq!(preview.metadata.fields["overview"], "<b>Text</b>");
        assert_eq!(std::fs::read(&file).unwrap(), content);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn rejects_unsafe_xml_oversized_files_and_unselected_extensions() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("unsafe.nfo");
        std::fs::write(
            &file,
            "<!DOCTYPE movie [<!ENTITY secret SYSTEM 'file:///private'>]><movie/>",
        )
        .unwrap();
        assert_eq!(read_preview(&file).unwrap_err(), "unsafe_xml");
        std::fs::write(&file, vec![b'x'; MAX_NFO_BYTES as usize + 1]).unwrap();
        assert_eq!(read_preview(&file).unwrap_err(), "local_metadata_too_large");
        assert_eq!(
            read_preview(&directory.path().join("secret.txt")).unwrap_err(),
            "invalid_input"
        );
    }

    #[test]
    fn displays_plain_text_and_does_not_follow_a_selected_symlink() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("notes.NFO");
        std::fs::write(&file, "plain text\nNFO").unwrap();
        let preview = read_preview(&file).unwrap();
        assert_eq!(preview.format, "text");
        assert_eq!(preview.source, "plain text\nNFO");
        #[cfg(unix)]
        {
            let link = directory.path().join("alias.nfo");
            std::os::unix::fs::symlink(&file, &link).unwrap();
            assert!(read_preview(&link).is_err());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn refuses_named_pipes_before_opening_them() {
        let directory = tempfile::tempdir().unwrap();
        let pipe = directory.path().join("blocked.nfo");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &pipe,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .unwrap();
        assert_eq!(read_preview(&pipe).unwrap_err(), "invalid_input");
    }
}
