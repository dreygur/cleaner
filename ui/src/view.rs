//! Copies the state machine into the window's properties. Every string the
//! user reads is built here, so the Slint files only lay things out.

use std::rc::Rc;

use cleaner::report::{FailReason, RuleKind};
use cleaner::settings::Settings;
use cleaner::util::human_size;
use slint::{ModelRc, SharedString, VecModel};

use crate::state::{App, Check, Group, Outcome, Planned, Screen};
use crate::{AppWindow, Check as UiCheck, FailRow, GroupRow, LineRow, ProgressRow, RootRow, Screen as UiScreen, Seg, TargetRow};

const DAY: u64 = 86_400;
/// Projects untouched this long are called out in teal.
const OLD_AFTER_DAYS: u64 = 90;

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::from(Rc::new(VecModel::from(v)))
}

fn s(text: impl AsRef<str>) -> SharedString {
    text.as_ref().into()
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") }
}

/// `~/...` under home, absolute elsewhere.
pub fn display_path(home: &str, path: &str) -> String {
    if path == home {
        return "~/".into();
    }
    match path.strip_prefix(home).and_then(|r| r.strip_prefix('/')) {
        Some(rel) if !home.is_empty() => format!("~/{rel}"),
        _ => path.into(),
    }
}

/// "untouched 8 months" once a project has sat for 90 days, else how
/// recently it changed. The bool says whether it counts as old.
pub fn age(modified: Option<u64>, now: u64) -> (String, bool) {
    let Some(m) = modified else {
        return (String::new(), false);
    };
    let days = now.saturating_sub(m) / DAY;
    if days >= OLD_AFTER_DAYS {
        return (format!("untouched {} months", days / 30), true);
    }
    let text = match days {
        0 => "modified today".to_string(),
        1..=13 => format!("modified {} ago", plural(days as usize, "day")),
        14..=59 => format!("modified {} ago", plural((days / 7) as usize, "week")),
        _ => format!("modified {} ago", plural((days / 30) as usize, "month")),
    };
    (text, false)
}

/// "2.9 of 9.8 GiB": the unit is written once when both sides share it.
fn part_of(part: u64, whole: u64) -> String {
    let (a, b) = (human_size(part), human_size(whole));
    match (a.rsplit_once(' '), b.rsplit_once(' ')) {
        (Some((na, ua)), Some((_, ub))) if ua == ub => format!("{na} of {b}"),
        _ => format!("{a} of {b}"),
    }
}

fn reason(r: FailReason) -> (&'static str, &'static str) {
    match r {
        FailReason::PermissionDenied => ("Permission denied", "Cleaner has no write access to this folder."),
        FailReason::SymlinkRejected => ("Symlink rejected", "Cleaner never deletes through a symlink."),
        FailReason::Gone => ("Path gone", "It was moved or removed before Cleaner got to it."),
        FailReason::Protected => ("Protected", "This location is always refused."),
        FailReason::NotAllowed => ("Not allowed", "No rule offers this path any more, or it is kept."),
        FailReason::Other => ("Failed", ""),
    }
}

fn friction_tag(id: &str) -> &'static str {
    match id {
        "cargo-registry" => "Needs network to restore · not in Select all",
        "trash" => "Cannot be restored · not in Select all",
        _ => "",
    }
}

fn group_row(app: &App, g: &Group, max: u64, now: u64, selectable: bool) -> GroupRow {
    let r = &g.report;
    let frac = |n: u64| if max == 0 { 0.0 } else { n as f32 / max as f32 };
    let tail = match r.kind {
        RuleKind::Artifact if selectable => format!(" · {}", plural(r.targets.len(), "folder")),
        _ => String::new(),
    };
    let targets = r
        .targets
        .iter()
        .zip(&g.selected)
        .map(|(t, on)| {
            let (age, old) = age(t.modified, now);
            TargetRow {
                path: s(display_path(&app.home, &t.path)),
                age: s(age),
                old,
                size: s(human_size(t.size)),
                checked: *on,
            }
        })
        .collect();
    GroupRow {
        name: s(&r.description),
        slug: s(&r.id),
        regen: s(&r.regen),
        command: r.regen_is_command,
        tail: s(tail),
        tag: s(friction_tag(&r.id)),
        size: s(human_size(r.size)),
        bar: frac(r.size),
        bar_sel: frac(g.selected_size()),
        check: match g.check() {
            Check::None => UiCheck::None,
            Check::Some => UiCheck::Some,
            Check::All => UiCheck::All,
        },
        open: g.open,
        targets: model(targets),
    }
}

