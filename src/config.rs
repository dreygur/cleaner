use std::path::{Path, PathBuf};

use crate::container;
use crate::types::Regen::{Auto, Run};
use crate::types::{Kind, Regen, Rule, Safety};

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
/// Slug for stale entries in the system temp directories.
pub const RULE_TEMP: &str = "temp";
/// Slug for stopped containers, dangling images and Docker build cache.
pub const RULE_DOCKER_CACHE: &str = "docker-cache";
/// Slug for every Docker image no container uses.
pub const RULE_DOCKER_IMAGES: &str = "docker-images";
/// Slug for every Docker volume no container uses.
pub const RULE_DOCKER_VOLUMES: &str = "docker-volumes";
/// Slug for stopped containers and dangling images in Podman.
pub const RULE_PODMAN_CACHE: &str = "podman-cache";
/// Slug for every Podman image no container uses.
pub const RULE_PODMAN_IMAGES: &str = "podman-images";
/// Slug for every Podman volume no container uses.
pub const RULE_PODMAN_VOLUMES: &str = "podman-volumes";

/// Shared by every user; macOS links it to /private/tmp.
const SYSTEM_TMP: &str = "/tmp";

const DOCKER: &str = "docker";
const PODMAN: &str = "podman";

/// Plain `system prune` keeps unused build cache that is not dangling, so
/// Docker gets an explicit builder prune too.
const DOCKER_CACHE_CMDS: &[&[&str]] = &[&["system", "prune", "-f"], &["builder", "prune", "-af"]];
const PODMAN_CACHE_CMDS: &[&[&str]] = &[&["system", "prune", "-f"]];
const IMAGES_CMDS: &[&[&str]] = &[&["image", "prune", "-af"]];
/// Docker prunes only anonymous volumes without -a; Podman has no -a and
/// prunes every unused volume.
const DOCKER_VOLUMES_CMDS: &[&[&str]] = &[&["volume", "prune", "-af"]];
const PODMAN_VOLUMES_CMDS: &[&[&str]] = &[&["volume", "prune", "-f"]];

const DF_CACHE: &[&str] = &["Containers", "Build Cache"];
const DF_IMAGES: &[&str] = &["Images"];
const DF_VOLUMES: &[&str] = &["Local Volumes"];

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
        commands: &[],
        df: &[],
        safety: Safety::Safe,
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
        commands: &[],
        df: &[],
        safety: Safety::Safe,
        regen,
    }
}

/// The shared temp directory and the per user one in TMPDIR, resolved
/// through their symlinks so the deletion guard sees real directories.
pub fn temp_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = [PathBuf::from(SYSTEM_TMP), std::env::temp_dir()]
        .iter()
        .filter_map(|d| d.canonicalize().ok())
        .collect();
    dirs.dedup();
    dirs
}

/// A Prune rule against bin, or one with no paths when the engine is not
/// installed, so its id still works on the command line.
fn engine(
    id: &'static str,
    description: &'static str,
    regen: Regen,
    bin: &Option<PathBuf>,
    commands: &'static [&'static [&'static str]],
    df: &'static [&'static str],
    safety: Safety,
) -> Rule {
    Rule {
        id,
        description,
        kind: Kind::Prune,
        dir: "",
        marker: "",
        roots: Vec::new(),
        prune: Vec::new(),
        paths: bin.iter().cloned().collect(),
        commands,
        df,
        safety,
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
    let docker = container::find(home, DOCKER);
    let podman = container::find(home, PODMAN);
    let temp = Rule {
        kind: Kind::Temp,
        ..fixed(RULE_TEMP, "Temp files untouched for 3 days", Auto("Apps recreate temp files when they need them"),
            temp_dirs())
    };

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
        temp,

        engine(RULE_DOCKER_CACHE, "Docker stopped containers and build cache", Auto("Rebuilt on the next docker build"),
            &docker, DOCKER_CACHE_CMDS, DF_CACHE, Safety::Safe),
        engine(RULE_DOCKER_IMAGES, "Docker images no container uses", Run("docker pull"),
            &docker, IMAGES_CMDS, DF_IMAGES, Safety::OptIn),
        engine(RULE_DOCKER_VOLUMES, "Docker volumes no container uses", Auto("Nothing brings it back, volume data is gone for good"),
            &docker, DOCKER_VOLUMES_CMDS, DF_VOLUMES, Safety::OptIn),
        engine(RULE_PODMAN_CACHE, "Podman stopped containers and dangling images", Auto("Rebuilt on the next podman build"),
            &podman, PODMAN_CACHE_CMDS, DF_CACHE, Safety::Safe),
        engine(RULE_PODMAN_IMAGES, "Podman images no container uses", Run("podman pull"),
            &podman, IMAGES_CMDS, DF_IMAGES, Safety::OptIn),
        engine(RULE_PODMAN_VOLUMES, "Podman volumes no container uses", Auto("Nothing brings it back, volume data is gone for good"),
            &podman, PODMAN_VOLUMES_CMDS, DF_VOLUMES, Safety::OptIn),
    ]
}
