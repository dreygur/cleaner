use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;

use crate::types::{Group, Kind, Rule, Target};
use crate::walker::collect_artifacts_each;
use crate::{container, temp};

/// Sum of apparent file sizes under root. Symlinks count as their own size
/// and are never followed.
pub fn dir_size(root: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.filter_map(|e| e.ok()) {
            let Ok(ft) = e.file_type() else {
                continue;
            };
            if ft.is_dir() {
                stack.push(e.path());
                continue;
            }
            if let Ok(m) = e.metadata() {
                total += m.len();
            }
        }
    }
    total
}

fn collect_fixed(rule: &Rule) -> Vec<PathBuf> {
    if rule.kind == Kind::Prune {
        return rule.paths.clone();
    }
    rule.paths
        .iter()
        .filter(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_dir()))
        .cloned()
        .collect()
}

/// Bytes removing path under rule would free: the whole tree for build
/// output and caches, only the stale entries of a temp directory, and what
/// the engine reports as reclaimable for a container CLI.
pub fn measure(rule: &Rule, path: &Path) -> u64 {
    match rule.kind {
        Kind::Artifact | Kind::Fixed => dir_size(path),
        Kind::Temp => temp::stale_size(path, SystemTime::now()),
        Kind::Prune => container::reclaimable(path, rule.df).unwrap_or(0),
    }
}

fn unix_secs(t: SystemTime) -> Option<u64> {
    t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Newest modification time among the project's own entries, the folder
/// that holds the artifact, leaving the artifact itself out. Shallow on
/// purpose: one listing per target, so staleness costs almost nothing.
pub fn project_modified(target: &Path) -> Option<u64> {
    let project = target.parent()?;
    let skip = target.file_name()?;
    fs::read_dir(project)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() != skip)
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .filter_map(unix_secs)
        .max()
}

fn measure_all(rule: &Rule, paths: Vec<PathBuf>) -> Vec<Target> {
    paths
        .into_par_iter()
        .map(|path| {
            let size = measure(rule, &path);
            Target { path, size }
        })
        .collect()
}

fn group_of(rule: &Rule, paths: Vec<PathBuf>, keep: &[PathBuf]) -> Option<Group> {
    let paths: Vec<PathBuf> = paths.into_iter().filter(|p| !keep.contains(p)).collect();
    if paths.is_empty() {
        return None;
    }
    let mut targets = measure_all(rule, paths);
    // A temp dir with no leftovers, or an engine that is stopped or has
    // nothing to prune, offers nothing.
    if matches!(rule.kind, Kind::Temp | Kind::Prune) {
        targets.retain(|t| t.size > 0);
        if targets.is_empty() {
            return None;
        }
    }
    let size = targets.iter().map(|t| t.size).sum();
    Some(Group { rule: rule.clone(), targets, size })
}

/// Measures every rule and hands each result to emit as soon as it is ready:
/// fixed caches first, since they need no search, then artifact rules once
/// the walk is done. root_done fires as each search root finishes. Paths on
/// the keep list are dropped before measuring.
pub fn scan_each(
    rules: &[Rule],
    keep: &[PathBuf],
    root_done: impl FnMut(&Path),
    mut emit: impl FnMut(&Rule, Option<Group>),
) {
    for rule in rules.iter().filter(|r| r.kind != Kind::Artifact) {
        emit(rule, group_of(rule, collect_fixed(rule), keep));
    }

    let mut artifacts = collect_artifacts_each(rules, root_done);
    for rule in rules.iter().filter(|r| r.kind == Kind::Artifact) {
        let paths = artifacts.remove(rule.id).unwrap_or_default();
        emit(rule, group_of(rule, paths, keep));
    }
}

