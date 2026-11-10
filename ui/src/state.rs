//! The UI's state machine, free of any rendering. Engine events and user
//! actions go in; the view reads the fields and derived values back out.
//!
//!   Scanning ─┬─> Review <──> Confirm ─> Deleting ─┬─> Done ──────┐
//!             └─> Nothing                          └─> Partial ───┤
//!                    └──────────── rescan <─────────────────────────┘
//!   Settings is reachable from anywhere and returns where it came from.

use cleaner::report::{DeleteEvent, FailReason, GroupReport, ScanEvent};

/// Rules that get an amber tag and stay out of "Select all safe groups":
/// they need network to restore or cannot be restored.
pub const FRICTION_IDS: [&str; 6] = [
    "cargo-registry",
    "trash",
    "docker-images",
    "docker-volumes",
    "podman-images",
    "podman-volumes",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Scanning,
    Nothing,
    Review,
    Confirm,
    Deleting,
    Done,
    Partial,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Size,
    Untouched,
}

/// Tri-state checkbox value of a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    None,
    Some,
    All,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub report: GroupReport,
    pub selected: Vec<bool>,
    pub open: bool,
}

impl Group {
    pub fn check(&self) -> Check {
        match self.selected.iter().filter(|s| **s).count() {
            0 => Check::None,
            n if n == self.selected.len() => Check::All,
            _ => Check::Some,
        }
    }

    pub fn selected_size(&self) -> u64 {
        self.report.targets.iter().zip(&self.selected).filter(|(_, s)| **s).map(|(t, _)| t.size).sum()
    }

    pub fn friction(&self) -> bool {
        FRICTION_IDS.contains(&self.report.id.as_str())
    }

    /// Oldest project time in the group, for the Untouched sort.
    fn oldest(&self) -> u64 {
        self.report.targets.iter().filter_map(|t| t.modified).min().unwrap_or(u64::MAX)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Pending,
    Removing,
    Removed(u64),
    Failed(FailReason, String),
}

/// One path handed to the engine for deletion.
#[derive(Clone, Debug)]
pub struct Planned {
    pub rule: String,
    pub path: String,
    pub size: u64,
    pub outcome: Outcome,
}

#[derive(Clone, Debug)]
pub struct App {
    pub screen: Screen,
    back_to: Screen,
    pub home: String,
    pub free: u64,
    pub roots: Vec<String>,
    pub roots_done: Vec<String>,
    pub disks: Vec<String>,
    pub rules_total: usize,
    pub rules_measured: usize,
    pub elapsed_ms: u64,
    pub groups: Vec<Group>,
    pub sort: Sort,
    pub plan: Vec<Planned>,
    pub free_before: u64,
    pub stopping: bool,
}

impl Default for App {
    fn default() -> Self {
        App {
            screen: Screen::Scanning,
            back_to: Screen::Scanning,
            home: String::new(),
            free: 0,
            roots: Vec::new(),
            roots_done: Vec::new(),
            disks: Vec::new(),
            rules_total: 0,
            rules_measured: 0,
            elapsed_ms: 0,
            groups: Vec::new(),
            sort: Sort::Size,
            plan: Vec::new(),
            free_before: 0,
            stopping: false,
        }
    }
}

impl App {
    fn resort(&mut self) {
        match self.sort {
            Sort::Size => {
                self.groups.sort_by_key(|g| std::cmp::Reverse(g.report.size));
                for g in &mut self.groups {
                    sort_targets(g, |t| std::cmp::Reverse(t.size));
                }
            }
            Sort::Untouched => {
                self.groups.sort_by_key(|g| g.oldest());
                for g in &mut self.groups {
                    sort_targets(g, |t| t.modified.unwrap_or(u64::MAX));
                }
            }
        }
    }

