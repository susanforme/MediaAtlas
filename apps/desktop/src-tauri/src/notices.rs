//! Explicit offline notice export, before any GUI, database or network startup.
use std::{ffi::OsString, fs, io, io::Write, path::Path};

include!(concat!(env!("OUT_DIR"), "/embedded-notices.rs"));

/// `false` means normal GUI startup; `true` means the export completed.
pub fn handle_args(args: impl IntoIterator<Item = OsString>) -> io::Result<bool> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Ok(false);
    };
    let target = args.next();
    if command != "--export-licenses" || target.is_none() || args.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Usage: MediaAtlas --export-licenses <new-directory>",
        ));
    }
    export(Path::new(&target.unwrap()))?;
    Ok(true)
}

fn export(directory: &Path) -> io::Result<()> {
    // An existing directory (even an empty one) or symlink must never be reused.
    // Parent directories are user-selected and must already exist.
    fs::create_dir(directory)?;
    for (relative, bytes) in EMBEDDED_NOTICES {
        let destination = directory.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        file.write_all(bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Temporary(std::path::PathBuf);
    impl Temporary {
        fn new() -> Self {
            let value = Self(std::env::temp_dir().join(format!(
                "mediaatlas-notices-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )));
            fs::create_dir(&value.0).unwrap();
            value
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn no_arguments_request_normal_startup_without_export() {
        assert!(!handle_args(Vec::<OsString>::new()).unwrap());
    }

    #[test]
    fn invalid_arguments_never_create_an_output_directory() {
        let parent = Temporary::new();
        let target = parent.0.join("export");
        for args in [
            vec![OsString::from("--export-licenses")],
            vec![OsString::from("--unknown"), target.as_os_str().to_owned()],
            vec![
                OsString::from("--export-licenses"),
                target.as_os_str().to_owned(),
                OsString::from("extra"),
            ],
        ] {
            assert_eq!(
                handle_args(args).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
            assert!(!target.exists());
        }
    }

    #[test]
    fn exports_every_original_byte_and_refuses_existing_destinations() {
        let parent = Temporary::new();
        let target = parent.0.join("export");
        assert!(handle_args([
            OsString::from("--export-licenses"),
            target.as_os_str().to_owned()
        ])
        .unwrap());
        for (relative, bytes) in EMBEDDED_NOTICES {
            assert_eq!(fs::read(target.join(relative)).unwrap(), *bytes);
        }
        let marker = target.join("LICENSE.md");
        fs::write(&marker, b"user-edited sentinel").unwrap();
        assert!(export(&target).is_err());
        assert_eq!(fs::read(&marker).unwrap(), b"user-edited sentinel");
        assert!(export(&marker).is_err());
    }
}
