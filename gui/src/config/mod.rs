use std::path::{Path, PathBuf};

use crate::types::{Kind, Rule};

pub const DEFAULT_SEARCH_ROOTS: &[&str] = &["Code", "Projects", "src", "work", "dev"];

pub const PROTECTED_NAMES: &[&str] = &[
    "Documents", "Desktop", "Downloads", "Pictures",
    "Movies", "Music", "Library", "Applications", "Public",
];

fn artifact(
    id: &'static str,
    description: &'static str,
    dir: &'static str,
    marker: &'static str,
    regen: &'static str,
    roots: &[PathBuf],
) -> Rule {
    Rule {
        id,
        description,
        kind: Kind::Artifact,
        dir,
        marker,
        roots: roots.to_vec(),
        paths: Vec::new(),
        regen,
    }
}

fn fixed(id: &'static str, description: &'static str, regen: &'static str, paths: Vec<PathBuf>) -> Rule {
    Rule {
        id,
        description,
        kind: Kind::Fixed,
        dir: "",
        marker: "",
        roots: Vec::new(),
        paths,
        regen,
    }
}

pub fn rules(home: &Path, search_roots: &[PathBuf]) -> Vec<Rule> {
    let lib = home.join("Library");
    let caches = lib.join("Caches");

    vec![
        artifact("rust-target", "Cargo build output", "target", "Cargo.toml",
            "cargo build", search_roots),
        artifact("node-modules", "Installed npm dependencies", "node_modules", "package.json",
            "npm install / pnpm install", search_roots),
        artifact("next-build", "Next.js build output", ".next", "package.json",
            "next build", search_roots),
        artifact("pycache", "Compiled Python bytecode", "__pycache__", "",
            "regenerated on next import", search_roots),

        fixed("go-build", "Go build cache", "go build",
            vec![caches.join("go-build")]),
        fixed("pnpm-store", "pnpm package store", "pnpm install",
            vec![caches.join("pnpm"), lib.join("pnpm").join("store")]),
        fixed("brew-cache", "Downloaded Homebrew bottles", "brew reinstalls on demand",
            vec![caches.join("Homebrew")]),
        fixed("pip-cache", "pip wheel cache", "pip download on next install",
            vec![caches.join("pip")]),
        fixed("js-tool-cache", "node-gyp and TypeScript caches", "re-downloaded on demand",
            vec![caches.join("node-gyp"), caches.join("typescript")]),
        fixed("chrome-cache", "Chrome on-disk HTTP cache (profile untouched)", "refills as you browse",
            vec![caches.join("Google")]),
        fixed("cargo-registry", "Downloaded crate sources", "cargo fetch (needs network)",
            vec![
                home.join(".cargo").join("registry").join("cache"),
                home.join(".cargo").join("registry").join("src"),
                home.join(".cargo").join("git").join("checkouts"),
            ]),
        fixed("xcode-derived", "Xcode DerivedData", "rebuilt by Xcode",
            vec![lib.join("Developer").join("Xcode").join("DerivedData")]),
        fixed("trash", "Trash", "nothing, this is a permanent delete",
            vec![home.join(".Trash")]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rules_returns_all_thirteen_in_order() {
        let home = PathBuf::from("/Users/test");
        let roots = vec![home.join("Code")];
        let rs = rules(&home, &roots);

        let ids: Vec<&str> = rs.iter().map(|r| r.id).collect();
        assert_eq!(
            ids,
            vec![
                "rust-target", "node-modules", "next-build", "pycache",
                "go-build", "pnpm-store", "brew-cache", "pip-cache",
                "js-tool-cache", "chrome-cache", "cargo-registry",
                "xcode-derived", "trash",
            ]
        );
    }

    #[test]
    fn artifact_rule_carries_the_given_search_roots_and_marker() {
        let home = PathBuf::from("/Users/test");
        let roots = vec![home.join("Code"), home.join("Projects")];
        let rs = rules(&home, &roots);
        let rust_target = &rs[0];
        assert_eq!(rust_target.roots, roots);
        assert_eq!(rust_target.dir, "target");
        assert_eq!(rust_target.marker, "Cargo.toml");
    }

    #[test]
    fn fixed_rule_paths_are_absolute_under_home() {
        let home = PathBuf::from("/Users/test");
        let rs = rules(&home, &[]);

        let trash = rs.iter().find(|r| r.id == "trash").unwrap();
        assert_eq!(trash.paths, vec![home.join(".Trash")]);

        let cargo_registry = rs.iter().find(|r| r.id == "cargo-registry").unwrap();
        assert_eq!(
            cargo_registry.paths,
            vec![
                home.join(".cargo").join("registry").join("cache"),
                home.join(".cargo").join("registry").join("src"),
                home.join(".cargo").join("git").join("checkouts"),
            ]
        );
    }
}
