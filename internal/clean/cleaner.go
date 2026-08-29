package clean

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/dreygur/cleaner/internal/models"
)

var (
	// ErrOutsideHome rejects any path that does not live under the user's home
	// directory. System and other users' files are never this tool's business.
	ErrOutsideHome = errors.New("path is outside the home directory")

	// ErrProtectedPath rejects the home directory itself and the top-level
	// folders that hold a user's own documents.
	ErrProtectedPath = errors.New("path is protected")

	// ErrNotDirectory rejects files and symlinks, so a symlinked target can
	// never be followed into somewhere it should not reach.
	ErrNotDirectory = errors.New("path is not a real directory")
)

// protectedNames are directories directly under home that hold user data and
// must never be deleted, even if a rule somehow names one.
var protectedNames = map[string]struct{}{
	"Documents": {}, "Desktop": {}, "Downloads": {}, "Pictures": {},
	"Movies": {}, "Music": {}, "Library": {}, "Applications": {}, "Public": {},
}

func validate(home, path string) error {
	if !filepath.IsAbs(path) {
		return fmt.Errorf("%q: %w", path, ErrOutsideHome)
	}
	clean := filepath.Clean(path)
	if clean == filepath.Clean(home) {
		return fmt.Errorf("%q: %w", clean, ErrProtectedPath)
	}
	rel, err := filepath.Rel(home, clean)
	if err != nil || rel == "." || strings.HasPrefix(rel, "..") {
		return fmt.Errorf("%q: %w", clean, ErrOutsideHome)
	}
	if _, bad := protectedNames[rel]; bad {
		return fmt.Errorf("%q: %w", clean, ErrProtectedPath)
	}
	info, err := os.Lstat(clean)
	if err != nil {
		return fmt.Errorf("stat %q: %w", clean, err)
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("%q: %w", clean, ErrNotDirectory)
	}
	return nil
}

func removeContents(path string) error {
	entries, err := os.ReadDir(path)
	if err != nil {
		return fmt.Errorf("read %q: %w", path, err)
	}
	for _, e := range entries {
		child := filepath.Join(path, e.Name())
		if err := os.RemoveAll(child); err != nil {
			return fmt.Errorf("remove %q: %w", child, err)
		}
	}
	return nil
}

func removeTarget(home string, kind models.Kind, target models.Target) error {
	if err := validate(home, target.Path); err != nil {
		return err
	}
	// Fixed caches keep their directory so the owning tool finds it next run;
	// build artifacts are recreated wholesale, so the directory goes too.
	if kind == models.KindFixed {
		return removeContents(target.Path)
	}
	if err := os.RemoveAll(target.Path); err != nil {
		return fmt.Errorf("remove %q: %w", target.Path, err)
	}
	return nil
}

// Group deletes every target in one group and returns the bytes freed along
// with any per-target failures. One failure never stops the remaining targets.
func Group(home string, group models.Group) (int64, []error) {
	var freed int64
	var failures []error

	for _, target := range group.Targets {
		if err := removeTarget(home, group.Rule.Kind, target); err != nil {
			failures = append(failures, err)
			continue
		}
		freed += target.Size
	}
	return freed, failures
}
