//! `--json` mode: one event per line on stdout, read by the UI.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use rayon::prelude::*;

use crate::clean::{self, Scope};
use crate::error::CleanError;
use crate::report::{DeleteEvent, FailReason, GroupReport, RuleKind, ScanEvent, TargetReport};
use crate::scan::{self, project_modified};
use crate::types::{Group, Kind, Regen, Rule, Target};

/// Serializes events from any thread, one whole line at a time.
pub struct Emitter<W: Write> {
    out: Mutex<W>,
}

impl<W: Write> Emitter<W> {
    pub fn new(out: W) -> Self {
        Emitter { out: Mutex::new(out) }
    }

    /// A broken pipe means the UI went away; there is nobody left to tell.
    pub fn emit<T: serde::Serialize>(&self, event: &T) {
        let mut out = self.out.lock().unwrap();
        if serde_json::to_writer(&mut *out, event).is_ok() {
            let _ = out.write_all(b"\n");
            let _ = out.flush();
        }
    }

    pub fn into_inner(self) -> W {
        self.out.into_inner().unwrap()
    }
}

fn text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn group_report(g: &Group) -> GroupReport {
    let kind = match g.rule.kind {
        Kind::Artifact => RuleKind::Artifact,
        Kind::Fixed => RuleKind::Fixed,
    };
    let targets = g
        .targets
        .iter()
        .map(|t| TargetReport {
            path: text(&t.path),
            size: t.size,
            modified: (kind == RuleKind::Artifact).then(|| project_modified(&t.path)).flatten(),
        })
        .collect();
    GroupReport {
        id: g.rule.id.to_string(),
        description: g.rule.description.to_string(),
        kind,
        regen: g.rule.regen.text().to_string(),
        regen_is_command: matches!(g.rule.regen, Regen::Run(_)),
        size: g.size,
        targets,
    }
}

/// What a scan is about to search, for the Started line.
pub struct ScanPlan<'a> {
    pub home: &'a Path,
    pub free: u64,
    pub roots: &'a [PathBuf],
    pub disks: &'a [PathBuf],
}

/// Streams a scan: Started, then RootDone, Group and Empty as they happen,
/// then Finished.
pub fn scan_json<W: Write>(plan: &ScanPlan, rules: &[Rule], keep: &[PathBuf], out: &Emitter<W>) {
    let start = Instant::now();
    out.emit(&ScanEvent::Started {
        home: text(plan.home),
        free: plan.free,
        roots: plan.roots.iter().map(|p| text(p)).collect(),
        disks: plan.disks.iter().map(|p| text(p)).collect(),
        rules: rules.len(),
    });

    scan::scan_each(
        rules,
        keep,
        |root| out.emit(&ScanEvent::RootDone { root: text(root) }),
        |rule, group| match group {
            Some(g) => out.emit(&ScanEvent::Group { group: group_report(&g) }),
            None => out.emit(&ScanEvent::Empty { id: rule.id.to_string() }),
        },
    );

    out.emit(&ScanEvent::Finished { elapsed_ms: start.elapsed().as_millis() as u64 });
}

fn io_reason(e: &io::Error) -> FailReason {
    match e.kind() {
        io::ErrorKind::PermissionDenied => FailReason::PermissionDenied,
        io::ErrorKind::NotFound => FailReason::Gone,
        _ => FailReason::Other,
    }
}

/// Maps an engine error onto the reason tag the UI shows.
pub fn reason(e: &CleanError) -> FailReason {
    match e {
        CleanError::Symlink(_) => FailReason::SymlinkRejected,
        CleanError::Protected(_) | CleanError::OutsideHome(_) => FailReason::Protected,
        CleanError::NotDirectory(_) => FailReason::Other,
        CleanError::Stat { source, .. }
        | CleanError::Remove { source, .. }
        | CleanError::Empty { source, .. } => io_reason(source),
    }
}

fn remove_one<W: Write + Send>(scope: &Scope, rule: &Rule, path: &Path, out: &Emitter<W>) -> u64 {
    out.emit(&DeleteEvent::Removing { rule: rule.id.to_string(), path: text(path) });
    // Measured here, just before removal, so the freed figure is the engine's
    // own and not whatever the caller last saw.
    let target = Target { path: path.to_path_buf(), size: scan::dir_size(path) };
    match clean::remove_target(scope, rule.kind, &target) {
        Ok(()) => {
            out.emit(&DeleteEvent::Removed {
                rule: rule.id.to_string(),
                path: text(path),
                freed: target.size,
            });
            target.size
        }
        Err(e) => {
            out.emit(&DeleteEvent::Failed {
                rule: Some(rule.id.to_string()),
                path: text(path),
                reason: reason(&e),
                message: e.to_string(),
            });
            0
        }
    }
}

