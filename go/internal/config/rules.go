package config

import (
	"path/filepath"

	"github.com/dreygur/cleaner/internal/models"
)

const (
	// RuleRustTarget is the slug for Cargo build output.
	RuleRustTarget = "rust-target"
	// RuleNodeModules is the slug for installed npm dependencies.
	RuleNodeModules = "node-modules"
	// RuleNextBuild is the slug for Next.js build output.
	RuleNextBuild = "next-build"
	// RulePycache is the slug for compiled Python bytecode.
	RulePycache = "pycache"
	// RuleGoBuild is the slug for the Go build cache.
	RuleGoBuild = "go-build"
	// RulePnpmStore is the slug for the pnpm content-addressable store.
	RulePnpmStore = "pnpm-store"
	// RuleBrewCache is the slug for downloaded Homebrew bottles.
	RuleBrewCache = "brew-cache"
	// RulePipCache is the slug for the pip wheel cache.
	RulePipCache = "pip-cache"
	// RuleJsToolCache is the slug for node-gyp and TypeScript caches.
	RuleJsToolCache = "js-tool-cache"
	// RuleChromeCache is the slug for the Chrome on-disk HTTP cache.
	RuleChromeCache = "chrome-cache"
	// RuleCargoRegistry is the slug for downloaded crate sources.
	RuleCargoRegistry = "cargo-registry"
	// RuleXcodeDerived is the slug for Xcode DerivedData.
	RuleXcodeDerived = "xcode-derived"
	// RuleTrash is the slug for the user's Trash.
	RuleTrash = "trash"
)

// prunedHomeDirs are skipped when the artifact search walks the home
// directory. Library is covered by the fixed rules and holds app data, the
// media folders hold photo and music libraries that are slow to walk and never
// hold builds, and go/pkg is the read only Go module cache.
var prunedHomeDirs = []string{
	"Library", "Applications", "Pictures", "Movies", "Music", "Public",
	filepath.Join("go", "pkg"),
}

// SearchPrune lists the absolute directories the artifact search never enters.
func SearchPrune(home string) []string {
	var paths []string
	for _, name := range prunedHomeDirs {
		paths = append(paths, filepath.Join(home, name))
	}
	return paths
}

func artifact(id, desc, dir, marker, regen string, roots, prune []string) models.Rule {
	return models.Rule{
		ID:          id,
		Description: desc,
		Kind:        models.KindArtifact,
		Dir:         dir,
		Marker:      marker,
		Roots:       roots,
		Prune:       prune,
		Regen:       regen,
	}
}

func fixed(id, desc, regen string, paths ...string) models.Rule {
	return models.Rule{
		ID:          id,
		Description: desc,
		Kind:        models.KindFixed,
		Paths:       paths,
		Regen:       regen,
	}
}

// Rules builds the full rule set for one machine. Every rule targets data that
// a tool can rebuild or re-download, so nothing here is a source of truth.
func Rules(home string, searchRoots []string) []models.Rule {
	lib := filepath.Join(home, "Library")
	caches := filepath.Join(lib, "Caches")
	prune := SearchPrune(home)

	return []models.Rule{
		artifact(RuleRustTarget, "Cargo build output", "target", "Cargo.toml",
			"cargo build", searchRoots, prune),
		artifact(RuleNodeModules, "Installed npm dependencies", "node_modules", "package.json",
			"npm install / pnpm install", searchRoots, prune),
		artifact(RuleNextBuild, "Next.js build output", ".next", "package.json",
			"next build", searchRoots, prune),
		artifact(RulePycache, "Compiled Python bytecode", "__pycache__", "",
			"regenerated on next import", searchRoots, prune),

		fixed(RuleGoBuild, "Go build cache", "go build",
			filepath.Join(caches, "go-build")),
		fixed(RulePnpmStore, "pnpm package store", "pnpm install",
			filepath.Join(caches, "pnpm"), filepath.Join(lib, "pnpm", "store")),
		fixed(RuleBrewCache, "Downloaded Homebrew bottles", "brew reinstalls on demand",
			filepath.Join(caches, "Homebrew")),
		fixed(RulePipCache, "pip wheel cache", "pip download on next install",
			filepath.Join(caches, "pip")),
		fixed(RuleJsToolCache, "node-gyp and TypeScript caches", "re-downloaded on demand",
			filepath.Join(caches, "node-gyp"), filepath.Join(caches, "typescript")),
		fixed(RuleChromeCache, "Chrome on-disk HTTP cache (profile untouched)", "refills as you browse",
			filepath.Join(caches, "Google")),
		fixed(RuleCargoRegistry, "Downloaded crate sources", "cargo fetch (needs network)",
			filepath.Join(home, ".cargo", "registry", "cache"),
			filepath.Join(home, ".cargo", "registry", "src"),
			filepath.Join(home, ".cargo", "git", "checkouts")),
		fixed(RuleXcodeDerived, "Xcode DerivedData", "rebuilt by Xcode",
			filepath.Join(lib, "Developer", "Xcode", "DerivedData")),
		fixed(RuleTrash, "Trash", "nothing, this is a permanent delete",
			filepath.Join(home, ".Trash")),
	}
}
