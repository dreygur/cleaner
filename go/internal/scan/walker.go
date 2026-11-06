package scan

import (
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"sync"
	"syscall"

	"github.com/dreygur/cleaner/internal/models"
)

// walkWorkers bounds how many directories are read at once during the
// search. ReadDir is I/O bound, so the pool runs wider than the CPU count.
var walkWorkers = runtime.NumCPU() * 4

// gitDir holds only history; walking its object store finds nothing.
const gitDir = ".git"

// walker searches every root once for all artifact rules together. The old
// search walked each root once per rule, so four rules meant four full walks.
//
//	root ─┬─> match (target beside Cargo.toml)   recorded, not entered
//	      ├─> pruned, .git, or hidden at the root  skipped
//	      ├─> another filesystem mounted inside   skipped
//	      └─> anything else                       walked, in parallel when a worker is free
type walker struct {
	rules []models.Rule
	prune map[string]bool
	slots chan struct{}

	mu    sync.Mutex
	found map[int]map[string]bool
}

func device(path string) (int32, bool) {
	info, err := os.Lstat(path)
	if err != nil {
		return 0, false
	}
	st, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		return 0, false
	}
	return st.Dev, true
}

func newWalker(rules []models.Rule) *walker {
	w := &walker{
		prune: map[string]bool{},
		slots: make(chan struct{}, walkWorkers),
		found: map[int]map[string]bool{},
	}
	for _, r := range rules {
		if r.Kind != models.KindArtifact {
			continue
		}
		w.rules = append(w.rules, r)
		for _, p := range r.Prune {
			w.prune[filepath.Clean(p)] = true
		}
	}
	return w
}

func (w *walker) record(rule int, path string) {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.found[rule] == nil {
		w.found[rule] = map[string]bool{}
	}
	w.found[rule][path] = true
}

// match reports whether a directory named name inside parent is a build
// artifact for any rule. Markers are checked against the parent's listing,
// which costs no extra stat per candidate.
func (w *walker) match(path, name string, siblings map[string]bool) bool {
	matched := false
	for i, r := range w.rules {
		if r.Dir != name {
			continue
		}
		if r.Marker != "" && !siblings[r.Marker] {
			continue
		}
		w.record(i, path)
		matched = true
	}
	return matched
}

// skip reports whether the walk stays out of child. Hidden directories right
// under a root hold tool state (~/.nvm, ~/.cargo) whose node_modules belong
// to installed tools. Deeper ones, like a .claude/worktrees checkout, are
// project trees and are walked.
func (w *walker) skip(root, dir, name, child string) bool {
	if name == gitDir || w.prune[child] {
		return true
	}
	return dir == root && strings.HasPrefix(name, ".")
}

func (w *walker) visit(root, dir string, dev int32) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}
	siblings := make(map[string]bool, len(entries))
	for _, e := range entries {
		siblings[e.Name()] = true
	}

	var wg sync.WaitGroup
	for _, e := range entries {
		if !e.IsDir() {
			continue
		}
		name := e.Name()
		child := filepath.Join(dir, name)
		if w.match(child, name, siblings) {
			continue
		}
		if w.skip(root, dir, name, child) {
			continue
		}
		if d, ok := device(child); !ok || d != dev {
			continue
		}
		select {
		case w.slots <- struct{}{}:
			wg.Add(1)
			go func() {
				defer wg.Done()
				defer func() { <-w.slots }()
				w.visit(root, child, dev)
			}()
		default:
			w.visit(root, child, dev)
		}
	}
	wg.Wait()
}

func sortedKeys(set map[string]bool) []string {
	keys := make([]string, 0, len(set))
	for k := range set {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

// collectArtifacts returns the matched paths for each artifact rule, keyed by
// the rule's ID. Roots shared between rules are walked once.
func collectArtifacts(rules []models.Rule) map[string][]string {
	w := newWalker(rules)
	roots := map[string]bool{}
	for _, r := range w.rules {
		for _, root := range r.Roots {
			roots[filepath.Clean(root)] = true
		}
	}

	var wg sync.WaitGroup
	for root := range roots {
		info, err := os.Stat(root)
		if err != nil || !info.IsDir() {
			continue
		}
		dev, ok := device(root)
		if !ok {
			continue
		}
		wg.Add(1)
		go func() {
			defer wg.Done()
			w.visit(root, root, dev)
		}()
	}
	wg.Wait()

	out := map[string][]string{}
	for i, r := range w.rules {
		out[r.ID] = sortedKeys(w.found[i])
	}
	return out
}
