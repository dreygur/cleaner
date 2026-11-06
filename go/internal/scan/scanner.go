package scan

import (
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"sync"

	"github.com/dreygur/cleaner/internal/models"
)

// sizeWorkers bounds how many directories are measured at once. Sizing is
// I/O bound, so oversubscribing the CPU count keeps the disk queue full.
var sizeWorkers = runtime.NumCPU() * 2

func dirSize(root string) int64 {
	var total int64
	filepath.WalkDir(root, func(_ string, d fs.DirEntry, err error) error {
		if err != nil || d.IsDir() {
			return nil
		}
		info, err := d.Info()
		if err != nil {
			return nil
		}
		total += info.Size()
		return nil
	})
	return total
}

func collectFixed(rule models.Rule) []string {
	var found []string
	for _, p := range rule.Paths {
		info, err := os.Lstat(p)
		if err != nil || !info.IsDir() {
			continue
		}
		found = append(found, p)
	}
	return found
}

func measure(paths []string) []models.Target {
	targets := make([]models.Target, len(paths))
	sem := make(chan struct{}, sizeWorkers)
	var wg sync.WaitGroup

	for i, p := range paths {
		wg.Add(1)
		go func(i int, p string) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()
			targets[i] = models.Target{Path: p, Size: dirSize(p)}
		}(i, p)
	}
	wg.Wait()
	return targets
}

// Scan locates and measures every directory the given rules match, dropping
// rules that matched nothing. Groups come back sorted by the caller, not here.
func Scan(rules []models.Rule) []models.Group {
	artifacts := collectArtifacts(rules)

	var groups []models.Group
	for _, rule := range rules {
		paths := artifacts[rule.ID]
		if rule.Kind == models.KindFixed {
			paths = collectFixed(rule)
		}
		if len(paths) == 0 {
			continue
		}
		targets := measure(paths)

		var total int64
		for _, t := range targets {
			total += t.Size
		}
		groups = append(groups, models.Group{Rule: rule, Targets: targets, Size: total})
	}
	return groups
}
