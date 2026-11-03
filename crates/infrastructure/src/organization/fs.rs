use crate::library::RepositoryError;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, File, Metadata, OpenOptions},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

thread_local! {static CANCEL:RefCell<Option<Arc<AtomicBool>>>=const{RefCell::new(None)};static ACCESS:RefCell<Option<crate::media_output::AdultOutputAccess>>=const{RefCell::new(None)};static DEADLINE:Cell<Option<std::time::Instant>>=const{Cell::new(None)};}
pub(super) fn with_budget<T>(
    access: Option<crate::media_output::AdultOutputAccess>,
    f: impl FnOnce() -> T,
) -> T {
    ACCESS.with(|a| *a.borrow_mut() = access);
    DEADLINE.with(|d| {
        d.set(Some(
            std::time::Instant::now() + std::time::Duration::from_secs(60),
        ))
    });
    let result = f();
    ACCESS.with(|a| *a.borrow_mut() = None);
    DEADLINE.with(|d| d.set(None));
    result
}
pub(super) fn with_cancel<T>(
    cancel: Arc<AtomicBool>,
    access: Option<crate::media_output::AdultOutputAccess>,
    f: impl FnOnce() -> T,
) -> T {
    CANCEL.with(|c| *c.borrow_mut() = Some(cancel));
    ACCESS.with(|a| *a.borrow_mut() = access);
    let result = f();
    CANCEL.with(|c| *c.borrow_mut() = None);
    ACCESS.with(|a| *a.borrow_mut() = None);
    result
}
#[cfg(test)]
pub(super) fn expire_budget() {
    DEADLINE.with(|d| d.set(Some(std::time::Instant::now())));
}
pub(super) fn cancellation() -> Result<()> {
    ACCESS.with(|a| a.borrow().as_ref().map(|v| v.ensure()).transpose())?;
    if DEADLINE.with(|d| {
        d.get()
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
    }) {
        return Err(RepositoryError::Conflict);
    }
    if CANCEL.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|v| v.load(Ordering::SeqCst))
    }) {
        Err(RepositoryError::Conflict)
    } else {
        Ok(())
    }
}
type DirectoryStates = (Vec<(String, Option<String>)>, Vec<Dir>);
pub(super) type Result<T> = std::result::Result<T, RepositoryError>;
pub(super) fn err(_: std::io::Error) -> RepositoryError {
    RepositoryError::Conflict
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Stamp {
    pub len: u64,
    pub modified: u128,
    pub created: Option<u128>,
    pub identity: String,
    pub hash: String,
}
pub(super) fn identity(m: &Metadata) -> Result<String> {
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        Ok(format!("{}:{}", m.dev(), m.ino()))
    }
    #[cfg(windows)]
    {
        use cap_fs_ext::MetadataExt as Handle;
        use cap_std::fs::MetadataExt as Win;
        Ok(format!(
            "{:?}:{:?}:{}",
            Handle::dev(m),
            Handle::ino(m),
            Win::creation_time(m)
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = m;
        Err(RepositoryError::InvalidInput)
    }
}
pub(super) fn stamp(file: &mut File) -> Result<Stamp> {
    let before = file.metadata().map_err(err)?;
    if !before.is_file() {
        return Err(RepositoryError::InvalidInput);
    }
    file.seek(SeekFrom::Start(0)).map_err(err)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        cancellation()?;
        let n = file.read(&mut buffer).map_err(err)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    let snapshot = |m: &Metadata, hash: String| -> Result<Stamp> {
        Ok(Stamp {
            len: m.len(),
            modified: m
                .modified()
                .map_err(err)?
                .into_std()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| RepositoryError::Conflict)?
                .as_nanos(),
            created: m
                .created()
                .ok()
                .and_then(|v| v.into_std().duration_since(std::time::UNIX_EPOCH).ok())
                .map(|v| v.as_nanos()),
            identity: identity(m)?,
            hash,
        })
    };
    let result = snapshot(&before, format!("{:x}", digest.finalize()))?;
    if snapshot(&file.metadata().map_err(err)?, result.hash.clone())? != result {
        return Err(RepositoryError::Conflict);
    }
    Ok(result)
}
pub(super) fn file(dir: &Dir, name: &str) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.share_mode(0x1 | 0x2);
    }
    let f = dir.open_with(name, &options).map_err(err)?;
    if !f.metadata().map_err(err)?.is_file() {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(f)
}
pub(super) fn optional_stamp(dir: &Dir, name: &str) -> Result<Option<Stamp>> {
    match dir.symlink_metadata(name) {
        Ok(m) if m.is_file() => Ok(Some(stamp(&mut file(dir, name)?)?)),
        Ok(_) => Err(RepositoryError::Conflict),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(err(e)),
    }
}
pub(super) fn directory(dir: &Dir, name: impl AsRef<Path>) -> Result<Dir> {
    #[cfg(windows)]
    {
        use cap_fs_ext::OpenOptionsMaybeDirExt;
        use cap_std::fs::OpenOptionsExt;
        let mut o = OpenOptions::new();
        o.read(true)
            .maybe_dir(true)
            .follow(FollowSymlinks::No)
            .share_mode(0x1 | 0x2);
        let f = dir.open_with(name, &o).map_err(err)?;
        if !f.metadata().map_err(err)?.is_dir() {
            return Err(RepositoryError::Conflict);
        }
        Ok(Dir::from_std_file(f.into_std()))
    }
    #[cfg(not(windows))]
    {
        dir.open_dir_nofollow(name).map_err(err)
    }
}
pub(super) struct RootCap {
    pub path: PathBuf,
    pub root: Dir,
    pub _ancestors: Vec<Dir>,
    pub id: String,
}
impl RootCap {
    pub fn open(path: &Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err(RepositoryError::InvalidInput);
        }
        let mut anchor = PathBuf::new();
        let mut parts = vec![];
        for c in path.components() {
            match c {
                Component::Prefix(p) => anchor.push(p.as_os_str()),
                Component::RootDir => anchor.push(c.as_os_str()),
                Component::Normal(p) => parts.push(p.to_os_string()),
                _ => return Err(RepositoryError::InvalidInput),
            }
        }
        let mut current = Dir::open_ambient_dir(anchor, ambient_authority()).map_err(err)?;
        let mut ancestors = vec![];
        for p in parts {
            let next = directory(&current, p)?;
            ancestors.push(current);
            current = next;
        }
        let id = identity(&current.dir_metadata().map_err(err)?)?;
        Ok(Self {
            path: path.to_path_buf(),
            root: current,
            _ancestors: ancestors,
            id,
        })
    }
    pub fn directory_states(&self, relative: &str) -> Result<DirectoryStates> {
        checked(relative)?;
        let parts = relative.split('/').collect::<Vec<_>>();
        let mut current = self.root.try_clone().map_err(err)?;
        let mut held = vec![];
        let mut states = vec![];
        let mut prefix = String::new();
        for part in parts.iter().take(parts.len() - 1) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            match current.symlink_metadata(part) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    states.push((prefix.clone(), None));
                    break;
                }
                Err(e) => return Err(err(e)),
                Ok(m) if !m.is_dir() => return Err(RepositoryError::Conflict),
                Ok(_) => {}
            }
            let next = directory(&current, part)?;
            states.push((
                prefix.clone(),
                Some(identity(&next.dir_metadata().map_err(err)?)?),
            ));
            held.push(current);
            current = next;
        }
        held.push(current);
        Ok((states, held))
    }
    pub fn recheck(&self) -> Result<()> {
        if Self::open(&self.path)?.id != self.id {
            return Err(RepositoryError::Conflict);
        }
        Ok(())
    }
    pub fn parent(&self, relative: &str, create: bool) -> Result<Parent> {
        checked(relative)?;
        let mut current = self.root.try_clone().map_err(err)?;
        let mut ancestors = vec![];
        let mut absolute = self.path.clone();
        let mut parts = relative.split('/').collect::<Vec<_>>();
        let name = parts.pop().ok_or(RepositoryError::InvalidInput)?.to_owned();
        for p in parts {
            if create {
                match current.create_dir(p) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(err(e)),
                }
            }
            let next = directory(&current, p)?;
            ancestors.push(current);
            current = next;
            absolute.push(p);
        }
        Ok(Parent {
            dir: current,
            name,
            absolute,
            _ancestors: ancestors,
        })
    }
}
pub(super) struct Parent {
    pub dir: Dir,
    pub name: String,
    pub absolute: PathBuf,
    pub _ancestors: Vec<Dir>,
}
pub(super) fn checked(relative: &str) -> Result<()> {
    if relative.is_empty()
        || relative.contains('\\')
        || relative.contains(':')
        || relative.contains('\0')
        || relative
            .split('/')
            .any(|c| c.is_empty() || c == "." || c == "..")
    {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(())
}
pub(super) fn create_owned(parent: &Dir, name: &str) -> Result<Dir> {
    parent.create_dir(name).map_err(err)?;
    directory(parent, name)
}
pub(super) fn no_replace(
    from: &Dir,
    from_name: &str,
    from_abs: &Path,
    to: &Dir,
    to_name: &str,
    to_abs: &Path,
) -> Result<()> {
    #[cfg(test)]
    super::tests::before_move(from_abs);
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))]
    {
        let _ = (from_abs, to_abs);
        rustix::fs::renameat_with(
            from,
            from_name,
            to,
            to_name,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|_| RepositoryError::Conflict)
    }
    #[cfg(windows)]
    {
        let _ = (from, from_name, to, to_name);
        atomicwrites::move_atomic(from_abs, to_abs).map_err(err)
    }
    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    )))]
    {
        let _ = (from, from_name, from_abs, to, to_name, to_abs);
        Err(RepositoryError::InvalidInput)
    }
}