/// Locates and measures every directory the given rules match, dropping
/// rules that matched nothing. Groups come back unsorted; the caller sorts.
pub fn scan(rules: &[Rule], keep: &[PathBuf]) -> Vec<Group> {
    let mut groups = Vec::new();
    scan_each(rules, keep, |_| {}, |_, g| groups.extend(g));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::collect_artifacts;

    fn rule(id: &'static str, dir: &'static str, marker: &'static str, root: &Path) -> Rule {
        Rule {
            id,
            description: "",
            kind: Kind::Artifact,
            dir,
            marker,
            roots: vec![root.to_path_buf()],
            prune: Vec::new(),
            paths: Vec::new(),
            commands: &[],
            df: &[],
            safety: crate::types::Safety::Safe,
            regen: crate::types::Regen::Run(""),
        }
    }

    fn mock_tree() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let krate = root.path().join("crate");
        fs::create_dir_all(krate.join("target/debug")).unwrap();
        fs::write(krate.join("Cargo.toml"), "[package]\n").unwrap();
        fs::write(krate.join("target/debug/bin"), vec![0u8; 4096]).unwrap();
        // A "target" directory with no Cargo.toml beside it must never match.
        fs::create_dir_all(root.path().join("notes/target")).unwrap();
        root
    }

    fn mock_project(parts: &[&str]) -> PathBuf {
        let dir: PathBuf = parts.iter().collect();
        fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
        fs::write(dir.join("package.json"), "{}").unwrap();
        dir.join("node_modules")
    }

    #[test]
    fn collect_artifacts_requires_marker() {
        let root = mock_tree();
        let rules = [rule("rust-target", "target", "Cargo.toml", root.path())];

        let got = &collect_artifacts(&rules)["rust-target"];
        assert_eq!(got, &vec![root.path().join("crate/target")]);
    }

    #[test]
    fn collect_artifacts_skips_hidden_and_pruned() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_str().unwrap();
        let deep = mock_project(&[root, "a", "b", "c", "app"]);
        let worktree = mock_project(&[root, "repo", ".claude", "worktrees", "w1"]);
        mock_project(&[root, ".nvm", "versions", "tool"]);
        mock_project(&[root, "repo", ".git", "app"]);
        mock_project(&[root, "Library", "app"]);

        let mut r = rule("node-modules", "node_modules", "package.json", tmp.path());
        r.prune = vec![tmp.path().join("Library")];

        let got = &collect_artifacts(&[r])["node-modules"];
        assert_eq!(got, &vec![deep, worktree]);
    }

    #[test]
    fn collect_artifacts_matches_hidden_rule_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("web");
        fs::create_dir_all(app.join(".next/cache")).unwrap();
        fs::write(app.join("package.json"), "{}").unwrap();

        let got = &collect_artifacts(&[rule("next-build", ".next", "package.json", tmp.path())])["next-build"];
        assert_eq!(got, &vec![app.join(".next")]);
    }

    #[test]
    fn collect_fixed_skips_missing_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let present = tmp.path().join("cache");
        fs::create_dir(&present).unwrap();

        let mut r = rule("cache", "", "", tmp.path());
        r.kind = Kind::Fixed;
        r.paths = vec![present.clone(), tmp.path().join("absent")];

        assert_eq!(collect_fixed(&r), vec![present]);
    }

    #[test]
    fn scan_measures_sizes() {
        let root = mock_tree();
        let groups = scan(&[rule("rust-target", "target", "Cargo.toml", root.path())], &[]);

        assert_eq!(groups.len(), 1);
        assert!(groups[0].size >= 4096, "group size = {}", groups[0].size);
    }

    #[test]
    fn scan_drops_empty_rules() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = rule("nope", "", "", tmp.path());
        r.kind = Kind::Fixed;
        r.paths = vec![tmp.path().join("missing")];

        assert!(scan(&[r], &[]).is_empty());
    }

    #[test]
    fn scan_drops_kept_paths() {
        let root = mock_tree();
        let kept = root.path().join("crate/target");
        let groups = scan(&[rule("rust-target", "target", "Cargo.toml", root.path())], &[kept]);
        assert!(groups.is_empty());
    }

    #[test]
    fn scan_each_emits_fixed_first_then_roots_then_artifacts() {
        let root = mock_tree();
        let cache = root.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let mut fixed = rule("cache", "", "", root.path());
        fixed.kind = Kind::Fixed;
        fixed.paths = vec![cache];
        let rules = [rule("rust-target", "target", "Cargo.toml", root.path()), fixed];

        let log = std::cell::RefCell::new(Vec::new());
        scan_each(
            &rules,
            &[],
            |_| log.borrow_mut().push("root".to_string()),
            |r, g| log.borrow_mut().push(format!("{}:{}", r.id, g.is_some())),
        );
        assert_eq!(log.into_inner(), ["cache:true", "root", "rust-target:true"]);
    }

    #[test]
    fn project_modified_ignores_the_artifact() {
        let root = mock_tree();
        let target = root.path().join("crate/target");
        let modified = project_modified(&target).unwrap();
        let toml = fs::metadata(root.path().join("crate/Cargo.toml")).unwrap().modified().unwrap();
        assert_eq!(modified, unix_secs(toml).unwrap());
    }
}
