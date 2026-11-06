use std::ffi::CString;
use std::fs;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Where macOS mounts every disk other than the boot volume.
pub const MOUNT_DIR: &str = "/Volumes";

// Mount flags from <sys/mount.h>. A read only mount cannot be cleaned and a
// non local one (SMB, NFS, AFP) would turn the scan into a network crawl.
const MNT_RDONLY: u32 = 0x0000_0001;
const MNT_LOCAL: u32 = 0x0000_1000;
const MNT_DONTBROWSE: u32 = 0x0010_0000;

/// Sit at the root of a Time Machine destination. Its contents are backups,
/// so a node_modules in there is history, not a build artifact.
const BACKUP_MARKERS: &[&str] = &["Backups.backupdb", ".timemachine"];

fn statfs(path: &Path) -> Option<libc::statfs> {
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st = MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: c is a valid NUL terminated string and st is written in full
    // by statfs when it returns 0.
    if unsafe { libc::statfs(c.as_ptr(), st.as_mut_ptr()) } != 0 {
        return None;
    }
    Some(unsafe { st.assume_init() })
}

/// Bytes available to an unprivileged user on the filesystem holding path.
pub fn free_bytes(path: &Path) -> Option<u64> {
    let st = statfs(path)?;
    Some(st.f_bavail * u64::from(st.f_bsize))
}

fn is_backup(root: &Path) -> bool {
    BACKUP_MARKERS.iter().any(|m| fs::symlink_metadata(root.join(m)).is_ok())
}

fn mount_point(st: &libc::statfs) -> PathBuf {
    let bytes: Vec<u8> = st
        .f_mntonname
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    PathBuf::from(std::ffi::OsStr::from_bytes(&bytes))
}

fn cleanable(root: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(root) else {
        return false;
    };
    if !meta.is_dir() {
        return false;
    }
    let Some(st) = statfs(root) else {
        return false;
    };
    // An unmounted /Volumes/X is a plain folder on the boot disk, whose
    // filesystem is mounted somewhere else.
    if mount_point(&st) != root {
        return false;
    }
    if st.f_flags & MNT_LOCAL == 0 || st.f_flags & (MNT_RDONLY | MNT_DONTBROWSE) != 0 {
        return false;
    }
    !is_backup(root)
}

/// Mount points of local, writable disks attached under dir, external and
/// removable drives included. The boot volume, network shares, read only
/// images and Time Machine destinations are left out.
pub fn list(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut found: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|root| cleanable(root))
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_skips_plain_folders_and_links() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("unmounted")).unwrap();
        std::os::unix::fs::symlink("/", dir.path().join("Macintosh HD")).unwrap();

        assert!(list(dir.path()).is_empty());
    }
}
