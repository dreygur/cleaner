package volumes

import (
	"os"
	"path/filepath"
	"testing"
)

func TestListSkipsPlainFoldersAndLinks(t *testing.T) {
	dir := t.TempDir()
	if err := os.Mkdir(filepath.Join(dir, "unmounted"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("/", filepath.Join(dir, "Macintosh HD")); err != nil {
		t.Fatal(err)
	}

	if got := List(dir); len(got) != 0 {
		t.Errorf("List() = %v, want none", got)
	}
}
