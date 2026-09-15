package clean

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/dreygur/cleaner/internal/models"
)

func mockDir(t *testing.T, parts ...string) string {
	t.Helper()
	path := filepath.Join(parts...)
	if err := os.MkdirAll(path, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(path, "blob"), make([]byte, 2048), 0o644); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestValidateRejectsHomeItself(t *testing.T) {
	home := t.TempDir()
	if err := validate(home, home); !errors.Is(err, ErrProtectedPath) {
		t.Errorf("validate(home, home) = %v, want ErrProtectedPath", err)
	}
}

func TestValidateRejectsProtectedNames(t *testing.T) {
	home := t.TempDir()
	docs := mockDir(t, home, "Documents")
	if err := validate(home, docs); !errors.Is(err, ErrProtectedPath) {
		t.Errorf("validate(home, Documents) = %v, want ErrProtectedPath", err)
	}
}

func TestValidateRejectsOutsideHome(t *testing.T) {
	home := t.TempDir()
	outside := mockDir(t, t.TempDir(), "elsewhere")
	if err := validate(home, outside); !errors.Is(err, ErrOutsideHome) {
		t.Errorf("validate(home, outside) = %v, want ErrOutsideHome", err)
	}
}

func TestValidateRejectsSymlink(t *testing.T) {
	home := t.TempDir()
	real := mockDir(t, home, "real")
	link := filepath.Join(home, "link")
	if err := os.Symlink(real, link); err != nil {
		t.Fatal(err)
	}
	if err := validate(home, link); !errors.Is(err, ErrNotDirectory) {
		t.Errorf("validate(home, symlink) = %v, want ErrNotDirectory", err)
	}
}

func TestGroupRemovesArtifactDirectory(t *testing.T) {
	home := t.TempDir()
	target := mockDir(t, home, "Code", "app", "target")

	group := models.Group{
		Rule:    models.Rule{ID: "rust-target", Kind: models.KindArtifact},
		Targets: []models.Target{{Path: target, Size: 2048}},
	}

	freed, failures := Group(home, group)
	if len(failures) != 0 {
		t.Fatalf("Group() failures = %v", failures)
	}
	if freed != 2048 {
		t.Errorf("freed = %d, want 2048", freed)
	}
	if _, err := os.Stat(target); !os.IsNotExist(err) {
		t.Errorf("target still exists after Group()")
	}
}

func TestGroupEmptiesFixedDirectoryButKeepsIt(t *testing.T) {
	home := t.TempDir()
	cache := mockDir(t, home, "Library", "Caches", "pip")

	group := models.Group{
		Rule:    models.Rule{ID: "pip-cache", Kind: models.KindFixed},
		Targets: []models.Target{{Path: cache, Size: 2048}},
	}

	if _, failures := Group(home, group); len(failures) != 0 {
		t.Fatalf("Group() failures = %v", failures)
	}
	entries, err := os.ReadDir(cache)
	if err != nil {
		t.Fatalf("cache directory was removed: %v", err)
	}
	if len(entries) != 0 {
		t.Errorf("cache holds %d entries, want 0", len(entries))
	}
}

func TestGroupReportsFailureWithoutStopping(t *testing.T) {
	home := t.TempDir()
	good := mockDir(t, home, "Code", "app", "target")
	bad := filepath.Join(home, "Code", "gone", "target")

	group := models.Group{
		Rule: models.Rule{ID: "rust-target", Kind: models.KindArtifact},
		Targets: []models.Target{
			{Path: bad, Size: 999},
			{Path: good, Size: 2048},
		},
	}

	freed, failures := Group(home, group)
	if len(failures) != 1 {
		t.Fatalf("failures = %v, want exactly 1", failures)
	}
	if freed != 2048 {
		t.Errorf("freed = %d, want 2048", freed)
	}
	if _, err := os.Stat(good); !os.IsNotExist(err) {
		t.Errorf("good target survived a sibling failure")
	}
}
