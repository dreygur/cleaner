//! Deletion fans subdirectories across rayon's pool, one worker per CPU.
//! fs::remove_dir_all walks one entry at a time, which is what makes large
//! node_modules and target trees slow.
//!
//!   dir ─┬─> file, file, file   unlinked inline
//!        ├─> subdir             handed to the pool
//!        └─> subdir             handed to the pool

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

fn ignore_missing(r: io::Result<()>) -> io::Result<()> {
    match r {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Removes everything inside path and keeps path itself. DirEntry reports
/// symlinks by their own type, so a link is unlinked, never entered.
pub fn contents(path: &Path) -> io::Result<()> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };

    let mut first: Option<io::Error> = None;
    let mut dirs: Vec<PathBuf> = Vec::new();
    for e in entries {
        let e = e?;
        if e.file_type()?.is_dir() {
            dirs.push(e.path());
            continue;
        }
        if let Err(err) = ignore_missing(fs::remove_file(e.path())) {
            first.get_or_insert(err);
        }
    }

    let failed = dirs.par_iter().filter_map(|d| tree(d).err()).collect::<Vec<_>>();
    match first.or_else(|| failed.into_iter().next()) {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// Removes path and everything under it. Any failure in the fast path falls
/// back to fs::remove_dir_all, which reports the real cause.
pub fn tree(path: &Path) -> io::Result<()> {
    if contents(path).is_ok() && ignore_missing(fs::remove_dir(path)).is_ok() {
        return Ok(());
    }
    ignore_missing(fs::remove_dir_all(path))
}

#[cfg(test)]
mod tests {
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
}
