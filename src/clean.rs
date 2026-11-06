use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::error::{CleanError, Result};
use crate::remove;
use crate::types::{Group, Kind, Rule, Target};
use crate::util::clean_path;

/// Directories directly under home that hold user data and must never be
/// deleted, even if a rule somehow names one.
const PROTECTED_NAMES: &[&str] = &[
    "Documents", "Desktop", "Downloads", "Pictures", "Movies", "Music",
    "Library", "Applications", "Public",
];

/// Everywhere deletion may happen: the home directory, under its own guards,
/// and the mount points of attached disks.
#[derive(Clone, Debug)]
pub struct Scope {
    pub home: PathBuf,
    pub volumes: Vec<PathBuf>,
}

/// Path relative to base, or None when path is base itself or lies outside
/// it. strip_prefix compares whole components, so /Users/meow is not under
/// /Users/me.
fn within<'p>(base: &Path, path: &'p Path) -> Option<&'p Path> {
    let rel = path.strip_prefix(clean_path(base)).ok()?;
    if rel.as_os_str().is_empty() {
        return None;
    }
    Some(rel)
}

fn on_volume(volumes: &[PathBuf], path: &Path) -> bool {
    volumes.iter().any(|v| within(v, path).is_some())
}

fn validate(scope: &Scope, path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(CleanError::OutsideHome(path.to_path_buf()));
    }
    let clean = clean_path(path);
    if clean == clean_path(&scope.home) {
        return Err(CleanError::Protected(clean));
    }
    let rel = within(&scope.home, &clean);
    if rel.is_none() && !on_volume(&scope.volumes, &clean) {
        return Err(CleanError::OutsideHome(clean));
    }
    if rel.is_some_and(|r| PROTECTED_NAMES.iter().any(|n| r == Path::new(n))) {
        return Err(CleanError::Protected(clean));
    }

    let meta = fs::symlink_metadata(&clean).map_err(|source| CleanError::Stat {
        path: clean.clone(),
        source,
    })?;
    if meta.file_type().is_symlink() {
        return Err(CleanError::Symlink(clean));
    }
    if !meta.is_dir() {
        return Err(CleanError::NotDirectory(clean));
    }
    Ok(())
}

/// The rule that would have matched path during a scan, or None. A path
/// list handed to `--delete` comes from outside, so each entry must be
/// something a scan could have offered: a fixed cache path, or a rule's
/// directory with its marker beside it under one of the rule's roots. The
/// keep list is refused here too.
pub fn resolve<'r>(rules: &'r [Rule], keep: &[PathBuf], path: &Path) -> Option<&'r Rule> {
    let path = clean_path(path);
    if keep.iter().any(|k| clean_path(k) == path) {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    let parent = path.parent()?;

    rules.iter().find(|r| match r.kind {
        Kind::Fixed => r.paths.iter().any(|p| clean_path(p) == path),
        Kind::Artifact => {
            r.dir == name
                && (r.marker.is_empty() || parent.join(r.marker).exists())
                && r.roots.iter().any(|root| within(root, &path).is_some())
                && !r.prune.iter().any(|p| path.starts_with(clean_path(p)))
        }
    })
}

/// Validates target again and removes it.
pub fn remove_target(scope: &Scope, kind: Kind, target: &Target) -> Result<()> {
    validate(scope, &target.path)?;
    let path = target.path.clone();

    // Fixed caches keep their directory so the owning tool finds it next
    // run; build artifacts are recreated wholesale, so the directory goes too.
    match kind {
        Kind::Fixed => remove::contents(&target.path).map_err(|source| CleanError::Empty { path, source }),
        Kind::Artifact => remove::tree(&target.path).map_err(|source| CleanError::Remove { path, source }),
    }
}

