//! Stale entries in the system temp directories. Temp folders hold live
//! sockets and lock files next to leftovers, so an entry goes only when it
//! is the user's own, holds no socket anywhere below it, and nothing in it
//! changed for STALE_AFTER.

use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::remove;

/// How long an entry must sit untouched before it counts as a leftover.
pub const STALE_AFTER: Duration = Duration::from_secs(3 * 24 * 60 * 60);

/// Size and newest modification time of one entry and everything below it,
/// or None when a socket sits anywhere inside.
fn survey(path: &Path, meta: &fs::Metadata) -> Option<(u64, SystemTime)> {
    let mut size = 0;
    let mut newest = meta.modified().ok()?;
    let mut stack = vec![(path.to_path_buf(), meta.clone())];

    while let Some((p, m)) = stack.pop() {
        let ft = m.file_type();
        if ft.is_socket() {
            return None;
        }
        newest = newest.max(m.modified().unwrap_or(newest));
        if !ft.is_dir() {
            size += m.len();
            continue;
        }
        let Ok(entries) = fs::read_dir(&p) else {
            continue;
        };
        for e in entries.filter_map(|e| e.ok()) {
            if let Ok(m) = fs::symlink_metadata(e.path()) {
                stack.push((e.path(), m));
            }
        }
    }
    Some((size, newest))
}

/// Entries directly inside dir that are safe to remove at now, each with
/// its size.
pub fn stale(dir: &Path, now: SystemTime) -> Vec<(PathBuf, u64)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    // SAFETY: getuid has no preconditions and cannot fail.
    let me = unsafe { libc::getuid() };
    let cutoff = now.checked_sub(STALE_AFTER).unwrap_or(SystemTime::UNIX_EPOCH);

    entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let meta = fs::symlink_metadata(e.path()).ok()?;
            if meta.uid() != me {
                return None;
            }
            let (size, newest) = survey(&e.path(), &meta)?;
            (newest < cutoff).then(|| (e.path(), size))
        })
        .collect()
}

/// Bytes stale would free in dir at now.
pub fn stale_size(dir: &Path, now: SystemTime) -> u64 {
    stale(dir, now).iter().map(|(_, n)| n).sum()
}

/// Removes every stale entry in dir. Each is checked again here, since
/// time passed since the scan. One failure does not stop the rest; the
/// first one is returned.
pub fn clear(dir: &Path, now: SystemTime) -> io::Result<()> {
    let mut first = None;
    for (path, _) in stale(dir, now) {
        let is_dir = fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir());
        let removed = if is_dir { remove::tree(&path) } else { fs::remove_file(&path) };
        if let Err(e) = removed
            && first.is_none()
        {
            first = Some(e);
        }
    }
    first.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;

    use super::*;

    fn later() -> SystemTime {
        SystemTime::now() + STALE_AFTER + Duration::from_secs(60)
    }

    fn mock_temp() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("old.log"), vec![0u8; 100]).unwrap();
        fs::create_dir_all(tmp.path().join("build/a")).unwrap();
        fs::write(tmp.path().join("build/a/blob"), vec![0u8; 50]).unwrap();
        fs::create_dir(tmp.path().join("tmux-501")).unwrap();
        UnixListener::bind(tmp.path().join("tmux-501/default")).unwrap();
        tmp
    }

    #[test]
    fn fresh_entries_are_not_stale() {
        let tmp = mock_temp();
        assert!(stale(tmp.path(), SystemTime::now()).is_empty());
    }

    #[test]
    fn old_entries_are_stale_except_sockets() {
        let tmp = mock_temp();
        let mut got: Vec<PathBuf> = stale(tmp.path(), later()).into_iter().map(|(p, _)| p).collect();
        got.sort();
        assert_eq!(got, [tmp.path().join("build"), tmp.path().join("old.log")]);
        assert_eq!(stale_size(tmp.path(), later()), 150);
    }

    #[test]
    fn clear_keeps_the_dir_and_socket_holders() {
        let tmp = mock_temp();
        clear(tmp.path(), later()).unwrap();

        assert!(!tmp.path().join("old.log").exists());
        assert!(!tmp.path().join("build").exists());
        assert!(tmp.path().join("tmux-501/default").exists());
    }

    #[test]
    fn clear_leaves_fresh_entries() {
        let tmp = mock_temp();
        clear(tmp.path(), SystemTime::now()).unwrap();
        assert!(tmp.path().join("old.log").exists());
    }
}
