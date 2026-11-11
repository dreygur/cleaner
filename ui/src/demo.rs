//! The design mockups' sample data, driven through the real state machine.
//! Used only by `--shot` to render a screen for comparison with its PNG.

use cleaner::report::{DeleteEvent, FailReason, GroupReport, RuleKind, ScanEvent, TargetReport};
use cleaner::settings::Settings;

use crate::state::{App, Screen};

pub const HOME: &str = "/Users/me";
pub const DISK: &str = "/Volumes/T7";
const DAY: u64 = 86_400;
pub const NOW: u64 = 1_800_000_000;

fn gib(x: f64) -> u64 {
    (x * (1u64 << 30) as f64).round() as u64
}

fn target(path: &str, size: f64, days_ago: Option<u64>) -> TargetReport {
    let path = match path.strip_prefix("~/") {
        Some(rel) => format!("{HOME}/{rel}"),
        None => path.to_string(),
    };
    TargetReport { path, size: gib(size), modified: days_ago.map(|d| NOW - d * DAY) }
}

fn group(id: &str, name: &str, regen: &str, command: bool, kind: RuleKind, targets: Vec<TargetReport>) -> ScanEvent {
    ScanEvent::Group {
        group: GroupReport {
            id: id.into(),
            description: name.into(),
            kind,
            regen: regen.into(),
            regen_is_command: command,
            size: targets.iter().map(|t| t.size).sum(),
            targets,
        },
    }
}

fn fixed(id: &str, name: &str, regen: &str, command: bool, size: f64) -> ScanEvent {
    group(id, name, regen, command, RuleKind::Fixed, vec![target(&format!("~/Library/Caches/{id}"), size, None)])
}

fn started() -> ScanEvent {
    ScanEvent::Started {
        home: HOME.into(),
        free: gib(57.4),
        roots: vec![HOME.into(), DISK.into()],
        disks: vec![DISK.into()],
        rules: 13,
    }
}

fn rust() -> ScanEvent {
    group("rust-target", "Cargo build output", "cargo build", true, RuleKind::Artifact, vec![
        target("/Volumes/T7/proj/target", 5.9, Some(240)),
        target("~/Code/riku/target", 4.2, Some(2)),
        target("~/Code/knot/target", 3.6, Some(150)),
        target("~/Code/cdp-protocol/target", 3.2, Some(21)),
    ])
}

fn node() -> ScanEvent {
    group("node-modules", "Node dependencies", "npm install", true, RuleKind::Artifact, vec![
        target("~/Code/web/node_modules", 2.9, Some(60)),
        target("~/Code/old-api/node_modules", 3.2, Some(300)),
        target("~/Code/site/node_modules", 2.4, Some(10)),
        target("~/Code/legacy/web/node_modules", 1.3, Some(400)),
    ])
}

fn xcode() -> ScanEvent {
    fixed("xcode-derived", "Xcode DerivedData", "Xcode rebuilds it on the next build", false, 6.4)
}

fn scanned() -> App {
    let mut app = App::default();
    app.on_scan(started());
    app.on_scan(ScanEvent::RootDone { root: HOME.into() });
    for e in [
        rust(),
        node(),
        xcode(),
        fixed("cargo-registry", "Cargo registry cache", "cargo build", true, 3.1),
        fixed("pnpm", "pnpm store", "pnpm install", true, 2.7),
        fixed("next-build", "Next.js build output", "next build", true, 2.0),
        fixed("go-build", "Go build cache", "go build", true, 1.5),
        fixed("brew-cache", "Downloaded Homebrew bottles", "Homebrew downloads them again when needed", false, 1.1),
        fixed("pip-cache", "pip wheel cache", "pip downloads it again on the next install", false, 0.7),
        fixed("chrome-cache", "Chrome HTTP cache", "Chrome refills it as you browse", false, 0.5),
        fixed("pycache", "Compiled Python bytecode", "Python rebuilds it on the next import", false, 0.4),
    ] {
        app.on_scan(e);
    }
    app.on_scan(ScanEvent::Finished { elapsed_ms: 1400 });
    app
}