    pub fn on_scan(&mut self, event: ScanEvent) {
        match event {
            ScanEvent::Started { home, free, roots, disks, rules } => {
                self.home = home;
                self.free = free;
                self.roots = roots;
                self.disks = disks;
                self.rules_total = rules;
            }
            ScanEvent::RootDone { root } => self.roots_done.push(root),
            ScanEvent::Group { group } => {
                let selected = vec![false; group.targets.len()];
                self.groups.push(Group { report: group, selected, open: false });
                self.rules_measured += 1;
                self.resort();
            }
            ScanEvent::Empty { .. } => self.rules_measured += 1,
            ScanEvent::Finished { elapsed_ms } => {
                self.elapsed_ms = elapsed_ms;
                self.screen = if self.groups.is_empty() { Screen::Nothing } else { Screen::Review };
            }
        }
    }

    /// Cancel keeps what was measured so far.
    pub fn cancel_scan(&mut self) {
        if self.screen == Screen::Scanning {
            self.screen = if self.groups.is_empty() { Screen::Nothing } else { Screen::Review };
        }
    }

    pub fn total_size(&self) -> u64 {
        self.groups.iter().map(|g| g.report.size).sum()
    }

    pub fn selected_size(&self) -> u64 {
        self.groups.iter().map(Group::selected_size).sum()
    }

    pub fn selected_folders(&self) -> usize {
        self.groups.iter().flat_map(|g| &g.selected).filter(|s| **s).count()
    }

    pub fn selected_groups(&self) -> usize {
        self.groups.iter().filter(|g| g.check() != Check::None).count()
    }

    /// Free space after deleting the selection: free now plus what is picked.
    pub fn after_selection(&self) -> u64 {
        self.free + self.selected_size()
    }

    pub fn set_sort(&mut self, sort: Sort) {
        self.sort = sort;
        self.resort();
    }

    /// None or some selected selects all; all selected clears.
    pub fn toggle_group(&mut self, i: usize) {
        let g = &mut self.groups[i];
        let on = g.check() != Check::All;
        g.selected.iter_mut().for_each(|s| *s = on);
    }

    pub fn toggle_target(&mut self, i: usize, j: usize) {
        let s = &mut self.groups[i].selected[j];
        *s = !*s;
    }

    pub fn toggle_open(&mut self, i: usize) {
        self.groups[i].open = !self.groups[i].open;
    }

    pub fn select_safe(&mut self) {
        for g in self.groups.iter_mut().filter(|g| !g.friction()) {
            g.selected.iter_mut().for_each(|s| *s = true);
        }
    }

    pub fn clear(&mut self) {
        for g in &mut self.groups {
            g.selected.iter_mut().for_each(|s| *s = false);
        }
    }

    /// Confirm is the only door into deletion, and it needs a selection.
    pub fn open_confirm(&mut self) {
        if self.screen == Screen::Review && self.selected_folders() > 0 {
            self.screen = Screen::Confirm;
        }
    }

    pub fn back_to_review(&mut self) {
        if self.screen == Screen::Confirm {
            self.screen = Screen::Review;
        }
    }

    fn start_deleting(&mut self) -> Vec<String> {
        self.screen = Screen::Deleting;
        self.stopping = false;
        self.free_before = self.free;
        self.plan.iter().map(|p| p.path.clone()).collect()
    }

    /// Turns the selection into the path list for the engine.
    pub fn confirm(&mut self) -> Vec<String> {
        if self.screen != Screen::Confirm {
            return Vec::new();
        }
        self.plan = self
            .groups
            .iter()
            .flat_map(|g| {
                g.report.targets.iter().zip(&g.selected).filter(|(_, s)| **s).map(|(t, _)| Planned {
                    rule: g.report.id.clone(),
                    path: t.path.clone(),
                    size: t.size,
                    outcome: Outcome::Pending,
                })
            })
            .collect();
        self.start_deleting()
    }

    pub fn stop(&mut self) {
        if self.screen == Screen::Deleting {
            self.stopping = true;
        }
    }

    fn outcome(&mut self, path: &str, outcome: Outcome) {
        if let Some(p) = self.plan.iter_mut().find(|p| p.path == path) {
            p.outcome = outcome;
        }
    }