/// Deletes every target in one group concurrently and returns the bytes
/// freed with any per target failures. One failure never stops the rest.
pub fn group(scope: &Scope, group: &Group) -> (u64, Vec<CleanError>) {
    let results: Vec<(u64, Option<CleanError>)> = group
        .targets
        .par_iter()
        .map(|t| match remove_target(scope, group.rule.kind, t) {
            Ok(()) => (t.size, None),
            Err(e) => (0, Some(e)),
        })
        .collect();

    let freed = results.iter().map(|(n, _)| n).sum();
    let failures = results.into_iter().filter_map(|(_, e)| e).collect();
    (freed, failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_dir(parts: &[&Path]) -> PathBuf {
        let path: PathBuf = parts.iter().collect();
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("blob"), vec![0u8; 2048]).unwrap();
        path
    }

    fn home_scope(home: &Path) -> Scope {
        Scope { home: home.to_path_buf(), volumes: Vec::new() }
    }

    fn rule(kind: Kind) -> Rule {
        Rule {
            id: "test",
            description: "",
            kind,
            dir: "",
            marker: "",
            roots: Vec::new(),
            prune: Vec::new(),
            paths: Vec::new(),
            regen: crate::types::Regen::Run(""),
        }
    }

    #[test]
    fn validate_rejects_home_itself() {
        let home = tempfile::tempdir().unwrap();
        let err = validate(&home_scope(home.path()), home.path());
        assert!(matches!(err, Err(CleanError::Protected(_))), "{err:?}");
    }

    #[test]
    fn validate_rejects_protected_names() {
        let home = tempfile::tempdir().unwrap();
        let docs = mock_dir(&[home.path(), Path::new("Documents")]);
        let err = validate(&home_scope(home.path()), &docs);
        assert!(matches!(err, Err(CleanError::Protected(_))), "{err:?}");
    }

    #[test]
    fn validate_rejects_outside_home() {
        let (home, other) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let outside = mock_dir(&[other.path(), Path::new("elsewhere")]);
        let err = validate(&home_scope(home.path()), &outside);
        assert!(matches!(err, Err(CleanError::OutsideHome(_))), "{err:?}");
    }

    #[test]
    fn validate_rejects_symlink() {
        let home = tempfile::tempdir().unwrap();
        let real = mock_dir(&[home.path(), Path::new("real")]);
        let link = home.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let err = validate(&home_scope(home.path()), &link);
        assert!(matches!(err, Err(CleanError::Symlink(_))), "{err:?}");
    }

    #[test]
    fn resolve_accepts_only_what_a_scan_could_offer() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("Code");
        let target = mock_dir(&[&root, Path::new("app/target")]);
        fs::write(root.join("app/Cargo.toml"), "").unwrap();
        let stray = mock_dir(&[&root, Path::new("notes/target")]);
        let cache = mock_dir(&[home.path(), Path::new("Library/Caches/pip")]);

        let mut artifact = rule(Kind::Artifact);
        artifact.dir = "target";
        artifact.marker = "Cargo.toml";
        artifact.roots = vec![root.clone()];
        let mut fixed = rule(Kind::Fixed);
        fixed.paths = vec![cache.clone()];
        let rules = [artifact, fixed];

        assert!(resolve(&rules, &[], &target).is_some_and(|r| r.kind == Kind::Artifact));
        assert!(resolve(&rules, &[], &cache).is_some_and(|r| r.kind == Kind::Fixed));
        assert!(resolve(&rules, &[], &stray).is_none(), "no marker beside it");
        assert!(resolve(&rules, &[], home.path()).is_none());
        assert!(resolve(&rules, std::slice::from_ref(&target), &target).is_none(), "kept");
        assert!(resolve(&rules, &[], &cache.join("sub")).is_none());
    }

    #[test]
    fn validate_allows_attached_disk() {
        let (home, disk) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let target = mock_dir(&[disk.path(), Path::new("proj/target")]);
        let scope = Scope { home: home.path().to_path_buf(), volumes: vec![disk.path().to_path_buf()] };

        assert!(validate(&scope, &target).is_ok());
        let err = validate(&scope, disk.path());
        assert!(matches!(err, Err(CleanError::OutsideHome(_))), "{err:?}");
    }

    #[test]
    fn group_removes_artifact_directory() {
        let home = tempfile::tempdir().unwrap();
        let target = mock_dir(&[home.path(), Path::new("Code/app/target")]);
        let g = Group {
            rule: rule(Kind::Artifact),
            targets: vec![Target { path: target.clone(), size: 2048 }],
            size: 2048,
        };

        let (freed, failures) = group(&home_scope(home.path()), &g);
        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(freed, 2048);
        assert!(!target.exists());
    }

    #[test]
    fn group_empties_fixed_directory_but_keeps_it() {
        let home = tempfile::tempdir().unwrap();
        let cache = mock_dir(&[home.path(), Path::new("Library/Caches/pip")]);
        let g = Group {
            rule: rule(Kind::Fixed),
            targets: vec![Target { path: cache.clone(), size: 2048 }],
            size: 2048,
        };

        let (_, failures) = group(&home_scope(home.path()), &g);
        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn group_reports_failure_without_stopping() {
        let home = tempfile::tempdir().unwrap();
        let good = mock_dir(&[home.path(), Path::new("Code/app/target")]);
        let bad = home.path().join("Code/gone/target");
        let g = Group {
            rule: rule(Kind::Artifact),
            targets: vec![Target { path: bad, size: 999 }, Target { path: good.clone(), size: 2048 }],
            size: 3047,
        };

        let (freed, failures) = group(&home_scope(home.path()), &g);
        assert_eq!(failures.len(), 1);
        assert_eq!(freed, 2048);
        assert!(!good.exists(), "good target survived a sibling failure");
    }
}