fn selected() -> App {
    let mut app = scanned();
    app.toggle_open(0);
    app.toggle_target(0, 0);
    app.toggle_target(0, 2);
    app.toggle_group(1);    app
}

fn deleting() -> (App, Vec<String>) {
    let mut app = selected();
    app.open_confirm();
    app.confirm();
    let paths = app.plan.iter().map(|p| p.path.clone()).collect();
    (app, paths)
}

fn rule_of(app: &App, path: &str) -> String {
    app.plan.iter().find(|p| p.path == path).map(|p| p.rule.clone()).unwrap_or_default()
}

fn removed(app: &mut App, path: &str, size: f64) {
    let rule = rule_of(app, path);
    app.on_delete(DeleteEvent::Removed { rule, path: path.into(), freed: gib(size) });
}

fn failed(app: &mut App, path: &str, reason: FailReason) {
    app.on_delete(DeleteEvent::Failed { rule: None, path: path.into(), reason, message: String::new() });
}

/// The App for one mockup screen, by its file name.
pub fn app_for(name: &str) -> Option<App> {
    let nm = |rel: &str| format!("{HOME}/Code/{rel}/node_modules");
    let app = match name {
        "1-scanning" => {
            let mut app = App::default();
            app.on_scan(started());
            app.on_scan(ScanEvent::RootDone { root: HOME.into() });
            for e in [rust(), node(), xcode()] {
                app.on_scan(e);
            }
            app
        }
        "2-nothing-to-clean" => {
            let mut app = App::default();
            app.on_scan(started());
            app.on_scan(ScanEvent::Finished { elapsed_ms: 900 });
            app
        }
        "3-review" => selected(),
        "4-confirm" => {
            let mut app = selected();
            app.toggle_open(0);
            app.open_confirm();
            app
        }
        "5-deleting" => {
            let (mut app, paths) = deleting();
            removed(&mut app, &paths[0], 5.9);
            removed(&mut app, &paths[1], 3.6);
            removed(&mut app, &nm("web"), 2.9);
            app.on_delete(DeleteEvent::Removing { rule: rule_of(&app, &nm("old-api")), path: nm("old-api") });
            app
        }
        "6-done" => {
            let (mut app, paths) = deleting();
            for p in &paths {
                let size = app.plan.iter().find(|x| &x.path == p).unwrap().size as f64 / (1u64 << 30) as f64;
                removed(&mut app, p, size);
            }
            app.on_delete(DeleteEvent::Finished { freed: 0, free_after: gib(76.7) });
            app
        }
        "7-partial-failure" => {
            let (mut app, paths) = deleting();
            removed(&mut app, &paths[0], 5.9);
            removed(&mut app, &paths[1], 3.6);
            removed(&mut app, &nm("web"), 2.9);
            removed(&mut app, &nm("site"), 2.4);
            failed(&mut app, &nm("old-api"), FailReason::PermissionDenied);
            failed(&mut app, &nm("legacy/web"), FailReason::SymlinkRejected);
            app.on_delete(DeleteEvent::Finished { freed: 0, free_after: gib(72.2) });
            app
        }
        "8-settings" => {
            let mut app = scanned();
            app.open_settings();
            app
        }
        _ => return None,
    };
    debug_assert!(name != "8-settings" || app.screen == Screen::Settings);
    Some(app)
}

pub fn settings() -> Settings {
    Settings {
        disabled: Vec::new(),
        extra_roots: Vec::new(),
        keep: vec![
            format!("{HOME}/Code/riku/target").into(),
            format!("{HOME}/Code/knot/node_modules").into(),
            "/Volumes/T7/proj/client-app/target".into(),
        ],
    }
}