fn group_name<'a>(app: &'a App, id: &'a str) -> (&'a str, bool, &'a str) {
    app.groups
        .iter()
        .find(|g| g.report.id == id)
        .map(|g| (g.report.description.as_str(), g.report.regen_is_command, g.report.regen.as_str()))
        .unwrap_or((id, false, ""))
}

/// Planned paths grouped by rule, in plan order.
fn by_rule(plan: &[Planned]) -> Vec<(&str, Vec<&Planned>)> {
    let mut out: Vec<(&str, Vec<&Planned>)> = Vec::new();
    for p in plan {
        match out.iter_mut().find(|(r, _)| *r == p.rule) {
            Some((_, v)) => v.push(p),
            None => out.push((&p.rule, vec![p])),
        }
    }
    out
}

fn freed_of(ps: &[&Planned]) -> u64 {
    ps.iter().map(|p| if let Outcome::Removed(n) = p.outcome { n } else { 0 }).sum()
}

fn roots_rows(app: &App) -> Vec<RootRow> {
    app.roots
        .iter()
        .map(|r| RootRow {
            path: s(display_path(&app.home, r)),
            note: s(""),
            on: true,
            done: app.roots_done.contains(r),
        })
        .collect()
}

fn command_hint(app: &App) -> Vec<Seg> {
    let mut cmds: Vec<&str> = Vec::new();
    for (rule, _) in by_rule(&app.plan) {
        let (_, is_cmd, regen) = group_name(app, rule);
        if is_cmd && !cmds.contains(&regen) {
            cmds.push(regen);
        }
    }
    if cmds.is_empty() {
        return Vec::new();
    }
    let text = |t: &str| Seg { text: s(t), code: false, bold: false };
    let mut segs = vec![text("Need them back? Run ")];
    for (i, c) in cmds.iter().enumerate() {
        if i > 0 {
            segs.push(text(if i == cmds.len() - 1 { " or " } else { ", " }));
        }
        segs.push(Seg { text: s(c), code: true, bold: false });
    }
    segs.push(text(" in the project."));
    segs
}

fn done_rows(app: &App, with_counts: bool) -> Vec<LineRow> {
    by_rule(&app.plan)
        .into_iter()
        .filter(|(_, ps)| ps.iter().any(|p| matches!(p.outcome, Outcome::Removed(_))))
        .map(|(rule, ps)| {
            let removed = ps.iter().filter(|p| matches!(p.outcome, Outcome::Removed(_))).count();
            LineRow {
                name: s(group_name(app, rule).0),
                slug: s(rule),
                note: s(if with_counts { format!("{removed} of {}", plural(ps.len(), "folder")) } else { String::new() }),
                regen: s(""),
                command: false,
                size: s(human_size(freed_of(&ps))),
            }
        })
        .collect()
}

fn render_review(app: &App, w: &AppWindow, now: u64) {
    let max = app.groups.iter().map(|g| g.report.size).max().unwrap_or(0);
    let rows: Vec<GroupRow> = app.groups.iter().map(|g| group_row(app, g, max, now, true)).collect();
    let selected = human_size(app.selected_size());
    let secs = app.elapsed_ms as f64 / 1000.0;

    w.set_eyebrow(s(format!(
        "{} · {} · {secs:.1} s",
        plural(app.groups.len(), "group"),
        plural(app.roots.len(), "disk"),
    )
    .to_uppercase()));
    w.set_big(s(human_size(app.total_size())));
    w.set_free(s(human_size(app.free)));
    w.set_after(s(human_size(app.after_selection())));
    w.set_groups(model(rows));
    w.set_untouched(app.sort == crate::state::Sort::Untouched);
    w.set_folders(s(plural(app.selected_folders(), "folder")));
    w.set_groups_text(s(format!("in {} selected", plural(app.selected_groups(), "group"))));
    w.set_delete_label(s(format!("Delete {selected}…")));
    w.set_confirm_label(s(format!("Delete {selected}")));
    w.set_can_delete(app.selected_folders() > 0);

    w.set_confirm_title(s(format!("Delete {selected} for good?")));
    w.set_confirm_body1(s(format!(
        "{} will be removed right now. They do not go to the",
        plural(app.selected_folders(), "folder")
    )));
    w.set_confirm_body2(s("Trash, and Cleaner cannot bring them back."));
    let confirm: Vec<LineRow> = app
        .groups
        .iter()
        .filter(|g| g.check() != Check::None)
        .map(|g| {
            let n = plural(g.selected.iter().filter(|x| **x).count(), "folder");
            let r = &g.report;
            LineRow {
                name: s(&r.description),
                slug: s(&r.id),
                note: s(format!("{n} · rebuild with")),
                regen: s(if r.regen_is_command { r.regen.clone() } else { format!("{n} · {}", r.regen) }),
                command: r.regen_is_command,
                size: s(human_size(g.selected_size())),
            }
        })
        .collect();
    w.set_confirm_rows(model(confirm));
}