/// Deletes an explicit path list. Paths that no rule would have offered are
/// refused before anything is touched. The rest run one rule group at a
/// time, in rule order, with the group's paths in parallel. An interrupt
/// lets the running group finish and skips the others.
pub fn delete_json<W: Write + Send>(
    scope: &Scope,
    rules: &[Rule],
    keep: &[PathBuf],
    paths: &[PathBuf],
    free: impl Fn() -> u64,
    interrupted: &AtomicBool,
    out: &Emitter<W>,
) {
    out.emit(&DeleteEvent::Started { free: free(), targets: paths.len() });

    let mut by_rule: Vec<(&Rule, Vec<&PathBuf>)> = rules.iter().map(|r| (r, Vec::new())).collect();
    for p in paths {
        match clean::resolve(rules, keep, p) {
            Some(rule) => {
                let slot = by_rule.iter_mut().find(|(r, _)| r.id == rule.id).unwrap();
                slot.1.push(p);
            }
            None => out.emit(&DeleteEvent::Failed {
                rule: None,
                path: text(p),
                reason: FailReason::NotAllowed,
                message: format!("{p:?}: not something a scan would offer"),
            }),
        }
    }

    let mut freed = 0;
    for (rule, group) in by_rule.iter().filter(|(_, g)| !g.is_empty()) {
        if interrupted.load(Ordering::SeqCst) {
            out.emit(&DeleteEvent::Stopped);
            break;
        }
        freed += group.par_iter().map(|p| remove_one(scope, rule, p, out)).sum::<u64>();
    }

    out.emit(&DeleteEvent::Finished { freed, free_after: free() });
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::config;

    fn events<T: serde::de::DeserializeOwned>(bytes: Vec<u8>) -> Vec<T> {
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn project(root: &Path, name: &str) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(dir.join("target/debug")).unwrap();
        fs::write(dir.join("Cargo.toml"), "").unwrap();
        fs::write(dir.join("target/debug/bin"), vec![0u8; 3000]).unwrap();
        dir.join("target")
    }

    #[test]
    fn scan_json_streams_every_rule() {
        let home = tempfile::tempdir().unwrap();
        let target = project(home.path(), "app");
        let roots = [home.path().to_path_buf()];
        let rules = config::rules(home.path(), &roots);
        let out = Emitter::new(Vec::new());
        let plan = ScanPlan { home: home.path(), free: 7, roots: &roots, disks: &[] };

        scan_json(&plan, &rules, &[], &out);
        let got: Vec<ScanEvent> = events(out.into_inner());

        assert!(matches!(got.first(), Some(ScanEvent::Started { free: 7, rules: 13, .. })));
        assert!(matches!(got.last(), Some(ScanEvent::Finished { .. })));
        let measured = got
            .iter()
            .filter(|e| matches!(e, ScanEvent::Group { .. } | ScanEvent::Empty { .. }))
            .count();
        assert_eq!(measured, 13);
        let group = got
            .iter()
            .find_map(|e| match e {
                ScanEvent::Group { group } if group.id == "rust-target" => Some(group),
                _ => None,
            })
            .unwrap();
        assert_eq!(group.targets[0].path, text(&target));
        assert!(group.targets[0].modified.is_some());
        assert!(got.iter().any(|e| matches!(e, ScanEvent::RootDone { .. })));
    }

    #[test]
    fn delete_json_refuses_unknown_paths_and_reports_each_target() {
        let home = tempfile::tempdir().unwrap();
        let good = project(home.path(), "app");
        let stray = home.path().join("notes");
        fs::create_dir(&stray).unwrap();
        let gone = home.path().join("old/target");
        fs::create_dir_all(home.path().join("old")).unwrap();
        fs::write(home.path().join("old/Cargo.toml"), "").unwrap();

        let roots = [home.path().to_path_buf()];
        let rules = config::rules(home.path(), &roots);
        let scope = Scope { home: home.path().to_path_buf(), volumes: Vec::new() };
        let out = Emitter::new(Vec::new());
        let paths = [good.clone(), stray.clone(), gone.clone()];

        delete_json(&scope, &rules, &[], &paths, || 1, &AtomicBool::new(false), &out);
        let got: Vec<DeleteEvent> = events(out.into_inner());

        assert!(!good.exists());
        assert!(stray.exists(), "refused path was touched");
        assert!(got.contains(&DeleteEvent::Removed {
            rule: "rust-target".into(),
            path: text(&good),
            freed: 3000,
        }));
        assert!(got.iter().any(|e| matches!(e,
            DeleteEvent::Failed { path, reason: FailReason::NotAllowed, .. } if *path == text(&stray))));
        assert!(got.iter().any(|e| matches!(e,
            DeleteEvent::Failed { path, reason: FailReason::Gone, .. } if *path == text(&gone))));
        assert!(matches!(got.last(), Some(DeleteEvent::Finished { freed: 3000, .. })));
    }

    #[test]
    fn delete_json_stops_before_the_next_group() {
        let home = tempfile::tempdir().unwrap();
        let target = project(home.path(), "app");
        let roots = [home.path().to_path_buf()];
        let rules = config::rules(home.path(), &roots);
        let scope = Scope { home: home.path().to_path_buf(), volumes: Vec::new() };
        let out = Emitter::new(Vec::new());

        delete_json(&scope, &rules, &[], std::slice::from_ref(&target), || 0, &AtomicBool::new(true), &out);
        let got: Vec<DeleteEvent> = events(out.into_inner());

        assert!(target.exists());
        assert!(got.contains(&DeleteEvent::Stopped));
    }
}
