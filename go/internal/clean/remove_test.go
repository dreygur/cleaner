package clean

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

// benchPackages and benchFiles shape a node_modules sized tree: many
// packages, each a few levels deep with small files.
const (
	benchPackages = 400
	benchFiles    = 25
)

func mockModules(b *testing.B) string {
	b.Helper()
	root := filepath.Join(b.TempDir(), "node_modules")
	for p := range benchPackages {
		dir := filepath.Join(root, fmt.Sprintf("pkg%d", p), "lib", "src")
		if err := os.MkdirAll(dir, 0o755); err != nil {
			b.Fatal(err)
		}
		for f := range benchFiles {
			if err := os.WriteFile(filepath.Join(dir, fmt.Sprintf("f%d.js", f)), []byte("x"), 0o644); err != nil {
				b.Fatal(err)
			}
		}
	}
	return root
}

func BenchmarkRemoveAll(b *testing.B) {
	for b.Loop() {
		b.StopTimer()
		root := mockModules(b)
		b.StartTimer()
		if err := os.RemoveAll(root); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkRemoverTree(b *testing.B) {
	for _, workers := range []int{2, 4, 8, 16, 32} {
		b.Run(fmt.Sprintf("workers=%d", workers), func(b *testing.B) {
			for b.Loop() {
				b.StopTimer()
				root := mockModules(b)
				b.StartTimer()
				r := &remover{slots: make(chan struct{}, workers)}
				if err := r.tree(root); err != nil {
					b.Fatal(err)
				}
			}
		})
	}
}
