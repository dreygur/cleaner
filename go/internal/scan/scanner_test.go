package scan

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/dreygur/cleaner/internal/models"
)

func mockTree(t *testing.T) string {
	t.Helper()
	root := t.TempDir()

	crate := filepath.Join(root, "crate")
	if err := os.MkdirAll(filepath.Join(crate, "target", "debug"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(crate, "Cargo.toml"), []byte("[package]\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(crate, "target", "debug", "bin"), make([]byte, 4096), 0o644); err != nil {
		t.Fatal(err)
	}

	// A "target" directory with no Cargo.toml beside it must never match.
	if err := os.MkdirAll(filepath.Join(root, "notes", "target"), 0o755); err != nil {
		t.Fatal(err)
	}
	return root
}

func TestCollectArtifactRequiresMarker(t *testing.T) {
	root := mockTree(t)
	rule := models.Rule{
		Kind:   models.KindArtifact,
		Dir:    "target",
		Marker: "Cargo.toml",
		Roots:  []string{root},
	}

	got := collect(rule)
	if len(got) != 1 {
		t.Fatalf("collect() returned %d paths, want 1: %v", len(got), got)
	}
	want := filepath.Join(root, "crate", "target")
	if got[0] != want {
		t.Errorf("collect() = %q, want %q", got[0], want)
	}
}

func TestCollectFixedSkipsMissingPaths(t *testing.T) {
	root := t.TempDir()
	present := filepath.Join(root, "cache")
	if err := os.Mkdir(present, 0o755); err != nil {
		t.Fatal(err)
	}

	rule := models.Rule{
		Kind:  models.KindFixed,
		Paths: []string{present, filepath.Join(root, "absent")},
	}

	got := collect(rule)
	if len(got) != 1 || got[0] != present {
		t.Fatalf("collect() = %v, want [%s]", got, present)
	}
}

func TestScanMeasuresSizes(t *testing.T) {
	root := mockTree(t)
	rule := models.Rule{
		ID: "rust-target", Kind: models.KindArtifact,
		Dir: "target", Marker: "Cargo.toml", Roots: []string{root},
	}

	groups := Scan([]models.Rule{rule})
	if len(groups) != 1 {
		t.Fatalf("Scan() returned %d groups, want 1", len(groups))
	}
	if groups[0].Size < 4096 {
		t.Errorf("group size = %d, want at least 4096", groups[0].Size)
	}
}

func TestScanDropsEmptyRules(t *testing.T) {
	rule := models.Rule{
		ID: "nope", Kind: models.KindFixed,
		Paths: []string{filepath.Join(t.TempDir(), "missing")},
	}
	if groups := Scan([]models.Rule{rule}); len(groups) != 0 {
		t.Fatalf("Scan() returned %d groups, want 0", len(groups))
	}
}
