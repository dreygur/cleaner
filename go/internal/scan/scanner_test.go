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
		ID:     "rust-target",
		Kind:   models.KindArtifact,
		Dir:    "target",
		Marker: "Cargo.toml",
		Roots:  []string{root},
	}

	got := collectArtifacts([]models.Rule{rule})["rust-target"]
	if len(got) != 1 {
		t.Fatalf("collectArtifacts() returned %d paths, want 1: %v", len(got), got)
	}
	want := filepath.Join(root, "crate", "target")
	if got[0] != want {
		t.Errorf("collectArtifacts() = %q, want %q", got[0], want)
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

	got := collectFixed(rule)
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

func mockProject(t *testing.T, parts ...string) string {
	t.Helper()
	dir := filepath.Join(parts...)
	if err := os.MkdirAll(filepath.Join(dir, "node_modules", "pkg"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "package.json"), []byte("{}"), 0o644); err != nil {
		t.Fatal(err)
	}
	return filepath.Join(dir, "node_modules")
}

func TestCollectArtifactsSkipsHiddenAndPruned(t *testing.T) {
	root := t.TempDir()
	deep := mockProject(t, root, "a", "b", "c", "app")
	worktree := mockProject(t, root, "repo", ".claude", "worktrees", "w1")
	mockProject(t, root, ".nvm", "versions", "tool")
	mockProject(t, root, "repo", ".git", "app")
	mockProject(t, root, "Library", "app")

	rule := models.Rule{
		ID: "node-modules", Kind: models.KindArtifact,
		Dir: "node_modules", Marker: "package.json",
		Roots: []string{root}, Prune: []string{filepath.Join(root, "Library")},
	}

	got := collectArtifacts([]models.Rule{rule})["node-modules"]
	want := []string{deep, worktree}
	if len(got) != len(want) || got[0] != want[0] || got[1] != want[1] {
		t.Fatalf("collectArtifacts() = %v, want %v", got, want)
	}
}

func TestCollectArtifactsMatchesHiddenRuleDir(t *testing.T) {
	root := t.TempDir()
	app := filepath.Join(root, "web")
	if err := os.MkdirAll(filepath.Join(app, ".next", "cache"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(app, "package.json"), []byte("{}"), 0o644); err != nil {
		t.Fatal(err)
	}

	rule := models.Rule{
		ID: "next-build", Kind: models.KindArtifact,
		Dir: ".next", Marker: "package.json", Roots: []string{root},
	}

	got := collectArtifacts([]models.Rule{rule})["next-build"]
	if len(got) != 1 || got[0] != filepath.Join(app, ".next") {
		t.Fatalf("collectArtifacts() = %v, want the .next dir", got)
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
