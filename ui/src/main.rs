mod demo;
mod engine;
mod state;
mod view;

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cleaner::settings::{CONFIG_PATH, Settings};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Timer, TimerMode};

use crate::engine::Event;
use crate::state::{App, Sort};

slint::include_modules!();

/// How often engine events are drained into the UI.
const POLL: Duration = Duration::from_millis(16);
/// How often to check whether the native window exists yet.
const TITLEBAR_RETRY: Duration = Duration::from_millis(10);
/// Base font size the design and its mockups are drawn at.
const DESIGN_BASE_PX: f32 = 14.0;
/// Pixel channels further apart than this count as different in `--shot`.
const DIFF_THRESHOLD: u8 = 24;

struct Ctx {
    app: App,
    child: Option<Child>,
    settings: Settings,
    config: PathBuf,
    tx: Sender<Event>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn home() -> String {
    std::env::home_dir().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default()
}

fn render(ctx: &Ctx, w: &AppWindow) {
    view::render(&ctx.app, w, now());
    if ctx.app.screen == state::Screen::Settings {
        view::render_settings(w, &home(), &ctx.app.disks, &ctx.settings);
    }
}

fn start_scan(ctx: &mut Ctx) {
    ctx.app.rescan();
    ctx.child = engine::scan(ctx.tx.clone()).ok();
}

fn start_delete(ctx: &mut Ctx, paths: Vec<String>) {
    if !paths.is_empty() {
        ctx.child = engine::delete(&paths, ctx.tx.clone()).ok();
    }
}

fn save(ctx: &Ctx) {
    if let Err(e) = ctx.settings.save(&ctx.config) {
        eprintln!("[-] {e}");
    }
}

/// The root a switch row stands for, in the order render_settings lists them.
fn root_at(ctx: &Ctx, i: usize) -> Option<PathBuf> {
    std::iter::once(PathBuf::from(home()))
        .chain(ctx.app.disks.iter().map(PathBuf::from))
        .chain(ctx.settings.extra_roots.iter().cloned())
        .nth(i)
}

fn choose_folder() -> Option<PathBuf> {
    let out = Command::new("osascript").args(["-e", "POSIX path of (choose folder)"]).output().ok()?;
    let path = String::from_utf8(out.stdout).ok()?.trim().trim_end_matches('/').to_string();
    (out.status.success() && !path.is_empty()).then(|| PathBuf::from(path))
}

/// Wires a callback that changes the state, then re-renders.
macro_rules! on {
    ($w:expr, $ctx:expr, $setter:ident, |$c:ident $(, $arg:ident)*| $body:block) => {{
        let ctx = Rc::clone(&$ctx);
        let weak = $w.as_weak();
        $w.$setter(move |$($arg),*| {
            let Some(w) = weak.upgrade() else { return };
            {
                let $c = &mut *ctx.borrow_mut();
                $body
            }
            render(&ctx.borrow(), &w);
        });
    }};
}

fn wire(w: &AppWindow, ctx: &Rc<RefCell<Ctx>>) {
    let weak = w.as_weak();
    w.on_close_window(|| {
        let _ = slint::quit_event_loop();
    });
    w.on_quit(|| {
        let _ = slint::quit_event_loop();
    });
    w.on_minimize(move || {
        if let Some(w) = weak.upgrade() {
            w.window().set_minimized(true);
        }
    });
    let weak = w.as_weak();
    w.on_zoom(move || {
        if let Some(w) = weak.upgrade() {
            let win = w.window();
            win.set_maximized(!win.is_maximized());
        }
    });
    let weak = w.as_weak();
    w.on_drag(move || {
        if let Some(w) = weak.upgrade() {
            w.window().with_winit_window(|win| {
                let _ = win.drag_window();
            });
        }
    });

    on!(w, ctx, on_open_settings, |c| { c.app.open_settings(); });
    on!(w, ctx, on_close_settings, |c| { c.app.close_settings(); });
    on!(w, ctx, on_cancel_scan, |c| {
        if let Some(mut child) = c.child.take() {
            let _ = child.kill();
        }
        c.app.cancel_scan();
    });
    on!(w, ctx, on_rescan, |c| { start_scan(c); });
    on!(w, ctx, on_toggle_group, |c, i| { c.app.toggle_group(i as usize); });
    on!(w, ctx, on_toggle_target, |c, i, j| { c.app.toggle_target(i as usize, j as usize); });
    on!(w, ctx, on_open_group, |c, i| { c.app.toggle_open(i as usize); });
    on!(w, ctx, on_sort, |c, untouched| {
        c.app.set_sort(if untouched { Sort::Untouched } else { Sort::Size });
    });
    on!(w, ctx, on_select_safe, |c| { c.app.select_safe(); });
    on!(w, ctx, on_clear, |c| { c.app.clear(); });
    on!(w, ctx, on_ask_delete, |c| { c.app.open_confirm(); });
    on!(w, ctx, on_back, |c| { c.app.back_to_review(); });
    on!(w, ctx, on_confirm, |c| {
        let paths = c.app.confirm();
        start_delete(c, paths);
    });
    on!(w, ctx, on_stop, |c| {
        c.app.stop();
        if let Some(child) = &c.child {
            engine::interrupt(child);
        }
    });
    on!(w, ctx, on_retry, |c| {
        let paths = c.app.retry_failed();
        start_delete(c, paths);
    });
    on!(w, ctx, on_show, |c, i| {
        if let Some(p) = c.app.failed().nth(i as usize) {
            let _ = Command::new("open").args(["-R", &p.path]).status();
        }
    });
    on!(w, ctx, on_toggle_root, |c, i| {
        if let Some(root) = root_at(c, i as usize) {
            match c.settings.disabled.iter().position(|d| *d == root) {
                Some(at) => {
                    c.settings.disabled.remove(at);
                }
                None => c.settings.disabled.push(root),
            }
            save(c);
        }
    });
    on!(w, ctx, on_add_folder, |c| {
        if let Some(dir) = choose_folder()
            && !c.settings.extra_roots.contains(&dir)
        {
            c.settings.extra_roots.push(dir);
            save(c);
        }
    });
    on!(w, ctx, on_remove_keep, |c, i| {
        if (i as usize) < c.settings.keep.len() {
            c.settings.keep.remove(i as usize);
            save(c);
        }
    });
}

fn drain(ctx: &mut Ctx, rx: &Receiver<Event>) -> bool {
    let mut changed = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            Event::Scan(e) => ctx.app.on_scan(e),
            Event::Delete(e) => ctx.app.on_delete(e),
        }
        changed = true;
    }
    changed
}

