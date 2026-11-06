use std::path::{Path, PathBuf};

use crate::types::Regen::{Auto, Run};
use crate::types::{Kind, Regen, Rule};

/// Slug for Cargo build output.
pub const RULE_RUST_TARGET: &str = "rust-target";
/// Slug for installed npm dependencies.
pub const RULE_NODE_MODULES: &str = "node-modules";
/// Slug for Next.js build output.
pub const RULE_NEXT_BUILD: &str = "next-build";
/// Slug for compiled Python bytecode.
pub const RULE_PYCACHE: &str = "pycache";
/// Slug for the Go build cache.
pub const RULE_GO_BUILD: &str = "go-build";
/// Slug for the pnpm content addressable store.
pub const RULE_PNPM_STORE: &str = "pnpm-store";
/// Slug for downloaded Homebrew bottles.
pub const RULE_BREW_CACHE: &str = "brew-cache";
/// Slug for the pip wheel cache.
pub const RULE_PIP_CACHE: &str = "pip-cache";
/// Slug for node-gyp and TypeScript caches.
pub const RULE_JS_TOOL_CACHE: &str = "js-tool-cache";
/// Slug for the Chrome on disk HTTP cache.
pub const RULE_CHROME_CACHE: &str = "chrome-cache";
/// Slug for downloaded crate sources.
pub const RULE_CARGO_REGISTRY: &str = "cargo-registry";
/// Slug for Xcode DerivedData.
pub const RULE_XCODE_DERIVED: &str = "xcode-derived";
/// Slug for the user's Trash.
pub const RULE_TRASH: &str = "trash";

/// Skipped when the artifact search walks the home directory. Library is
/// covered by the fixed rules and holds app data, the media folders hold
/// photo and music libraries that are slow to walk and never hold builds, and
/// go/pkg is the read only Go module cache.
const PRUNED_HOME_DIRS: &[&str] = &[
    "Library",
    "Applications",
    "Pictures",
    "Movies",
    "Music",
    "Public",
    "go/pkg",
];

/// Absolute directories the artifact search never enters.
pub fn search_prune(home: &Path) -> Vec<PathBuf> {
    PRUNED_HOME_DIRS.iter().map(|name| home.join(name)).collect()
}

fn artifact(
    id: &'static str,
    description: &'static str,
    dir: &'static str,
    marker: &'static str,
    regen: Regen,
    roots: &[PathBuf],
    prune: &[PathBuf],
) -> Rule {
    Rule {
        id,
        description,
        kind: Kind::Artifact,
        dir,
        marker,
        roots: roots.to_vec(),
        prune: prune.to_vec(),
        paths: Vec::new(),
        regen,
    }
}

fn fixed(id: &'static str, description: &'static str, regen: Regen, paths: Vec<PathBuf>) -> Rule {
    Rule {
        id,
        description,
        kind: Kind::Fixed,
        dir: "",
        marker: "",
        roots: Vec::new(),
        prune: Vec::new(),
        paths,
        regen,
    }
}

/// The full rule set for one machine. Every rule targets data that a tool
/// can rebuild or download again, so nothing here is a source of truth.
pub fn rules(home: &Path, search_roots: &[PathBuf]) -> Vec<Rule> {
    let lib = home.join("Library");
    let caches = lib.join("Caches");
    let prune = search_prune(home);
    let roots = search_roots;

    vec![
        artifact(RULE_RUST_TARGET, "Cargo build output", "target", "Cargo.toml",
            Run("cargo build"), roots, &prune),
        artifact(RULE_NODE_MODULES, "Installed npm dependencies", "node_modules", "package.json",
            Run("npm install"), roots, &prune),
        artifact(RULE_NEXT_BUILD, "Next.js build output", ".next", "package.json",
            Run("next build"), roots, &prune),
        artifact(RULE_PYCACHE, "Compiled Python bytecode", "__pycache__", "",
            Auto("Python rebuilds it on the next import"), roots, &prune),

        fixed(RULE_GO_BUILD, "Go build cache", Run("go build"),
            vec![caches.join("go-build")]),
        fixed(RULE_PNPM_STORE, "pnpm package store", Run("pnpm install"),
            vec![caches.join("pnpm"), lib.join("pnpm").join("store")]),
        fixed(RULE_BREW_CACHE, "Downloaded Homebrew bottles", Auto("Homebrew downloads them again when needed"),
            vec![caches.join("Homebrew")]),
        fixed(RULE_PIP_CACHE, "pip wheel cache", Auto("pip downloads it again on the next install"),
            vec![caches.join("pip")]),
        fixed(RULE_JS_TOOL_CACHE, "node-gyp and TypeScript caches", Auto("Downloaded again when needed"),
            vec![caches.join("node-gyp"), caches.join("typescript")]),
        fixed(RULE_CHROME_CACHE, "Chrome on-disk HTTP cache (profile untouched)", Auto("Chrome refills it as you browse"),
            vec![caches.join("Google")]),
        fixed(RULE_CARGO_REGISTRY, "Downloaded crate sources", Run("cargo fetch"),
            vec![
                home.join(".cargo/registry/cache"),
                home.join(".cargo/registry/src"),
                home.join(".cargo/git/checkouts"),
            ]),
        fixed(RULE_XCODE_DERIVED, "Xcode DerivedData", Auto("Xcode rebuilds it on the next build"),
            vec![lib.join("Developer/Xcode/DerivedData")]),
        fixed(RULE_TRASH, "Trash", Auto("Nothing brings it back, this is a permanent delete"),
            vec![home.join(".Trash")]),
    ]
}