    pub fn on_delete(&mut self, event: DeleteEvent) {
        match event {
            DeleteEvent::Started { .. } | DeleteEvent::Stopped => {}
            DeleteEvent::Removing { path, .. } => self.outcome(&path, Outcome::Removing),
            DeleteEvent::Removed { path, freed, .. } => self.outcome(&path, Outcome::Removed(freed)),
            DeleteEvent::Failed { path, reason, message, .. } => self.outcome(&path, Outcome::Failed(reason, message)),
            DeleteEvent::Finished { free_after, .. } => {
                self.free = free_after;
                self.screen = if self.failed().next().is_some() { Screen::Partial } else { Screen::Done };
            }
        }
    }

    pub fn failed(&self) -> impl Iterator<Item = &Planned> {
        self.plan.iter().filter(|p| matches!(p.outcome, Outcome::Failed(..)))
    }

    pub fn removed(&self) -> impl Iterator<Item = &Planned> {
        self.plan.iter().filter(|p| matches!(p.outcome, Outcome::Removed(_)))
    }

    pub fn freed(&self) -> u64 {
        self.plan
            .iter()
            .map(|p| match p.outcome {
                Outcome::Removed(n) => n,
                _ => 0,
            })
            .sum()
    }

    pub fn planned_size(&self) -> u64 {
        self.plan.iter().map(|p| p.size).sum()
    }

    /// Re-runs only the paths that failed.
    pub fn retry_failed(&mut self) -> Vec<String> {
        if self.screen != Screen::Partial {
            return Vec::new();
        }
        self.plan.retain(|p| matches!(p.outcome, Outcome::Failed(..)));
        self.plan.iter_mut().for_each(|p| p.outcome = Outcome::Pending);
        self.start_deleting()
    }

    /// Back to a fresh scan, keeping the chosen sort.
    pub fn rescan(&mut self) {
        *self = App { sort: self.sort, ..App::default() };
    }

    pub fn open_settings(&mut self) {
        if self.screen != Screen::Settings {
            self.back_to = self.screen;
            self.screen = Screen::Settings;
        }
    }

    pub fn close_settings(&mut self) {
        if self.screen == Screen::Settings {
            self.screen = self.back_to;
        }
    }
}

fn sort_targets<K: Ord>(g: &mut Group, key: impl Fn(&cleaner::report::TargetReport) -> K) {
    let mut pairs: Vec<_> = g.report.targets.drain(..).zip(g.selected.drain(..)).collect();
    pairs.sort_by_key(|(t, _)| key(t));
    (g.report.targets, g.selected) = pairs.into_iter().unzip();
}

#[cfg(test)]
mod tests {
    use cleaner::report::{RuleKind, TargetReport};

    use super::*;

    fn group(id: &str, sizes: &[(u64, u64)]) -> ScanEvent {
        let targets: Vec<TargetReport> = sizes
            .iter()
            .enumerate()
            .map(|(i, (size, modified))| TargetReport {
                path: format!("/h/{id}/{i}"),
                size: *size,
                modified: Some(*modified),
            })
            .collect();
        ScanEvent::Group {
            group: GroupReport {
                id: id.into(),
                description: id.into(),
                kind: RuleKind::Artifact,
                regen: "x".into(),
                regen_is_command: true,
                size: sizes.iter().map(|(s, _)| s).sum(),
                targets,
            },
        }
    }

    fn scanned() -> App {
        let mut app = App::default();
        app.on_scan(ScanEvent::Started {
            home: "/h".into(),
            free: 100,
            roots: vec!["/h".into()],
            disks: vec![],
            rules: 4,
        });
        app.on_scan(group("node-modules", &[(30, 50), (10, 5)]));
        app.on_scan(group("rust-target", &[(5, 1), (60, 9)]));
        app.on_scan(group("trash", &[(7, 2)]));
        app.on_scan(ScanEvent::Empty { id: "pycache".into() });
        app.on_scan(ScanEvent::Finished { elapsed_ms: 1400 });
        app
    }

    #[test]
    fn scan_sorts_largest_first_and_lands_on_review() {
        let app = scanned();
        assert_eq!(app.screen, Screen::Review);
        assert_eq!(app.rules_measured, 4);
        let ids: Vec<&str> = app.groups.iter().map(|g| g.report.id.as_str()).collect();
        assert_eq!(ids, ["rust-target", "node-modules", "trash"]);
        assert_eq!(app.groups[0].report.targets[0].size, 60);
        assert_eq!(app.selected_folders(), 0, "nothing preselected");
    }