fn render_scanning(app: &App, w: &AppWindow, now: u64) {
    let max = app.groups.iter().map(|g| g.report.size).max().unwrap_or(0);
    let rows: Vec<GroupRow> = app.groups.iter().map(|g| group_row(app, g, max, now, false)).collect();
    let left = app.rules_total.saturating_sub(app.rules_measured);
    w.set_big(s(human_size(app.total_size())));
    w.set_lede(s("Groups appear as soon as they are measured. Nothing is deleted during a scan."));
    w.set_free(s(human_size(app.free)));
    w.set_progress(if app.rules_total == 0 { 0.0 } else { app.rules_measured as f32 / app.rules_total as f32 });
    w.set_roots(model(roots_rows(app)));
    w.set_groups(model(rows));
    w.set_measured(s(format!("{} of {} rules measured", app.rules_measured, app.rules_total)));
    w.set_skeletons(left.min(2) as i32);
}

fn render_deleting(app: &App, w: &AppWindow) {
    let freed = app.freed();
    let planned = app.planned_size();
    let rows: Vec<ProgressRow> = by_rule(&app.plan)
        .into_iter()
        .map(|(rule, ps)| {
            let finished = ps.iter().filter(|p| matches!(p.outcome, Outcome::Removed(_) | Outcome::Failed(..))).count();
            let removing = ps.iter().find(|p| p.outcome == Outcome::Removing);
            let total: u64 = ps.iter().map(|p| p.size).sum();
            let got = freed_of(&ps);
            let state = if finished == ps.len() { 2 } else if finished > 0 || removing.is_some() { 1 } else { 0 };
            ProgressRow {
                name: s(group_name(app, rule).0),
                slug: s(rule),
                state,
                note: s(format!("{finished} of {} removed", plural(ps.len(), "folder"))),
                current: s(removing.map(|p| display_path(&app.home, &p.path)).unwrap_or_default()),
                size: s(match state {
                    2 => human_size(got),
                    1 => part_of(got, total),
                    _ => human_size(total),
                }),
                progress: if total == 0 { 0.0 } else { got as f32 / total as f32 },
            }
        })
        .collect();
    w.set_big(s(human_size(freed)));
    w.set_small(s(format!("freed of {}", human_size(planned))));
    // Live estimate while deleting; the engine reads the real figure at the end.
    w.set_free(s(human_size(app.free_before + freed)));
    w.set_progress(if planned == 0 { 0.0 } else { freed as f32 / planned as f32 });
    w.set_meta(s(format!("{} · {}", plural(rows.len(), "group"), plural(app.plan.len(), "folder"))));
    w.set_progress_rows(model(rows));
    w.set_stopping(app.stopping);
}

