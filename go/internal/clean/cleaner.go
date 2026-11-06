package clean

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/dreygur/cleaner/internal/models"
)

var (
	// ErrOutsideHome rejects any path that does not live under the user's home
	// directory or an attached disk. System and other users' files are never
	// this tool's business.
	ErrOutsideHome = errors.New("path is outside the home directory")

	// ErrProtectedPath rejects the home directory itself and the top-level
	// folders that hold a user's own documents.
	ErrProtectedPath = errors.New("path is protected")

	// ErrNotDirectory rejects files and symlinks, so a symlinked target can
	// never be followed into somewhere it should not reach.
	ErrNotDirectory = errors.New("path is not a real directory")
)

// Scope is everywhere deletion may happen: the home directory, under its
// own guards, and the mount points of attached disks.
type Scope struct {
	Home    string
	Volumes []string
}

// protectedNames are directories directly under home that hold user data and
// must never be deleted, even if a rule somehow names one.
var protectedNames = map[string]struct{}{
	"Documents": {}, "Desktop": {}, "Downloads": {}, "Pictures": {},
	"Movies": {}, "Music": {}, "Library": {}, "Applications": {}, "Public": {},
}

// within returns path relative to base, or false when path is base itself
// or lies outside it.
func within(base, path string) (string, bool) {
	rel, err := filepath.Rel(filepath.Clean(base), path)
	if err != nil || rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", false
	}
	return rel, true
}

func onVolume(volumes []string, path string) bool {
	for _, v := range volumes {
		if _, ok := within(v, path); ok {
			return true
		}
	}
	return false
}

func validate(scope Scope, path string) error {
	if !filepath.IsAbs(path) {
		return fmt.Errorf("%q: %w", path, ErrOutsideHome)
	}
	clean := filepath.Clean(path)
	if clean == filepath.Clean(scope.Home) {
		return fmt.Errorf("%q: %w", clean, ErrProtectedPath)
	}
	rel, inHome := within(scope.Home, clean)
	if !inHome && !onVolume(scope.Volumes, clean) {
		return fmt.Errorf("%q: %w", clean, ErrOutsideHome)
	}
	if _, bad := protectedNames[rel]; inHome && bad {
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

func removeTarget(r *remover, scope Scope, kind models.Kind, target models.Target) error {
	if err := validate(scope, target.Path); err != nil {
		return err
	}
	// Fixed caches keep their directory so the owning tool finds it next run;
	// build artifacts are recreated wholesale, so the directory goes too.
	if kind == models.KindFixed {
		if err := r.contents(target.Path); err != nil {
			return fmt.Errorf("empty %q: %w", target.Path, err)
		}
		return nil
	}
	return r.tree(target.Path)
}

// Group deletes every target in one group concurrently and returns the bytes
// freed along with any per-target failures. One failure never stops the
// remaining targets.
func Group(scope Scope, group models.Group) (int64, []error) {
	r := newRemover()
	errs := make([]error, len(group.Targets))
	var wg sync.WaitGroup

	// Targets take a slot blocking, subdirectories only take a free one, so
	// thousands of small targets never open thousands of directories at once
	// and the pool can never deadlock on itself.
	for i, target := range group.Targets {
		wg.Add(1)
		r.slots <- struct{}{}
		go func() {
			defer wg.Done()
			defer func() { <-r.slots }()
			errs[i] = removeTarget(r, scope, group.Rule.Kind, target)
		}()
	}
	wg.Wait()

	var freed int64
	var failures []error
	for i, err := range errs {
		if err != nil {
			failures = append(failures, err)
			continue
		}
		freed += group.Targets[i].Size
	}
	return freed, failures
}
