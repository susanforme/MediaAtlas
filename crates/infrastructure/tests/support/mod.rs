//! Filesystem fixtures use the same canonical, native Root paths as BrowseService.

use mediaatlas_domain::PathStyle;
use std::{io, path::Path};

pub fn canonical_root_path(path: &Path) -> io::Result<String> {
    let path = std::fs::canonicalize(path)?;
    let path = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Non-UTF-8 fixture path"))?;
    if cfg!(windows) {
        if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
            return Ok(format!(r"\\{unc}"));
        }
        if let Some(disk) = path.strip_prefix(r"\\?\") {
            return Ok(disk.to_owned());
        }
    }
    Ok(path.to_owned())
}

pub fn native_path_style() -> PathStyle {
    if cfg!(windows) {
        PathStyle::Windows
    } else {
        PathStyle::Posix
    }
}

pub fn tempdir() -> io::Result<tempfile::TempDir> {
    // macOS commonly exposes /var through a symlink. Register the canonical
    // directory once, leaving every later descendant no-follow check intact.
    tempdir_in(&std::env::temp_dir())
}

fn tempdir_in(parent: &Path) -> io::Result<tempfile::TempDir> {
    tempfile::tempdir_in(canonical_root_path(parent)?)
}

#[cfg(unix)]
#[test]
fn aliased_temporary_parent_is_canonicalized_before_creating_a_root() {
    let parent = tempdir().unwrap();
    let actual = parent.path().join("actual");
    std::fs::create_dir(&actual).unwrap();
    let alias = parent.path().join("alias");
    std::os::unix::fs::symlink(&actual, &alias).unwrap();
    let dir = tempdir_in(&alias).unwrap();
    assert_eq!(dir.path().parent().unwrap(), actual);
    assert_eq!(
        dir.path().to_str().unwrap(),
        canonical_root_path(dir.path()).unwrap()
    );
}

#[test]
fn temporary_roots_use_native_canonical_paths() {
    let dir = tempdir().unwrap();
    assert_eq!(
        dir.path().to_str().unwrap(),
        canonical_root_path(dir.path()).unwrap()
    );
    assert!(mediaatlas_domain::normalize_root_path(
        dir.path().to_str().unwrap(),
        native_path_style()
    )
    .is_ok());
}