fn render_finished(app: &App, w: &AppWindow) {
    let removed = app.removed().count();
    let failed: Vec<&Planned> = app.failed().collect();
    w.set_big(s(human_size(app.freed())));
    w.set_free_before(s(human_size(app.free_before)));
    w.set_free(s(human_size(app.free)));
    w.set_hint(model(command_hint(app)));

    if failed.is_empty() {
        w.set_small(s(format!("freed from {}", plural(removed, "folder"))));
        w.set_done_rows(model(done_rows(app, false)));
        return;
    }
    w.set_small(s(format!("freed from {removed} of {}", plural(app.plan.len(), "folder"))));
    w.set_lede(s(format!(
        "The rest was deleted. {} could not be removed and {} still on disk.",
        plural(failed.len(), "folder"),
        if failed.len() == 1 { "is" } else { "are" }
    )));
    let rows: Vec<FailRow> = failed
        .iter()
        .map(|p| {
            let Outcome::Failed(r, msg) = &p.outcome else { unreachable!() };
            let (tag, detail) = reason(*r);
            FailRow {
                path: s(display_path(&app.home, &p.path)),
                reason: s(tag),
                detail: s(if detail.is_empty() { msg.as_str() } else { detail }),
                size: s(human_size(p.size)),
            }
        })
        .collect();
    w.set_failures(model(rows));
    w.set_done_rows(model(done_rows(app, true)));
    w.set_deleted_title(s(format!("Deleted · {}", plural(removed, "folder"))));
    w.set_retry_label(s(format!("Retry {} failed", failed.len())));
}

/// Search folder switches: the defaults the engine would use plus added
/// folders, each on unless the config switched it off.
pub fn render_settings(w: &AppWindow, home: &str, disks: &[String], settings: &Settings) {
    let row = |path: &str, note: &str| RootRow {
        path: s(display_path(home, path)),
        note: s(note),
        on: !settings.disabled.iter().any(|d| d.to_string_lossy() == path),
        done: false,
    };
    let mut rows = vec![row(home, "Home folder")];
    rows.extend(disks.iter().map(|d| row(d, "Attached local disk · writable")));
    rows.extend(settings.extra_roots.iter().map(|p| row(&p.to_string_lossy(), "Added folder")));
    w.set_setting_roots(model(rows));
    let keep: Vec<SharedString> =
        settings.keep.iter().map(|k| s(display_path(home, &k.to_string_lossy()))).collect();
    w.set_keep(model(keep));
}

pub fn render(app: &App, w: &AppWindow, now: u64) {
    w.set_screen(match app.screen {
        Screen::Scanning => UiScreen::Scanning,
        Screen::Nothing => UiScreen::Nothing,
        Screen::Review => UiScreen::Review,
        Screen::Confirm => UiScreen::Confirm,
        Screen::Deleting => UiScreen::Deleting,
        Screen::Done => UiScreen::Done,
        Screen::Partial => UiScreen::Partial,
        Screen::Settings => UiScreen::Settings,
    });
    match app.screen {
        Screen::Scanning => render_scanning(app, w, now),
        Screen::Nothing => {
            w.set_free(s(human_size(app.free)));
            w.set_roots(model(roots_rows(app)));
        }
        Screen::Review | Screen::Confirm => render_review(app, w, now),
        Screen::Deleting => render_deleting(app, w),
        Screen::Done | Screen::Partial => render_finished(app, w),
        Screen::Settings => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_under_home_get_a_tilde() {
        assert_eq!(display_path("/Users/me", "/Users/me/Code/riku/target"), "~/Code/riku/target");
        assert_eq!(display_path("/Users/me", "/Users/me"), "~/");
        assert_eq!(display_path("/Users/me", "/Users/meow/x"), "/Users/meow/x");
        assert_eq!(display_path("/Users/me", "/Volumes/T7/proj/target"), "/Volumes/T7/proj/target");
    }

    #[test]
    fn ages_read_like_the_design() {
        let now = 1_000 * DAY;
        assert_eq!(age(Some(now - 240 * DAY), now), ("untouched 8 months".into(), true));
        assert_eq!(age(Some(now - 2 * DAY), now), ("modified 2 days ago".into(), false));
        assert_eq!(age(Some(now - 21 * DAY), now), ("modified 3 weeks ago".into(), false));
        assert_eq!(age(None, now), (String::new(), false));
    }

    #[test]
    fn part_of_writes_a_shared_unit_once() {
        let gib = 1u64 << 30;
        assert_eq!(part_of(gib * 29 / 10, gib * 98 / 10), "2.9 of 9.8 GiB");
        assert_eq!(part_of(512, gib), "512 B of 1.0 GiB");
    }
}
