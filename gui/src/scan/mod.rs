use std::fs;
use std::path::{Path, PathBuf};

use crate::types::{Kind, Rule};

fn walk_files(dir: &Path, total: &mut u64) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_dir() {
            walk_files(&entry.path(), total);
        } else if let Ok(metadata) = entry.metadata() {
            *total += metadata.len();
        }
    }
}

pub fn dir_size(root: &Path) -> u64 {
    let mut total = 0u64;
    walk_files(root, &mut total);
    total
}

pub fn has_marker(dir: &Path, marker: &str) -> bool {
    if marker.is_empty() {
        return true;
    }
    dir.join(marker).exists()
}

fn collect_fixed(rule: &Rule) -> Vec<PathBuf> {
    rule.paths
        .iter()
        .filter(|p| fs::symlink_metadata(p).map(|m| m.is_dir()).unwrap_or(false))
        .cloned()
        .collect()
}

fn walk_for_match(dir: &Path, target_name: &str, marker: &str, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        if entry.file_name() == target_name {
            if has_marker(dir, marker) {
                found.push(path);
            }
            continue;
        }
        walk_for_match(&path, target_name, marker, found);
    }
}

fn collect_artifact(rule: &Rule) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in &rule.roots {
        if !root.is_dir() {
            continue;
        }
        walk_for_match(root, rule.dir, rule.marker, &mut found);
    }
    found
}

pub fn collect(rule: &Rule) -> Vec<PathBuf> {
    match rule.kind {
        Kind::Fixed => collect_fixed(rule),
        Kind::Artifact => collect_artifact(rule),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn dir_size_sums_files_recursively() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), vec![0u8; 100]).unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("nested/b.txt"), vec![0u8; 250]).unwrap();

        assert_eq!(dir_size(dir.path()), 350);
    }

    #[test]
    fn has_marker_with_empty_marker_always_matches() {
        let dir = tempdir().unwrap();
        assert!(has_marker(dir.path(), ""));
    }

    #[test]
    fn has_marker_checks_the_named_file() {
        let dir = tempdir().unwrap();
        assert!(!has_marker(dir.path(), "Cargo.toml"));
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert!(has_marker(dir.path(), "Cargo.toml"));
    }

    #[test]
    fn collect_fixed_keeps_only_existing_directories() {
        let dir = tempdir().unwrap();
        let present = dir.path().join("present");
        fs::create_dir(&present).unwrap();
        let missing = dir.path().join("missing");

        let rule = crate::types::Rule {
            id: "t", description: "t", kind: crate::types::Kind::Fixed,
            dir: "", marker: "", roots: vec![],
            paths: vec![present.clone(), missing],
            regen: "t",
        };

        assert_eq!(collect(&rule), vec![present]);
    }

    #[test]
    fn collect_fixed_excludes_a_symlinked_directory() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real");
        fs::create_dir(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let rule = crate::types::Rule {
            id: "t", description: "t", kind: crate::types::Kind::Fixed,
            dir: "", marker: "", roots: vec![],
            paths: vec![link],
            regen: "t",
        };

        assert!(collect(&rule).is_empty());
    }

    #[test]
    fn collect_artifact_matches_only_when_marker_sits_beside_it() {
        let root = tempdir().unwrap();
        let with_marker = root.path().join("proj-a");
        fs::create_dir_all(with_marker.join("target")).unwrap();
        fs::write(with_marker.join("Cargo.toml"), "").unwrap();

        let without_marker = root.path().join("proj-b");
        fs::create_dir_all(without_marker.join("target")).unwrap();

        let rule = crate::types::Rule {
            id: "rust-target", description: "t", kind: crate::types::Kind::Artifact,
            dir: "target", marker: "Cargo.toml",
            roots: vec![root.path().to_path_buf()],
            paths: vec![], regen: "t",
        };

        assert_eq!(collect(&rule), vec![with_marker.join("target")]);
    }
}