/// A normal macOS window whose title bar is transparent and sits over the
/// content, so the system traffic lights draw on top of the window's own
/// title bar.
fn select_backend() -> Result<(), slint::PlatformError> {
    use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
    slint::BackendSelector::new()
        .with_winit_window_attributes_hook(|attrs| {
            attrs.with_titlebar_transparent(true).with_fullsize_content_view(true).with_title_hidden(true)
        })
        .select()
}

/// An empty unified toolbar makes AppKit draw a 52pt title bar with the
/// traffic lights centered in it, the height of the design's title bar.
/// False until the native window exists.
fn unify_titlebar(w: &AppWindow) -> bool {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSToolbar, NSView, NSWindowToolbarStyle};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Some(mtm) = MainThreadMarker::new() else { return false };
    let handle = w.window().window_handle();
    let Ok(handle) = handle.window_handle() else { return false };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else { return false };
    // SAFETY: AppKit handles carry a live NSView for as long as the window
    // exists, and this runs on the main thread.
    let view: &NSView = unsafe { appkit.ns_view.cast().as_ref() };
    let Some(window) = view.window() else { return false };
    let toolbar = NSToolbar::new(mtm);
    window.setToolbar(Some(&toolbar));
    window.setToolbarStyle(NSWindowToolbarStyle::Unified);
    true
}

