//! Build-only inventory for the executable's complete, byte-for-byte notices.
//! Uses only std so the same code can be exercised without a desktop toolchain.
use std::{fs, io, path::Path};

fn collect(directory: &Path, files: &mut Vec<std::path::PathBuf>) -> io::Result<()> {
    // Watching every directory catches additions and removals, not only edits
    // to files that happened to exist during the previous build.
    println!("cargo:rerun-if-changed={}", directory.display());
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "License symlinks are not supported",
            ));
        }
        if kind.is_dir() {
            collect(&entry.path(), files)?;
        } else if kind.is_file() {
            files.push(entry.path());
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unexpected license file type",
            ));
        }
    }
    Ok(())
}

pub fn generate(repository: &Path, output: &Path) -> io::Result<()> {
    let root = fs::canonicalize(repository)?;
    let mut files = vec![
        root.join("LICENSE-STATUS.md"),
        root.join("THIRD_PARTY_NOTICES.md"),
        root.join("docs/dependency-licenses.json"),
    ];
    collect(&root.join("docs/third-party-licenses"), &mut files)?;
    if files.len() <= 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Full third-party license texts are missing",
        ));
    }
    files.sort();
    let mut generated = String::from("static EMBEDDED_NOTICES: &[(&str, &[u8])] = &[\n");
    for path in files {
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "License material is empty or not a regular file",
            ));
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let relative = path.strip_prefix(&root).map_err(io::Error::other)?;
        let relative = relative
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "License path is not UTF-8"))?
            .replace('\\', "/");
        if relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid license path",
            ));
        }
        let source = path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "License source path is not UTF-8",
            )
        })?;
        generated.push_str(&format!(
            "    ({relative:?}, include_bytes!({source:?})),\n"
        ));
    }
    generated.push_str("];\n");
    fs::write(output.join("embedded-notices.rs"), generated)
}
