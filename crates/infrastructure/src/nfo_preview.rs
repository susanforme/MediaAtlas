//! Explicitly selected NFO files are previewed without library, network or write operations.
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use mediaatlas_media_scanner::{parse_nfo, LocalMetadata};
use serde::Serialize;
use std::{io::Read, path::Path};

const MAX_NFO_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct NfoDocument {
    pub name: String,
    pub metadata: LocalMetadata,
    pub source: String,
    pub format: &'static str,
}

pub fn is_nfo(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("nfo"))
}

pub fn read_preview(path: &Path) -> Result<NfoDocument, &'static str> {
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
    Ok(NfoDocument {
        name: name.to_string_lossy().into_owned(),
        metadata,
        source,
        format: if xml { "xml" } else { "text" },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
