//! Deletion fans subdirectories across rayon's pool, one worker per CPU.
//! fs::remove_dir_all walks one entry at a time, which is what makes large
//! node_modules and target trees slow.
//!
//!   dir ─┬─> file, file, file   unlinked inline
//!        ├─> subdir             handed to the pool
//!        └─> subdir             handed to the pool
//!
//! Every step works on an open directory handle, never on a path: a child
//! is opened with O_NOFOLLOW relative to its parent and removed with
//! unlinkat. If a directory is swapped for a symlink while the tree is being
//! removed, the link itself is unlinked and its target is never entered.

use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rayon::prelude::*;

const DIR_FLAGS: libc::c_int = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;

fn ignore_missing(r: io::Result<()>) -> io::Result<()> {
    match r {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn check(rc: libc::c_int) -> io::Result<libc::c_int> {
    if rc < 0 { Err(io::Error::last_os_error()) } else { Ok(rc) }
}

fn cstring(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

fn open_dir_at(parent: RawFd, name: &CStr) -> io::Result<OwnedFd> {
    // SAFETY: name is NUL terminated; a non negative result is an fd we own.
    let fd = check(unsafe { libc::openat(parent, name.as_ptr(), DIR_FLAGS) })?;
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn unlink_at(parent: RawFd, name: &CStr, flags: libc::c_int) -> io::Result<()> {
    // SAFETY: name is NUL terminated.
    ignore_missing(check(unsafe { libc::unlinkat(parent, name.as_ptr(), flags) }).map(|_| ()))
}

/// Names in dir, each with whether it is a real directory (not a link).
fn entries(dir: &OwnedFd) -> io::Result<Vec<(CString, bool)>> {
    // fdopendir takes ownership of its fd, so it gets a duplicate.
    // SAFETY: dup of a valid fd; fdopendir/readdir/closedir on that stream.
    let dup = check(unsafe { libc::dup(dir.as_raw_fd()) })?;
    let stream = unsafe { libc::fdopendir(dup) };
    if stream.is_null() {
        let err = io::Error::last_os_error();
        unsafe { libc::close(dup) };
        return Err(err);
    }

    let mut out = Vec::new();
    loop {
        let ent = unsafe { libc::readdir(stream) };
        if ent.is_null() {
            break;
        }
        let name = unsafe { CStr::from_ptr((*ent).d_name.as_ptr()) };
        if name == c"." || name == c".." {
            continue;
        }
        let kind = unsafe { (*ent).d_type };
        let is_dir = match kind {
            libc::DT_DIR => true,
            libc::DT_UNKNOWN => is_dir_at(dir.as_raw_fd(), name),
            _ => false,
        };
        out.push((name.to_owned(), is_dir));
    }
    unsafe { libc::closedir(stream) };
    Ok(out)
}

/// For filesystems that leave d_type unknown, such as some external disks.
fn is_dir_at(parent: RawFd, name: &CStr) -> bool {
    let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: st is written in full when fstatat returns 0.
    let rc = unsafe { libc::fstatat(parent, name.as_ptr(), st.as_mut_ptr(), libc::AT_SYMLINK_NOFOLLOW) };
    rc == 0 && unsafe { st.assume_init() }.st_mode & libc::S_IFMT == libc::S_IFDIR
}

/// Removes the directory entry name inside parent and everything below it.
fn remove_dir_at(parent: RawFd, name: &CStr) -> io::Result<()> {
    let child = match open_dir_at(parent, name) {
        Ok(fd) => fd,
        // Swapped for a symlink or a file since it was listed: remove the
        // entry itself, never what it points to.
        Err(e) if matches!(e.raw_os_error(), Some(libc::ELOOP) | Some(libc::ENOTDIR)) => {
            return unlink_at(parent, name, 0);
        }
        Err(e) => return ignore_missing(Err(e)),
    };
    clear(&child)?;
    drop(child);
    unlink_at(parent, name, libc::AT_REMOVEDIR)
}

/// Empties an open directory, subdirectories in parallel. One failure does
/// not stop the rest; the first one is returned.
fn clear(dir: &OwnedFd) -> io::Result<()> {
    let fd = dir.as_raw_fd();
    let (dirs, files): (Vec<_>, Vec<_>) = entries(dir)?.into_iter().partition(|(_, is_dir)| *is_dir);

    let mut first: Option<io::Error> = files.iter().filter_map(|(name, _)| unlink_at(fd, name, 0).err()).next();
    let failed = dirs.par_iter().filter_map(|(name, _)| remove_dir_at(fd, name).err()).collect::<Vec<_>>();
    if first.is_none() {
        first = failed.into_iter().next();
    }
    first.map_or(Ok(()), Err)
}

fn open_dir(path: &Path) -> io::Result<Option<OwnedFd>> {
    match open_dir_at(libc::AT_FDCWD, &cstring(path)?) {
        Ok(fd) => Ok(Some(fd)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Removes everything inside path and keeps path itself. Refuses a path
/// that is itself a symlink.
pub fn contents(path: &Path) -> io::Result<()> {
    match open_dir(path)? {
        Some(dir) => clear(&dir),
        None => Ok(()),
    }
}

/// Removes path and everything under it. Refuses a path that is itself a
/// symlink.
pub fn tree(path: &Path) -> io::Result<()> {
    contents(path)?;
    unlink_at(libc::AT_FDCWD, &cstring(path)?, libc::AT_REMOVEDIR)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn tree_deletes_deep_tree_without_following_links() {
        let tmp = tempfile::tempdir().unwrap();
        let keep = tmp.path().join("keep");
        fs::create_dir(&keep).unwrap();
        fs::write(keep.join("blob"), b"x").unwrap();

        let root = tmp.path().join("node_modules");
        for i in 0..50 {
            let dir = root.join(format!("pkg{i}/lib/deep"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("f.js"), b"x").unwrap();
        }
        std::os::unix::fs::symlink(&keep, root.join("link")).unwrap();

        tree(&root).unwrap();
        assert!(!root.exists());
        assert!(keep.join("blob").exists(), "symlink target was touched");
    }

    #[test]
    fn contents_keeps_directory() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("a/b")).unwrap();
        fs::write(tmp.path().join("f"), b"x").unwrap();

        contents(tmp.path()).unwrap();
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    // A directory listed as a directory, then swapped for a link before it
    // is entered: the link goes, its target stays.
    #[test]
    fn swapped_directory_is_unlinked_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let keep = tmp.path().join("keep");
        fs::create_dir(&keep).unwrap();
        fs::write(keep.join("blob"), b"x").unwrap();
        let root = tmp.path().join("target");
        fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(&keep, root.join("debug")).unwrap();

        let dir = open_dir(&root).unwrap().unwrap();
        remove_dir_at(dir.as_raw_fd(), c"debug").unwrap();

        assert!(!root.join("debug").exists());
        assert!(keep.join("blob").exists(), "symlink target was entered");
    }

    #[test]
    fn symlinked_root_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let keep = tmp.path().join("keep");
        fs::create_dir(&keep).unwrap();
        fs::write(keep.join("blob"), b"x").unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&keep, &link).unwrap();

        assert!(contents(&link).is_err());
        assert!(keep.join("blob").exists());
    }

    #[test]
    fn missing_paths_are_not_errors() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(tree(&tmp.path().join("gone")).is_ok());
    }
}
