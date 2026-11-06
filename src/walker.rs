use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::types::{Kind, Rule};
use crate::util::clean_path;

/// Holds only history; walking its object store finds nothing.
pub(crate) const GIT_DIR: &str = ".git";

/// Searches every root once for all artifact rules together, fanning
/// directories out across rayon's pool.
///
///   root ─┬─> match (target beside Cargo.toml)   recorded, not entered
///         ├─> pruned, .git, or hidden at the root  skipped
///         ├─> another filesystem mounted inside   skipped
///         └─> anything else                       walked on a free worker
struct Walker<'a> {
    rules: Vec<&'a Rule>,
    prune: HashSet<PathBuf>,
    found: Mutex<Vec<BTreeSet<PathBuf>>>,
}

impl<'a> Walker<'a> {
    fn new(rules: &'a [Rule]) -> Self {
        let rules: Vec<&Rule> = rules.iter().filter(|r| r.kind == Kind::Artifact).collect();
        let prune = rules
            .iter()
            .flat_map(|r| r.prune.iter().map(|p| clean_path(p)))
            .collect();
        let found = Mutex::new(vec![BTreeSet::new(); rules.len()]);
        Walker { rules, prune, found }
    }

    /// Records child when it is a build artifact for any rule. Markers are
    /// checked against the parent's listing, which costs no extra stat.
    fn matches(&self, child: &Path, name: &str, siblings: &HashSet<OsString>) -> bool {
        let mut matched = false;
        for (i, r) in self.rules.iter().enumerate() {
            if r.dir != name {
                continue;
            }
            if !r.marker.is_empty() && !siblings.contains(std::ffi::OsStr::new(r.marker)) {
                continue;
            }
            self.found.lock().unwrap()[i].insert(child.to_path_buf());
            matched = true;
        }
        matched
    }

    /// Hidden directories right under a root hold tool state (~/.nvm,
    /// ~/.cargo) whose node_modules belong to installed tools. Deeper ones,
    /// like a .claude/worktrees checkout, are project trees and are walked.
    fn skip(&self, at_root: bool, name: &str, child: &Path) -> bool {
        if name == GIT_DIR || self.prune.contains(child) {
            return true;
        }
        at_root && name.starts_with('.')
    }

    fn visit<'s>(&'s self, s: &rayon::Scope<'s>, dir: PathBuf, dev: u64, at_root: bool) {
        let Ok(entries) = fs::read_dir(&dir) else {
            return;
        };
        let entries: Vec<fs::DirEntry> = entries.filter_map(|e| e.ok()).collect();
        let siblings: HashSet<OsString> = entries.iter().map(|e| e.file_name()).collect();

        for e in entries {
            let Ok(ft) = e.file_type() else {
                continue;
            };
            if !ft.is_dir() {
                continue;
            }
            let name = e.file_name();
            let name = name.to_string_lossy();
            let child = e.path();
            if self.matches(&child, &name, &siblings) {
                continue;
            }
            if self.skip(at_root, &name, &child) {
                continue;
            }
            match e.metadata() {
                Ok(m) if m.dev() == dev => {}
                _ => continue,
            }
            s.spawn(move |s| self.visit(s, child, dev, false));
        }
    }
}

/// Matched paths for each artifact rule, keyed by rule id and sorted. Roots
/// shared between rules are walked once.
pub fn collect_artifacts(rules: &[Rule]) -> HashMap<&'static str, Vec<PathBuf>> {
    collect_artifacts_each(rules, |_| {})
}

/// Same as collect_artifacts, walking one root at a time and calling
/// root_done after each, so a caller can show which roots are finished.
/// Each root is still walked in parallel inside.
pub fn collect_artifacts_each(
    rules: &[Rule],
    mut root_done: impl FnMut(&Path),
) -> HashMap<&'static str, Vec<PathBuf>> {
    let w = Walker::new(rules);
    let roots: BTreeSet<PathBuf> = w
        .rules
        .iter()
        .flat_map(|r| r.roots.iter().map(|p| clean_path(p)))
        .collect();

    for root in roots {
        if let Ok(meta) = fs::metadata(&root)
            && meta.is_dir()
        {
            let w = &w;
            let start = root.clone();
            rayon::scope(|s| s.spawn(move |s| w.visit(s, start, meta.dev(), true)));
        }
        root_done(&root);
    }

    let found = w.found.into_inner().unwrap();
    w.rules
        .iter()
        .zip(found)
        .map(|(r, set)| (r.id, set.into_iter().collect()))
        .collect()
}
