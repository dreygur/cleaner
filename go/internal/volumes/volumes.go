package volumes

import (
	"os"
	"path/filepath"
	"syscall"
)

// MountDir is where macOS mounts every disk other than the boot volume.
const MountDir = "/Volumes"

// Mount flags from <sys/mount.h>. A read only mount cannot be cleaned and a
// non local one (SMB, NFS, AFP) would turn the scan into a network crawl.
const (
	mntRdonly     = 0x00000001
	mntLocal      = 0x00001000
	mntDontBrowse = 0x00100000
)

// backupMarkers sit at the root of a Time Machine destination. Its contents
// are backups, so a node_modules in there is history, not a build artifact.
var backupMarkers = []string{"Backups.backupdb", ".timemachine"}

func isBackup(root string) bool {
	for _, m := range backupMarkers {
		if _, err := os.Lstat(filepath.Join(root, m)); err == nil {
			return true
		}
	}
	return false
}

func cleanable(root string, boot syscall.Fsid) bool {
	info, err := os.Lstat(root)
	if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return false
	}
	var st syscall.Statfs_t
	if err := syscall.Statfs(root, &st); err != nil {
		return false
	}
	// An unmounted /Volumes/X is a plain folder on the boot disk.
	if st.Fsid == boot {
		return false
	}
	if st.Flags&mntLocal == 0 || st.Flags&(mntRdonly|mntDontBrowse) != 0 {
		return false
	}
	return !isBackup(root)
}

// List returns the mount points of local, writable disks attached under dir,
// external and removable drives included. The boot volume, network shares,
// read only images and Time Machine destinations are left out.
func List(dir string) []string {
	var boot syscall.Statfs_t
	if err := syscall.Statfs("/", &boot); err != nil {
		return nil
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}

	var found []string
	for _, e := range entries {
		root := filepath.Join(dir, e.Name())
		if cleanable(root, boot.Fsid) {
			found = append(found, root)
		}
	}
	return found
}