    #[test]
    fn empty_scan_lands_on_nothing() {
        let mut app = App::default();
        app.on_scan(ScanEvent::Finished { elapsed_ms: 1 });
        assert_eq!(app.screen, Screen::Nothing);
    }

    #[test]
    fn cancel_keeps_groups_found_so_far() {
        let mut app = App::default();
        app.on_scan(group("trash", &[(7, 2)]));
        app.cancel_scan();
        assert_eq!(app.screen, Screen::Review);
        assert_eq!(app.groups.len(), 1);
    }

    #[test]
    fn untouched_sort_puts_oldest_first() {
        let mut app = scanned();
        app.set_sort(Sort::Untouched);
        let ids: Vec<&str> = app.groups.iter().map(|g| g.report.id.as_str()).collect();
        assert_eq!(ids, ["rust-target", "trash", "node-modules"]);
        assert_eq!(app.groups[0].report.targets[0].modified, Some(1));
    }

    #[test]
    fn selection_is_tri_state_and_drives_totals() {
        let mut app = scanned();
        app.toggle_target(0, 0);
        assert_eq!(app.groups[0].check(), Check::Some);
        app.toggle_group(0);
        assert_eq!(app.groups[0].check(), Check::All);
        assert_eq!(app.selected_size(), 65);
        assert_eq!(app.after_selection(), 165);
        app.toggle_group(0);
        assert_eq!(app.groups[0].check(), Check::None);
    }

    #[test]
    fn select_safe_skips_friction_groups() {
        let mut app = scanned();
        app.select_safe();
        assert_eq!(app.groups[2].check(), Check::None, "trash stays unselected");
        assert_eq!(app.selected_groups(), 2);
        app.clear();
        assert_eq!(app.selected_folders(), 0);
    }

    #[test]
    fn confirm_needs_a_selection_and_back_keeps_it() {
        let mut app = scanned();
        app.open_confirm();
        assert_eq!(app.screen, Screen::Review);
        app.toggle_group(1);
        app.open_confirm();
        assert_eq!(app.screen, Screen::Confirm);
        app.back_to_review();
        assert_eq!(app.screen, Screen::Review);
        assert_eq!(app.selected_folders(), 2);
    }

    #[test]
    fn delete_ends_in_done_or_partial_and_retry_runs_only_failures() {
        let mut app = scanned();
        app.toggle_group(1);
        app.open_confirm();
        let paths = app.confirm();
        assert_eq!(paths.len(), 2);
        assert_eq!(app.screen, Screen::Deleting);

        app.on_delete(DeleteEvent::Removed { rule: "node-modules".into(), path: paths[0].clone(), freed: 30 });
        app.on_delete(DeleteEvent::Failed {
            rule: Some("node-modules".into()),
            path: paths[1].clone(),
            reason: FailReason::PermissionDenied,
            message: "denied".into(),
        });
        app.on_delete(DeleteEvent::Finished { freed: 30, free_after: 130 });
        assert_eq!(app.screen, Screen::Partial);
        assert_eq!(app.freed(), 30);
        assert_eq!(app.free, 130);

        let retry = app.retry_failed();
        assert_eq!(retry, [paths[1].clone()]);
        app.on_delete(DeleteEvent::Removed { rule: "node-modules".into(), path: paths[1].clone(), freed: 10 });
        app.on_delete(DeleteEvent::Finished { freed: 10, free_after: 140 });
        assert_eq!(app.screen, Screen::Done);
    }

    #[test]
    fn stop_only_while_deleting_and_rescan_resets() {
        let mut app = scanned();
        app.stop();
        assert!(!app.stopping);
        app.set_sort(Sort::Untouched);
        app.rescan();
        assert_eq!(app.screen, Screen::Scanning);
        assert!(app.groups.is_empty());
        assert_eq!(app.sort, Sort::Untouched);
    }

    #[test]
    fn settings_returns_to_previous_screen() {
        let mut app = scanned();
        app.open_settings();
        assert_eq!(app.screen, Screen::Settings);
        app.close_settings();
        assert_eq!(app.screen, Screen::Review);
    }
}