fn run() -> Result<(), slint::PlatformError> {
    select_backend()?;
    let w = AppWindow::new()?;
    let (tx, rx) = mpsc::channel();
    let config = PathBuf::from(home()).join(CONFIG_PATH);
    let settings = Settings::load(&config).unwrap_or_else(|e| {
        eprintln!("[-] {e}");
        Settings::default()
    });
    let ctx = Rc::new(RefCell::new(Ctx { app: App::default(), child: None, settings, config, tx }));
    wire(&w, &ctx);
    start_scan(&mut ctx.borrow_mut());
    render(&ctx.borrow(), &w);

    let timer = Timer::default();
    let weak = w.as_weak();
    let poll_ctx = Rc::clone(&ctx);
    timer.start(TimerMode::Repeated, POLL, move || {
        let Some(w) = weak.upgrade() else { return };
        if drain(&mut poll_ctx.borrow_mut(), &rx) {
            render(&poll_ctx.borrow(), &w);
        }
    });
    // Slint creates the native window once the event loop runs, so the
    // title bar is set up from the loop, as soon as the window is there.
    let titlebar = Rc::new(Timer::default());
    let weak = w.as_weak();
    let stopper = Rc::downgrade(&titlebar);
    titlebar.start(TimerMode::Repeated, TITLEBAR_RETRY, move || {
        if weak.upgrade().is_none_or(|w| unify_titlebar(&w))
            && let Some(t) = stopper.upgrade()
        {
            t.stop();
        }
    });
    w.show()?;
    slint::run_event_loop()?;
    w.hide()?;

    // A scan has no reason to outlive the window. A deletion keeps going, as
    // the Deleting screen promises.
    let mut c = ctx.borrow_mut();
    if c.app.screen == state::Screen::Scanning
        && let Some(child) = c.child.as_mut()
    {
        let _ = child.kill();
    }
    Ok(())
}

/// Scales the whole window: every design length is a multiple of this size.
fn set_base_size(w: &AppWindow, px: f32) {
    w.set_base_size(px);
    w.global::<M>().set_unit(px);
}

/// Share of pixels that differ, and an image marking them in red.
fn diff(a: &image::RgbaImage, b: &image::RgbaImage) -> (f64, image::RgbaImage) {
    let (w, h) = (a.width().min(b.width()), a.height().min(b.height()));
    let mut out = image::RgbaImage::new(w, h);
    let mut bad = 0u64;
    for (x, y, px) in out.enumerate_pixels_mut() {
        let (p, q) = (a.get_pixel(x, y), b.get_pixel(x, y));
        let far = (0..3).any(|c| p[c].abs_diff(q[c]) > DIFF_THRESHOLD);
        bad += far as u64;
        let g = p[0] / 3 + p[1] / 3 + p[2] / 3;
        *px = if far { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([g, g, g, 255]) };
    }
    (bad as f64 / (w as u64 * h as u64) as f64, out)
}

/// Renders one mockup screen with its sample data and writes a PNG, plus a
/// diff against the reference when one is given:
/// `cleaner-ui --shot 3-review out.png [screens/3-review.png]`.
fn shot(name: &str, out: &str, reference: Option<&str>) -> Result<(), slint::PlatformError> {
    let app = demo::app_for(name).ok_or_else(|| slint::PlatformError::Other(format!("unknown screen {name}")))?;
    let w = AppWindow::new()?;
    w.global::<T>().set_frozen(true);
    w.set_mockup(true);
    // The mockups are drawn at the design's own 14px scale.
    set_base_size(&w, DESIGN_BASE_PX);
    view::render(&app, &w, demo::NOW);
    if app.screen == state::Screen::Settings {
        view::render_settings(&w, demo::HOME, &[demo::DISK.to_string()], &demo::settings());
    }
    w.show()?;

    let weak = w.as_weak();
    let (out, reference) = (out.to_string(), reference.map(str::to_string));
    Timer::single_shot(Duration::from_millis(400), move || {
        let Some(w) = weak.upgrade() else { return };
        match w.window().take_snapshot() {
            Ok(buf) => {
                let img = image::RgbaImage::from_raw(buf.width(), buf.height(), buf.as_bytes().to_vec()).unwrap();
                img.save(&out).unwrap();
                if let Some(r) = reference.and_then(|r| image::open(r).ok()) {
                    let (share, marked) = diff(&img, &r.to_rgba8());
                    marked.save(out.replace(".png", "-diff.png")).unwrap();
                    println!("{:.2}% of pixels differ", share * 100.0);
                }
            }
            Err(e) => eprintln!("[-] snapshot: {e}"),
        }
        let _ = slint::quit_event_loop();
    });
    slint::run_event_loop()
}

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().collect();
    match args.iter().position(|a| a == "--shot") {
        Some(i) if args.len() > i + 2 => shot(&args[i + 1], &args[i + 2], args.get(i + 3).map(String::as_str)),
        _ => run(),
    }
}
