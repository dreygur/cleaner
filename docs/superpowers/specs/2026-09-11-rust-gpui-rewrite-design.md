# cleaner: Rust + gpui rewrite

## Overview

Rewrite the disk reclaimer as a native macOS GUI app in Rust, using `gpui`
(Zed's UI framework) and `gpui-component` (the longbridge component library),
bundled through the single `gpui-kit` crate. The existing Go implementation
(`main.go`, `internal/`) stays in the repo untouched and unused; the new app
is a separate Cargo project that does not call into it.

Approved UI reference: the mockup at
`https://claude.ai/code/artifact/00b3b166-4218-4213-b228-829b28427328`
(Main list, Scanning, Settings, Confirm delete).

## Goals

- Port the 13 existing rules and every safety guard exactly: symlink
  rejection, `$HOME`-relative protected-name list, marker-file matching for
  artifact rules, contents-only deletion for fixed caches vs. whole-directory
  deletion for build artifacts.
- Scan list with checkboxes, per-rule size and regen hint, running total.
- Settings panel: editable search roots, per-rule enable switch (trash off
  by default).
- Confirm-delete modal before any deletion, with a standalone warning line
  for the trash rule.
- Native macOS window, light and dark theme, both following gpui-component's
  theme system.

## Non-goals (v1)

- No `.app` bundle / code signing / notarization. `cargo run` is the
  supported way to launch it for now.
- No CLI flags port (`--only`, `--skip`, `--roots` become settings-panel
  state instead of argv).
- No persistence format decided yet beyond "settings survive a restart"
  (see Open questions).

## Repo layout

```
cleaner/
├── main.go, internal/, go.mod       # untouched, unused
└── gui/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── error.rs
        ├── types.rs
        ├── format.rs
        ├── config/
        │   └── mod.rs
        ├── scan/
        │   └── mod.rs
        ├── clean/
        │   └── mod.rs
        └── ui/
            ├── mod.rs
            ├── state.rs
            ├── root.rs
            ├── main_view.rs
            ├── settings_view.rs
            └── confirm_dialog.rs
```

`gui/` is its own Cargo project so `cargo build` there never touches Go
tooling and `go build` at the repo root never touches Rust tooling.

## Dependencies

```toml
[dependencies]
gpui-kit = "0.6"
thiserror = "2"
rayon = "1"
dirs = "5"

[dev-dependencies]
tempfile = "3"
```

`gpui-kit` re-exports `gpui`, `gpui-component`, and `gpui-base` behind one
version-pinned crate on crates.io, instead of pinning two separate git
dependencies against a moving `main` branch. It satisfies "gpui and
gpui-component" without hand-tracking two upstream revs. If you'd rather
depend on `gpui` and `gpui-component` directly (git deps, more moving parts,
matches the ask literally) say so and the spec flips before the plan is
written.

`rayon` replaces the Go version's hand-rolled semaphore + goroutine pool for
`measure()`: `paths.par_iter().map(dir_size)` gets the same bounded
parallelism from a library instead of ten lines of channel bookkeeping.

## Domain layer

Ported 1:1 from `internal/`, same rule set, same guards, same test cases
translated from `scanner_test.go` and `cleaner_test.go`.

### `types.rs`

```rust
pub enum Kind { Artifact, Fixed }

pub struct Rule {
    pub id: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    pub dir: &'static str,       // Artifact only
    pub marker: &'static str,    // Artifact only, "" = no marker check
    pub roots: Vec<PathBuf>,     // Artifact only
    pub paths: Vec<PathBuf>,     // Fixed only
    pub regen: &'static str,
}

pub struct Target { pub path: PathBuf, pub size: u64 }
pub struct Group { pub rule: Rule, pub targets: Vec<Target>, pub size: u64 }
```

### `error.rs`

```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("{0}: path is outside the home directory")]
    OutsideHome(PathBuf),
    #[error("{0}: path is protected")]
    ProtectedPath(PathBuf),
    #[error("{0}: path is not a real directory")]
    NotDirectory(PathBuf),
    #[error("io on {path}: {source}")]
    Io { path: PathBuf, #[source] source: std::io::Error },
}
pub type Result<T> = std::result::Result<T, AppError>;
```

### `config/mod.rs`

- `DEFAULT_SEARCH_ROOTS: &[&str]` = `["Code", "Projects", "src", "work", "dev"]`
- `PROTECTED_NAMES: &[&str]` = the same nine names from `cleaner.go`
- `rules(home: &Path, search_roots: &[PathBuf]) -> Vec<Rule>` builds the same
  13 rules, same ids, same descriptions, same regen strings.

### `scan/mod.rs`

`dir_size`, `has_marker`, `collect_fixed`, `collect_artifact`, `collect`,
`measure` (rayon-parallel), `scan(rules: &[Rule]) -> Vec<Group>`, dropping
rules that matched nothing, same as the Go version.

### `clean/mod.rs`

`validate(home, path) -> Result<()>` carries over every check from
`cleaner.go` in order: absolute path required, not `$HOME` itself, must
resolve under `$HOME`, not one of `PROTECTED_NAMES`, must exist and be a
real directory (`symlink_metadata`, reject `is_symlink()`).
`remove_contents(path)` empties a directory without removing it.
`remove_target(home, kind, target)` picks contents-only vs. whole-directory
by `Kind`, same as Go. `clean_group(home, group) -> (u64, Vec<AppError>)`
mirrors `Group()`: one failure never stops the remaining targets.

### `format.rs`

`human_bytes(n: u64) -> String`, e.g. `8_400_000_000 -> "8.4 GB"`, feeding
every size shown in the UI. New, has no Go equivalent worth porting since
the CLI printed raw byte counts.

## UI layer

Three screens over one `AppState`, matching the mockup's four preview
states (Scanning is a transient flag on Main, not a fourth screen struct).

```rust
pub enum View { Main, Settings, ConfirmDelete }

pub struct AppState {
    pub view: View,
    pub scanning: bool,
    pub groups: Vec<Group>,
    pub selected: HashSet<&'static str>,   // rule ids
    pub roots: Vec<PathBuf>,
    pub rule_enabled: HashMap<&'static str, bool>,   // settings draft
}
```

- `main_view.rs`: title bar, reclaimable-total header with a Rescan button,
  the checkbox list grouped into "Build artifacts", "Package & tool caches",
  "Trash" sections, footer with selected count/size and "Clean Selected...".
  Rescan spawns the scan on `cx.background_executor()` and updates state
  through `cx.spawn` on completion so the window never blocks.
- `settings_view.rs`: root chips with remove buttons, add-root input,
  per-rule switches grouped the same three ways, trash switch defaults off.
- `confirm_dialog.rs`: rendered as a modal over Main, lists only the
  selected groups and their sizes, a standalone warning row when the trash
  rule is selected, Cancel / Delete Selected. Delete spawns `clean_group`
  per selected group on the background executor and returns to Main.

Selection state, root list, and rule-enabled map are the pieces a settings
Save currently promotes from a draft into `AppState`; Cancel discards the
draft. Whether that split needs its own type or just two `HashMap`s is an
implementation-time call, not a design one.

## Testing

- `scan` and `clean`: `#[test]` fns against `tempfile::tempdir()` fixtures,
  same scenarios as the Go tests (marker present/absent, symlink rejected,
  protected name rejected, fixed rule empties but keeps the directory,
  artifact rule removes the whole directory, one failing target doesn't
  stop the rest).
- `format::human_bytes`: table-driven test over boundary values (0, 999,
  1000, 1 GB, etc).
- UI views are not unit-tested; verified by running the app.

## Open questions

- Settings persistence: where does the roots/enabled-rules draft get saved
  between launches (a JSON file under `~/Library/Application Support`, or
  nothing persisted in v1 and it resets to defaults every launch)? Pick one
  before or during the plan.
