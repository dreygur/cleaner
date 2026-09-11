# cleaner Rust/gpui rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a native macOS GUI app in `gui/` (Rust, gpui, gpui-component) that ports the existing Go disk-cleaner's rule set and safety guards exactly, with a scan list, settings panel, and delete-confirmation flow.

**Architecture:** A domain layer (`types`, `error`, `config`, `scan`, `clean`, `format`) ported 1:1 from `internal/`, fully unit-tested and gpui-free. A UI layer (`ui/`) of three gpui `Render` views (Main, Settings, Confirm) driven by one `AppState`, with scanning and deletion run on gpui's background executor so the window never blocks.

**Tech Stack:** Rust 2021, `gpui` (git, zed-industries/zed), `gpui-component` (git, longbridge/gpui-component), `thiserror`, `rayon`, `dirs`, `tempfile` (dev).

**Spec:** `docs/superpowers/specs/2026-09-11-rust-gpui-rewrite-design.md`

## Global Constraints

- Every path-deletion guard in `internal/clean/cleaner.go` carries over exactly: absolute-path requirement, `$HOME`-equality rejection, outside-`$HOME` rejection, the nine-name protected list, symlink rejection, contents-only delete for `Kind::Fixed` vs. whole-directory delete for `Kind::Artifact`.
- All 13 rule ids, descriptions, and regen strings match `internal/config/rules.go` (regen text may lose the source's em dash; write it as a comma instead, per style rule below).
- No `.app` bundle, code signing, or CLI-flag port in this plan. Settings are not persisted between launches (reset to defaults every run).
- `gpui` and `gpui-component` are pinned to a specific commit (`rev = "..."`), never `branch = "main"`.
- No raw `println!`/`eprintln!` outside `main.rs`; domain-layer errors return `Result<_, AppError>`, never a bare string.
- No em dash anywhere (code, comments, commit messages). No section-banner comments (`// ---- Helpers ----`). A comment earns its place only by stating a *why* the code doesn't already show.
- Helpers are defined above their caller in every file (leaf functions first, composite last), constants at the top.
- Every new function with real logic gets a `#[cfg(test)] mod tests` in the same file that executes it against real behavior, not just a type check.
- Test fixtures use `tempfile::tempdir()`, never a hardcoded `/tmp/...` path.
- Every commit message ends with:
  ```
  Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
  ```

All `cargo` commands below run from the `gui/` directory (`cd gui && cargo ...`).

---

### Task 1: Scaffold the `gui/` Cargo project

**Files:**
- Create: `gui/Cargo.toml`
- Create: `gui/src/main.rs`
- Create: `gui/.gitignore`

**Interfaces:**
- Produces: a binary crate named `cleaner` that builds and runs (does nothing yet). Every later task adds a `mod` line to `main.rs`.

- [ ] **Step 1: Create the crate manifest**

`gui/Cargo.toml`:
```toml
[package]
name = "cleaner"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "cleaner"
path = "src/main.rs"

[dependencies]
thiserror = "2"
rayon = "1"
dirs = "5"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create the entry point**

`gui/src/main.rs`:
```rust
fn main() {}
```

- [ ] **Step 3: Ignore the build directory**

`gui/.gitignore`:
```
/target
```

- [ ] **Step 4: Build**

Run: `cd gui && cargo build`
Expected: compiles with no errors (unused-dependency warnings for `thiserror`/`rayon`/`dirs` are fine, they're consumed starting Task 3).

- [ ] **Step 5: Commit**

```bash
git add gui/Cargo.toml gui/Cargo.lock gui/src/main.rs gui/.gitignore
git commit -m "$(cat <<'EOF'
chore: scaffold gui/ cargo project

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 2: `format::human_bytes`

**Files:**
- Create: `gui/src/format.rs`
- Modify: `gui/src/main.rs` (add `mod format;`)

**Interfaces:**
- Produces: `pub fn human_bytes(n: u64) -> String`, used by every size shown in the UI (`main_view`, `confirm_dialog`).

- [ ] **Step 1: Write the failing test**

`gui/src/format.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_under_1000_have_no_unit_suffix_conversion() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
    }

    #[test]
    fn crosses_into_kilobytes_at_1000() {
        assert_eq!(human_bytes(1_000), "1.0 KB");
        assert_eq!(human_bytes(1_500), "1.5 KB");
    }

    #[test]
    fn crosses_into_megabytes_gigabytes_terabytes() {
        assert_eq!(human_bytes(8_400_000_000), "8.4 GB");
        assert_eq!(human_bytes(420_000_000), "420.0 MB");
        assert_eq!(human_bytes(1_000_000_000_000), "1.0 TB");
    }
}
```

- [ ] **Step 2: Run the test, confirm it fails to compile**

Run: `cd gui && cargo test format::`
Expected: FAIL, `cannot find function 'human_bytes' in this scope`

- [ ] **Step 3: Implement**

Add above the test module in `gui/src/format.rs`:
```rust
const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

pub fn human_bytes(n: u64) -> String {
    if n < 1000 {
        return format!("{n} B");
    }
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}
```

- [ ] **Step 4: Add the module and run the test**

In `gui/src/main.rs`, add above `fn main() {}`:
```rust
mod format;
```

Run: `cd gui && cargo test format::`
Expected: PASS, 3 tests

- [ ] **Step 5: Commit**

```bash
git add gui/src/format.rs gui/src/main.rs
git commit -m "$(cat <<'EOF'
feat: add human_bytes size formatter

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 3: `error::AppError`

**Files:**
- Create: `gui/src/error.rs`
- Modify: `gui/src/main.rs` (add `mod error;`)

**Interfaces:**
- Produces: `pub enum AppError { OutsideHome(String), ProtectedPath(String), NotDirectory(String), Io { path: String, source: std::io::Error } }` and `pub type Result<T> = std::result::Result<T, AppError>`. Consumed by `clean` (Task 8, 9).

- [ ] **Step 1: Write the failing test**

`gui/src/error.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_home_message_names_the_path() {
        let err = AppError::OutsideHome("/etc/passwd".to_string());
        assert_eq!(err.to_string(), "/etc/passwd: path is outside the home directory");
    }

    #[test]
    fn protected_path_message_names_the_path() {
        let err = AppError::ProtectedPath("/Users/test/Documents".to_string());
        assert_eq!(err.to_string(), "/Users/test/Documents: path is protected");
    }

    #[test]
    fn not_directory_message_names_the_path() {
        let err = AppError::NotDirectory("/Users/test/link".to_string());
        assert_eq!(err.to_string(), "/Users/test/link: path is not a real directory");
    }

    #[test]
    fn io_message_includes_path_and_source() {
        let source = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = AppError::Io { path: "/Users/test/x".to_string(), source };
        assert_eq!(err.to_string(), "io on /Users/test/x: denied");
    }
}
```

- [ ] **Step 2: Run the test, confirm it fails to compile**

Run: `cd gui && cargo test error::`
Expected: FAIL, `cannot find type 'AppError' in this scope`

- [ ] **Step 3: Implement**

Add above the test module in `gui/src/error.rs`:
```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("{0}: path is outside the home directory")]
    OutsideHome(String),
    #[error("{0}: path is protected")]
    ProtectedPath(String),
    #[error("{0}: path is not a real directory")]
    NotDirectory(String),
    #[error("io on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, AppError>;
```

`PathBuf` doesn't implement `Display`, so every variant carries a `String`
built with `.display().to_string()` at the call site rather than a
`PathBuf` directly.

- [ ] **Step 4: Add the module and run the test**

In `gui/src/main.rs`, add below `mod format;`:
```rust
mod error;
```

Run: `cd gui && cargo test error::`
Expected: PASS, 4 tests

- [ ] **Step 5: Commit**

```bash
git add gui/src/error.rs gui/src/main.rs
git commit -m "$(cat <<'EOF'
feat: add AppError

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 4: `types::{Kind, Rule, Target, Group}`

**Files:**
- Create: `gui/src/types.rs`
- Modify: `gui/src/main.rs` (add `mod types;`)

**Interfaces:**
- Produces:
  ```rust
  pub enum Kind { Artifact, Fixed }              // Copy, Eq
  pub struct Rule {
      pub id: &'static str,
      pub description: &'static str,
      pub kind: Kind,
      pub dir: &'static str,
      pub marker: &'static str,
      pub roots: Vec<PathBuf>,
      pub paths: Vec<PathBuf>,
      pub regen: &'static str,
  }                                                // Clone
  pub struct Target { pub path: PathBuf, pub size: u64 }   // Clone
  pub struct Group { pub rule: Rule, pub targets: Vec<Target>, pub size: u64 } // Clone
  ```
  Consumed by `config` (Task 5), `scan` (Task 6, 7), `clean` (Task 8, 9), `ui::state` (Task 10).

Pure data, no behavior beyond field access, so there's no separate red
step: the test and the definitions land together and get verified by one
`cargo test` pass.

- [ ] **Step 1: Write the module with its construction test**

`gui/src/types.rs`:
```rust
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Artifact,
    Fixed,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub id: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    pub dir: &'static str,
    pub marker: &'static str,
    pub roots: Vec<PathBuf>,
    pub paths: Vec<PathBuf>,
    pub regen: &'static str,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub rule: Rule,
    pub targets: Vec<Target>,
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_carries_its_rule_and_summed_targets() {
        let rule = Rule {
            id: "test-rule",
            description: "test",
            kind: Kind::Fixed,
            dir: "",
            marker: "",
            roots: vec![],
            paths: vec![PathBuf::from("/tmp/x")],
            regen: "n/a",
        };
        let group = Group {
            rule,
            targets: vec![Target { path: PathBuf::from("/tmp/x"), size: 42 }],
            size: 42,
        };
        assert_eq!(group.size, 42);
        assert_eq!(group.targets[0].size, 42);
        assert_eq!(group.rule.id, "test-rule");
        assert_eq!(group.rule.kind, Kind::Fixed);
    }
}
```

- [ ] **Step 2: Add the module and run the test**

In `gui/src/main.rs`, add below `mod error;`:
```rust
mod types;
```

Run: `cd gui && cargo test types::`
Expected: PASS, 1 test

- [ ] **Step 3: Commit**

```bash
git add gui/src/types.rs gui/src/main.rs
git commit -m "$(cat <<'EOF'
feat: add domain types Rule, Target, Group

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 5: `config::rules`

**Files:**
- Create: `gui/src/config/mod.rs`
- Modify: `gui/src/main.rs` (add `mod config;`)

**Interfaces:**
- Consumes: `crate::types::{Kind, Rule}` (Task 4).
- Produces: `pub const DEFAULT_SEARCH_ROOTS: &[&str]`, `pub const PROTECTED_NAMES: &[&str]`, `pub fn rules(home: &Path, search_roots: &[PathBuf]) -> Vec<Rule>`. Consumed by `clean::validate` (Task 8, via `PROTECTED_NAMES`) and by `ui` (Task 10+, via `rules`/`DEFAULT_SEARCH_ROOTS`).

- [ ] **Step 1: Write the failing test**

`gui/src/config/mod.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rules_returns_all_thirteen_in_order() {
        let home = PathBuf::from("/Users/test");
        let roots = vec![home.join("Code")];
        let rs = rules(&home, &roots);

        let ids: Vec<&str> = rs.iter().map(|r| r.id).collect();
        assert_eq!(
            ids,
            vec![
                "rust-target", "node-modules", "next-build", "pycache",
                "go-build", "pnpm-store", "brew-cache", "pip-cache",
                "js-tool-cache", "chrome-cache", "cargo-registry",
                "xcode-derived", "trash",
            ]
        );
    }

    #[test]
    fn artifact_rule_carries_the_given_search_roots_and_marker() {
        let home = PathBuf::from("/Users/test");
        let roots = vec![home.join("Code"), home.join("Projects")];
        let rs = rules(&home, &roots);
        let rust_target = &rs[0];
        assert_eq!(rust_target.roots, roots);
        assert_eq!(rust_target.dir, "target");
        assert_eq!(rust_target.marker, "Cargo.toml");
    }

    #[test]
    fn fixed_rule_paths_are_absolute_under_home() {
        let home = PathBuf::from("/Users/test");
        let rs = rules(&home, &[]);

        let trash = rs.iter().find(|r| r.id == "trash").unwrap();
        assert_eq!(trash.paths, vec![home.join(".Trash")]);

        let cargo_registry = rs.iter().find(|r| r.id == "cargo-registry").unwrap();
        assert_eq!(
            cargo_registry.paths,
            vec![
                home.join(".cargo").join("registry").join("cache"),
                home.join(".cargo").join("registry").join("src"),
                home.join(".cargo").join("git").join("checkouts"),
            ]
        );
    }
}
```

- [ ] **Step 2: Run the test, confirm it fails to compile**

Run: `cd gui && cargo test config::`
Expected: FAIL, `cannot find function 'rules' in this scope`

- [ ] **Step 3: Implement**

Add above the test module in `gui/src/config/mod.rs`:
```rust
use std::path::{Path, PathBuf};

use crate::types::{Kind, Rule};

pub const DEFAULT_SEARCH_ROOTS: &[&str] = &["Code", "Projects", "src", "work", "dev"];

pub const PROTECTED_NAMES: &[&str] = &[
    "Documents", "Desktop", "Downloads", "Pictures",
    "Movies", "Music", "Library", "Applications", "Public",
];

fn artifact(
    id: &'static str,
    description: &'static str,
    dir: &'static str,
    marker: &'static str,
    regen: &'static str,
    roots: &[PathBuf],
) -> Rule {
    Rule {
        id,
        description,
        kind: Kind::Artifact,
        dir,
        marker,
        roots: roots.to_vec(),
        paths: Vec::new(),
        regen,
    }
}

fn fixed(id: &'static str, description: &'static str, regen: &'static str, paths: Vec<PathBuf>) -> Rule {
    Rule {
        id,
        description,
        kind: Kind::Fixed,
        dir: "",
        marker: "",
        roots: Vec::new(),
        paths,
        regen,
    }
}

pub fn rules(home: &Path, search_roots: &[PathBuf]) -> Vec<Rule> {
    let lib = home.join("Library");
    let caches = lib.join("Caches");

    vec![
        artifact("rust-target", "Cargo build output", "target", "Cargo.toml",
            "cargo build", search_roots),
        artifact("node-modules", "Installed npm dependencies", "node_modules", "package.json",
            "npm install / pnpm install", search_roots),
        artifact("next-build", "Next.js build output", ".next", "package.json",
            "next build", search_roots),
        artifact("pycache", "Compiled Python bytecode", "__pycache__", "",
            "regenerated on next import", search_roots),

        fixed("go-build", "Go build cache", "go build",
            vec![caches.join("go-build")]),
        fixed("pnpm-store", "pnpm package store", "pnpm install",
            vec![caches.join("pnpm"), lib.join("pnpm").join("store")]),
        fixed("brew-cache", "Downloaded Homebrew bottles", "brew reinstalls on demand",
            vec![caches.join("Homebrew")]),
        fixed("pip-cache", "pip wheel cache", "pip download on next install",
            vec![caches.join("pip")]),
        fixed("js-tool-cache", "node-gyp and TypeScript caches", "re-downloaded on demand",
            vec![caches.join("node-gyp"), caches.join("typescript")]),
        fixed("chrome-cache", "Chrome on-disk HTTP cache (profile untouched)", "refills as you browse",
            vec![caches.join("Google")]),
        fixed("cargo-registry", "Downloaded crate sources", "cargo fetch (needs network)",
            vec![
                home.join(".cargo").join("registry").join("cache"),
                home.join(".cargo").join("registry").join("src"),
                home.join(".cargo").join("git").join("checkouts"),
            ]),
        fixed("xcode-derived", "Xcode DerivedData", "rebuilt by Xcode",
            vec![lib.join("Developer").join("Xcode").join("DerivedData")]),
        fixed("trash", "Trash", "nothing, this is a permanent delete",
            vec![home.join(".Trash")]),
    ]
}
```

- [ ] **Step 4: Add the module and run the test**

In `gui/src/main.rs`, add below `mod types;`:
```rust
mod config;
```

Run: `cd gui && cargo test config::`
Expected: PASS, 3 tests

- [ ] **Step 5: Commit**

```bash
git add gui/src/config gui/src/main.rs
git commit -m "$(cat <<'EOF'
feat: port the 13-rule table from internal/config/rules.go

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 6: `scan` collect helpers

**Files:**
- Create: `gui/src/scan/mod.rs`
- Modify: `gui/src/main.rs` (add `mod scan;`)

**Interfaces:**
- Consumes: `crate::types::{Kind, Rule}` (Task 4).
- Produces: `pub fn dir_size(root: &Path) -> u64`, `pub fn has_marker(dir: &Path, marker: &str) -> bool`, `pub fn collect(rule: &Rule) -> Vec<PathBuf>`. Consumed within this file by `scan()` (Task 7).

- [ ] **Step 1: Write the failing tests**

`gui/src/scan/mod.rs`:
```rust
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
```

- [ ] **Step 2: Run the tests, confirm they fail to compile**

Run: `cd gui && cargo test scan::`
Expected: FAIL, `cannot find function 'dir_size'` (and siblings) `in this scope`

- [ ] **Step 3: Implement**

Add above the test module in `gui/src/scan/mod.rs`:
```rust
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
```

`walk_for_match` stops descending once a directory named `rule.dir` is
found, matching the Go version's `filepath.SkipDir`: a `target/` full of
its own build output never gets walked looking for a nested match.

- [ ] **Step 4: Add the module and run the tests**

In `gui/src/main.rs`, add below `mod config;`:
```rust
mod scan;
```

Run: `cd gui && cargo test scan::`
Expected: PASS, 6 tests

- [ ] **Step 5: Commit**

```bash
git add gui/src/scan gui/src/main.rs
git commit -m "$(cat <<'EOF'
feat: port collect/dir_size/has_marker from internal/scan

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 7: `scan::{measure, scan}`

**Files:**
- Modify: `gui/src/scan/mod.rs`
- Modify: `gui/Cargo.toml` (activate `rayon`, already listed since Task 1)

**Interfaces:**
- Consumes: `dir_size`, `collect` (this file, Task 6); `crate::types::{Group, Rule, Target}` (Task 4).
- Produces: `pub fn measure(paths: Vec<PathBuf>) -> Vec<Target>`, `pub fn scan(rules: &[Rule]) -> Vec<Group>`. Consumed by `ui` (Task 12).

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `gui/src/scan/mod.rs` (inside the existing `mod tests { ... }` block, after the last test):
```rust
    #[test]
    fn measure_preserves_input_order_and_reports_real_sizes() {
        let root = tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("f"), vec![0u8; 10]).unwrap();
        fs::write(b.join("f"), vec![0u8; 20]).unwrap();

        let targets = measure(vec![a.clone(), b.clone()]);
        assert_eq!(targets[0].path, a);
        assert_eq!(targets[0].size, 10);
        assert_eq!(targets[1].path, b);
        assert_eq!(targets[1].size, 20);
    }

    #[test]
    fn scan_drops_rules_that_matched_nothing() {
        let root = tempdir().unwrap();
        let matching_dir = root.path().join("present");
        fs::create_dir(&matching_dir).unwrap();
        fs::write(matching_dir.join("f"), vec![0u8; 5]).unwrap();

        let matching = crate::types::Rule {
            id: "matches", description: "t", kind: crate::types::Kind::Fixed,
            dir: "", marker: "", roots: vec![],
            paths: vec![matching_dir], regen: "t",
        };
        let empty = crate::types::Rule {
            id: "empty", description: "t", kind: crate::types::Kind::Fixed,
            dir: "", marker: "", roots: vec![],
            paths: vec![root.path().join("does-not-exist")], regen: "t",
        };

        let groups = scan(&[matching, empty]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].rule.id, "matches");
        assert_eq!(groups[0].size, 5);
    }
```

- [ ] **Step 2: Run the tests, confirm they fail to compile**

Run: `cd gui && cargo test scan::`
Expected: FAIL, `cannot find function 'measure'` and `'scan'` `in this scope`

- [ ] **Step 3: Implement**

Add to `gui/src/scan/mod.rs`, above the test module, after `collect`:
```rust
use rayon::prelude::*;

use crate::types::{Group, Target};

pub fn measure(paths: Vec<PathBuf>) -> Vec<Target> {
    paths
        .into_par_iter()
        .map(|path| {
            let size = dir_size(&path);
            Target { path, size }
        })
        .collect()
}

pub fn scan(rules: &[Rule]) -> Vec<Group> {
    rules
        .iter()
        .filter_map(|rule| {
            let paths = collect(rule);
            if paths.is_empty() {
                return None;
            }
            let targets = measure(paths);
            let size = targets.iter().map(|t| t.size).sum();
            Some(Group { rule: rule.clone(), targets, size })
        })
        .collect()
}
```

`into_par_iter().map(...).collect()` on a `Vec` is index-preserving, so
`measure` returns targets in the same order the paths came in, same
contract as the Go version's indexed goroutine writes.

- [ ] **Step 4: Run the tests**

Run: `cd gui && cargo test scan::`
Expected: PASS, 8 tests

- [ ] **Step 5: Commit**

```bash
git add gui/src/scan/mod.rs
git commit -m "$(cat <<'EOF'
feat: add scan::measure and scan::scan

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 8: `clean::validate`

**Files:**
- Create: `gui/src/clean/mod.rs`
- Modify: `gui/src/main.rs` (add `mod clean;`)

**Interfaces:**
- Consumes: `crate::config::PROTECTED_NAMES` (Task 5), `crate::error::{AppError, Result}` (Task 3).
- Produces: `pub fn validate(home: &Path, path: &Path) -> Result<()>`. Consumed within this file by `remove_target` (Task 9).

- [ ] **Step 1: Write the failing tests**

`gui/src/clean/mod.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn relative_path_is_rejected() {
        let home = tempdir().unwrap();
        let err = validate(home.path(), std::path::Path::new("relative/x")).unwrap_err();
        assert!(matches!(err, crate::error::AppError::OutsideHome(_)));
    }

    #[test]
    fn home_itself_is_rejected() {
        let home = tempdir().unwrap();
        let err = validate(home.path(), home.path()).unwrap_err();
        assert!(matches!(err, crate::error::AppError::ProtectedPath(_)));
    }

    #[test]
    fn path_outside_home_is_rejected() {
        let home = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let err = validate(home.path(), outside.path()).unwrap_err();
        assert!(matches!(err, crate::error::AppError::OutsideHome(_)));
    }

    #[test]
    fn protected_top_level_name_is_rejected() {
        let home = tempdir().unwrap();
        let docs = home.path().join("Documents");
        fs::create_dir(&docs).unwrap();
        let err = validate(home.path(), &docs).unwrap_err();
        assert!(matches!(err, crate::error::AppError::ProtectedPath(_)));
    }

    #[test]
    fn symlink_is_rejected() {
        let home = tempdir().unwrap();
        let real = home.path().join("real");
        fs::create_dir(&real).unwrap();
        let link = home.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let err = validate(home.path(), &link).unwrap_err();
        assert!(matches!(err, crate::error::AppError::NotDirectory(_)));
    }

    #[test]
    fn nonexistent_path_is_an_io_error() {
        let home = tempdir().unwrap();
        let err = validate(home.path(), &home.path().join("Code/proj/target")).unwrap_err();
        assert!(matches!(err, crate::error::AppError::Io { .. }));
    }

    #[test]
    fn real_directory_under_home_is_accepted() {
        let home = tempdir().unwrap();
        let target = home.path().join("Code").join("proj").join("target");
        fs::create_dir_all(&target).unwrap();
        assert!(validate(home.path(), &target).is_ok());
    }
}
```

- [ ] **Step 2: Run the tests, confirm they fail to compile**

Run: `cd gui && cargo test clean::`
Expected: FAIL, `cannot find function 'validate' in this scope`

- [ ] **Step 3: Implement**

Add above the test module in `gui/src/clean/mod.rs`:
```rust
use std::path::Path;

use crate::config::PROTECTED_NAMES;
use crate::error::{AppError, Result};

pub fn validate(home: &Path, path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(AppError::OutsideHome(path.display().to_string()));
    }
    if path == home {
        return Err(AppError::ProtectedPath(path.display().to_string()));
    }

    let rel = path
        .strip_prefix(home)
        .map_err(|_| AppError::OutsideHome(path.display().to_string()))?;
    if rel.as_os_str().is_empty() {
        return Err(AppError::OutsideHome(path.display().to_string()));
    }
    if let Some(first) = rel.components().next() {
        let name = first.as_os_str().to_string_lossy();
        if PROTECTED_NAMES.contains(&name.as_ref()) {
            return Err(AppError::ProtectedPath(path.display().to_string()));
        }
    }

    let metadata = std::fs::symlink_metadata(path).map_err(|source| AppError::Io {
        path: path.display().to_string(),
        source,
    })?;
    if !metadata.is_dir() || metadata.is_symlink() {
        return Err(AppError::NotDirectory(path.display().to_string()));
    }
    Ok(())
}
```

This inherits the Go version's one limitation along with its guards: paths
are compared lexically, not canonicalized, so a `..` component could in
principle defeat `strip_prefix`. Every path this function actually sees
comes from `scan::collect` or `config::rules`, both of which build paths
with `Path::join` and never emit `..`, so the limitation is inherited, not
introduced.

- [ ] **Step 4: Add the module and run the tests**

In `gui/src/main.rs`, add below `mod scan;`:
```rust
mod clean;
```

Run: `cd gui && cargo test clean::`
Expected: PASS, 7 tests

- [ ] **Step 5: Commit**

```bash
git add gui/src/clean gui/src/main.rs
git commit -m "$(cat <<'EOF'
feat: port path safety guards from internal/clean

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 9: `clean::clean_group`

**Files:**
- Modify: `gui/src/clean/mod.rs`

**Interfaces:**
- Consumes: `validate` (this file, Task 8); `crate::types::{Group, Kind, Target}` (Task 4).
- Produces: `pub fn clean_group(home: &Path, group: &Group) -> (u64, Vec<AppError>)`. Consumed by `ui` (Task 14).

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `gui/src/clean/mod.rs` (after the last existing test):
```rust
    #[test]
    fn remove_contents_empties_but_keeps_the_directory() {
        let home = tempdir().unwrap();
        let cache = home.path().join("Library/Caches/go-build");
        fs::create_dir_all(cache.join("nested")).unwrap();
        fs::write(cache.join("file"), "x").unwrap();
        fs::write(cache.join("nested/file"), "y").unwrap();

        remove_contents(&cache).unwrap();

        assert!(cache.exists());
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn remove_target_fixed_empties_contents_only() {
        let home = tempdir().unwrap();
        let cache = home.path().join("Library/Caches/go-build");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("file"), "x").unwrap();
        let target = crate::types::Target { path: cache.clone(), size: 1 };

        remove_target(home.path(), crate::types::Kind::Fixed, &target).unwrap();

        assert!(cache.exists());
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn remove_target_artifact_removes_the_whole_directory() {
        let home = tempdir().unwrap();
        let build = home.path().join("Code/proj/target");
        fs::create_dir_all(&build).unwrap();
        let target = crate::types::Target { path: build.clone(), size: 1 };

        remove_target(home.path(), crate::types::Kind::Artifact, &target).unwrap();

        assert!(!build.exists());
        assert!(build.parent().unwrap().exists());
    }

    #[test]
    fn clean_group_keeps_going_after_one_target_fails() {
        let home = tempdir().unwrap();
        let good = home.path().join("Code/proj/target");
        fs::create_dir_all(&good).unwrap();

        let group = crate::types::Group {
            rule: crate::types::Rule {
                id: "rust-target", description: "t", kind: crate::types::Kind::Artifact,
                dir: "target", marker: "Cargo.toml", roots: vec![], paths: vec![],
                regen: "t",
            },
            targets: vec![
                crate::types::Target { path: std::path::PathBuf::from("/etc/hosts"), size: 100 },
                crate::types::Target { path: good.clone(), size: 50 },
            ],
            size: 150,
        };

        let (freed, failures) = clean_group(home.path(), &group);

        assert_eq!(freed, 50);
        assert_eq!(failures.len(), 1);
        assert!(!good.exists());
    }
```

- [ ] **Step 2: Run the tests, confirm they fail to compile**

Run: `cd gui && cargo test clean::`
Expected: FAIL, `cannot find function 'remove_contents'` (and siblings) `in this scope`

- [ ] **Step 3: Implement**

Add to `gui/src/clean/mod.rs`, above the test module, after `validate`:
```rust
use std::fs;

use crate::types::{Group, Kind, Target};

fn remove_any(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|source| AppError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let result = if metadata.is_dir() && !metadata.is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    result.map_err(|source| AppError::Io { path: path.display().to_string(), source })
}

fn remove_contents(path: &Path) -> Result<()> {
    let entries = fs::read_dir(path).map_err(|source| AppError::Io {
        path: path.display().to_string(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| AppError::Io {
            path: path.display().to_string(),
            source,
        })?;
        remove_any(&entry.path())?;
    }
    Ok(())
}

fn remove_target(home: &Path, kind: Kind, target: &Target) -> Result<()> {
    validate(home, &target.path)?;
    if kind == Kind::Fixed {
        return remove_contents(&target.path);
    }
    fs::remove_dir_all(&target.path).map_err(|source| AppError::Io {
        path: target.path.display().to_string(),
        source,
    })
}

pub fn clean_group(home: &Path, group: &Group) -> (u64, Vec<AppError>) {
    let mut freed = 0u64;
    let mut failures = Vec::new();

    for target in &group.targets {
        match remove_target(home, group.rule.kind, target) {
            Ok(()) => freed += target.size,
            Err(err) => failures.push(err),
        }
    }
    (freed, failures)
}
```

- [ ] **Step 4: Run the tests**

Run: `cd gui && cargo test clean::`
Expected: PASS, 11 tests

- [ ] **Step 5: Run the full domain-layer suite**

Run: `cd gui && cargo test`
Expected: PASS, all tests across `format`, `error`, `types`, `config`, `scan`, `clean`

- [ ] **Step 6: Commit**

```bash
git add gui/src/clean/mod.rs
git commit -m "$(cat <<'EOF'
feat: add clean::clean_group, completes the domain layer

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 10: UI bootstrap: window, `AppState`, empty `RootView`

**Files:**
- Modify: `gui/Cargo.toml` (add `gpui`, `gpui-component`)
- Create: `gui/src/ui/mod.rs`
- Create: `gui/src/ui/state.rs`
- Create: `gui/src/ui/root.rs`
- Modify: `gui/src/main.rs` (add `mod ui;`, replace `fn main() {}`)

**Interfaces:**
- Consumes: `crate::config::{DEFAULT_SEARCH_ROOTS, rules}` (Task 5), `crate::types::Group` (Task 4).
- Produces: `pub struct AppState { pub view: View, pub scanning: bool, pub groups: Vec<Group>, pub selected: HashSet<&'static str>, pub roots: Vec<PathBuf>, pub rule_enabled: HashMap<&'static str, bool> }`, `pub enum View { Main, Settings, ConfirmDelete }`, `pub struct RootView`. `AppState` and `View` are consumed by every later UI task.

This task has no automated test (per the spec, UI is verified by running
the app, not unit tests). Its deliverable is a window that opens and shows
placeholder text.

- [ ] **Step 1: Pin the two git dependencies**

Run:
```bash
git ls-remote https://github.com/zed-industries/zed refs/heads/main
git ls-remote https://github.com/longbridge/gpui-component refs/heads/main
```

Each prints a commit hash before `refs/heads/main`. Copy them.

- [ ] **Step 2: Add the dependencies**

In `gui/Cargo.toml`, under `[dependencies]`, add (replacing `<zed-rev>` and
`<gpui-component-rev>` with the hashes from Step 1):
```toml
gpui = { git = "https://github.com/zed-industries/zed", rev = "<zed-rev>", package = "gpui" }
gpui-component = { git = "https://github.com/longbridge/gpui-component", rev = "<gpui-component-rev>" }
```

- [ ] **Step 3: Define `AppState`**

`gui/src/ui/state.rs`:
```rust
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::config::DEFAULT_SEARCH_ROOTS;
use crate::types::Group;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Main,
    Settings,
    ConfirmDelete,
}

pub struct AppState {
    pub view: View,
    pub scanning: bool,
    pub groups: Vec<Group>,
    pub selected: HashSet<&'static str>,
    pub roots: Vec<PathBuf>,
    pub rule_enabled: HashMap<&'static str, bool>,
}

impl AppState {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("no home directory");
        let roots = DEFAULT_SEARCH_ROOTS.iter().map(|r| home.join(r)).collect();

        let mut rule_enabled = HashMap::new();
        for rule in crate::config::rules(&home, &[]) {
            rule_enabled.insert(rule.id, rule.id != "trash");
        }

        Self {
            view: View::Main,
            scanning: false,
            groups: Vec::new(),
            selected: HashSet::new(),
            roots,
            rule_enabled,
        }
    }
}
```

`rules(&home, &[])` here is only used for its 13 ids to seed
`rule_enabled`; the empty search-roots slice never gets walked in this
call. Task 12 calls `rules` again with the real `AppState.roots`.

- [ ] **Step 4: Define an empty `RootView`**

`gui/src/ui/root.rs`:
```rust
use gpui::{div, IntoElement, ParentElement, Render, Styled};

use super::state::AppState;

pub struct RootView {
    pub state: AppState,
}

impl RootView {
    pub fn new() -> Self {
        Self { state: AppState::new() }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div().size_full().child("Cleaner")
    }
}
```

- [ ] **Step 5: Wire the module tree**

`gui/src/ui/mod.rs`:
```rust
pub mod root;
pub mod state;
```

- [ ] **Step 6: Open the window from `main`**

`gui/src/main.rs`:
```rust
mod format;
mod error;
mod types;
mod config;
mod scan;
mod clean;
mod ui;

fn main() {
    gpui::Application::new().run(|cx: &mut gpui::App| {
        gpui_component::init(cx);

        let bounds = gpui::Bounds::centered(
            None,
            gpui::size(gpui::px(760.), gpui::px(560.)),
            cx,
        );

        cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|_| ui::root::RootView::new());
                cx.new(|cx| gpui_component::Root::new(view.into(), window, cx))
            },
        )
        .unwrap();
    });
}
```

- [ ] **Step 7: Build**

Run: `cd gui && cargo build`

This is a git dependency pinned to a specific commit, not a published
crate: if a type or method name in Step 6 or Step 4 doesn't match
(`gpui::Application` vs. `gpui::App::new`, `WindowBounds::Windowed` vs. a
renamed variant, `gpui_component::Root::new`'s exact signature), that's
expected drift, not a wrong approach. Resolve it by reading the actual
pinned source: `cargo doc --open -p gpui -p gpui-component`, or open the
checkout under `~/.cargo/git/checkouts/` and grep for the type. Adjust
Step 4 and Step 6 to match, then rebuild.

Expected once it compiles: `cargo build` succeeds.

- [ ] **Step 8: Run and confirm a window opens**

Run: `cd gui && cargo run`
Expected: a 760x560 window opens showing the text "Cleaner". Close it and
stop the process before continuing.

- [ ] **Step 9: Commit**

```bash
git add gui/Cargo.toml gui/Cargo.lock gui/src/ui gui/src/main.rs
git commit -m "$(cat <<'EOF'
feat: bootstrap gpui window and AppState

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 11: `ui::main_view` with sample data

**Files:**
- Create: `gui/src/ui/main_view.rs`
- Modify: `gui/src/ui/mod.rs` (add `pub mod main_view;`)
- Modify: `gui/src/ui/root.rs` (render `MainView` when `state.view == View::Main`)

**Interfaces:**
- Consumes: `AppState`, `View` (Task 10); `crate::types::Group` (Task 4); `crate::format::human_bytes` (Task 2).
- Produces: `pub fn render_main(state: &AppState, cx: &mut gpui::Context<RootView>) -> impl IntoElement`, called from `RootView::render`. Task 12 replaces the sample groups this task seeds with real scan results; the render function's signature does not change.

No automated test (UI, verified by running). Deliverable: the Main screen
from the mockup, populated with the same sample rows the mockup used, laid
out with `gpui-component`'s `Checkbox` and `Button`.

- [ ] **Step 1: Seed sample groups for layout work**

In `gui/src/ui/state.rs`, change `AppState::new` to populate `groups` and
`selected` with the same sample data the approved mockup used, so the
screen isn't empty while `main_view.rs` is being built:
```rust
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("no home directory");
        let roots = DEFAULT_SEARCH_ROOTS.iter().map(|r| home.join(r)).collect();

        let mut rule_enabled = HashMap::new();
        let rules = crate::config::rules(&home, &[]);
        for rule in &rules {
            rule_enabled.insert(rule.id, rule.id != "trash");
        }

        let selected: HashSet<&'static str> = rules
            .iter()
            .filter(|r| r.id != "cargo-registry")
            .map(|r| r.id)
            .collect();

        Self {
            view: View::Main,
            scanning: false,
            groups: crate::scan::scan(&rules),
            selected,
            roots,
            rule_enabled,
        }
    }
```

This calls the real `scan::scan` against the real filesystem at startup
(matching whatever the machine running it actually has), rather than
fabricating fake `Group` values. Task 12 wires this behind the Rescan
button instead of only running once at launch.

- [ ] **Step 2: Write the Main view**

`gui/src/ui/main_view.rs`:
```rust
use gpui::{div, prelude::*, Context};
use gpui_component::{button::Button, checkbox::Checkbox};

use super::root::RootView;
use super::state::AppState;
use crate::format::human_bytes;
use crate::types::Group;

fn total_size(state: &AppState) -> u64 {
    state.groups.iter().map(|g| g.size).sum()
}

fn selected_total(state: &AppState) -> (usize, u64) {
    let selected: Vec<&Group> = state
        .groups
        .iter()
        .filter(|g| state.selected.contains(g.rule.id))
        .collect();
    let size = selected.iter().map(|g| g.size).sum();
    (selected.len(), size)
}

fn row(group: &Group, checked: bool, cx: &mut Context<RootView>) -> impl IntoElement {
    let id = group.rule.id;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_5()
        .py_2()
        .child(
            Checkbox::new(id)
                .checked(checked)
                .on_click(cx.listener(move |view, checked: &bool, _window, cx| {
                    if *checked {
                        view.state.selected.insert(id);
                    } else {
                        view.state.selected.remove(id);
                    }
                    cx.notify();
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(group.rule.description.to_string()))
                .child(div().text_size(gpui::px(11.)).child(format!("{} / {}", group.rule.id, group.rule.regen))),
        )
        .child(div().child(human_bytes(group.size)))
}

pub fn render_main(state: &AppState, cx: &mut Context<RootView>) -> impl IntoElement {
    let (count, selected_bytes) = selected_total(state);

    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_end()
                .px_5()
                .py_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_size(gpui::px(27.)).child(human_bytes(total_size(state))))
                        .child(div().text_size(gpui::px(11.)).child("reclaimable")),
                )
                .child(Button::new("rescan").label("Rescan").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.scanning = true;
                        cx.notify();
                    },
                ))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .children(state.groups.iter().map(|g| row(g, state.selected.contains(g.rule.id), cx))),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px_5()
                .py_3()
                .child(div().child(format!("{count} of {} groups selected, {}", state.groups.len(), human_bytes(selected_bytes))))
                .child(Button::new("clean-selected").label("Clean Selected...").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.view = super::state::View::ConfirmDelete;
                        cx.notify();
                    },
                ))),
        )
}
```

As with Task 10, `Checkbox`/`Button`'s exact method names
(`.checked()`, `.on_click()`, the closure signature `gpui-component`
expects) come from the pinned commit, not a published API you can look up
externally. Build, read the compiler's suggestions, and check
`cargo doc -p gpui-component --open` for the real signatures if any of
these don't match.

- [ ] **Step 3: Call it from `RootView`**

`gui/src/ui/root.rs`, replace the `Render` impl:
```rust
impl Render for RootView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        match self.state.view {
            super::state::View::Main => super::main_view::render_main(&self.state, cx).into_any_element(),
            super::state::View::Settings => div().child("settings").into_any_element(),
            super::state::View::ConfirmDelete => div().child("confirm").into_any_element(),
        }
    }
}
```

- [ ] **Step 4: Wire the module**

`gui/src/ui/mod.rs`:
```rust
pub mod main_view;
pub mod root;
pub mod state;
```

- [ ] **Step 5: Build and run**

Run: `cd gui && cargo build`, fix any signature mismatches per Step 2's
note, then `cargo run`.
Expected: the window shows the real reclaimable total for whatever the
current machine's `~/Code`, `~/Projects`, `~/src`, `~/work`, `~/dev` and
caches contain, a scrollable list of matched rule groups with working
checkboxes, and a footer with a live selected-count/size. Compare the
layout against the Main screen in the approved mockup. Close the app.

- [ ] **Step 6: Commit**

```bash
git add gui/src/ui
git commit -m "$(cat <<'EOF'
feat: render the main scan list

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 12: Background rescan

**Files:**
- Modify: `gui/src/ui/main_view.rs` (Rescan button's `on_click`)
- Modify: `gui/src/ui/state.rs` (add `RescanInputs` and `AppState::rescan_inputs`)

**Interfaces:**
- Consumes: `crate::config::rules`, `crate::scan::scan` (Tasks 5, 7).
- Produces: `impl AppState { pub fn rescan_inputs(&self) -> RescanInputs }` and `impl RescanInputs { pub fn run(&self) -> Vec<Group> }`, called from a `cx.spawn` in `main_view`'s Rescan handler so the scan runs off the main thread and the window stays responsive.

No automated test (drives real I/O against the real filesystem through a
background executor; verified by running the app). Deliverable: clicking
Rescan shows a scanning state, then updates the list without freezing the
window.

- [ ] **Step 1: Add a `Send`-able rescan snapshot**

`AppState` itself isn't necessarily `Send`, so a background-executor
closure needs a plain-data snapshot of its scan inputs rather than
capturing `&self`. In `gui/src/ui/state.rs`, add to `impl AppState`:
```rust
    pub fn rescan_inputs(&self) -> RescanInputs {
        RescanInputs {
            roots: self.roots.clone(),
            rule_enabled: self.rule_enabled.clone(),
        }
    }
```
and after the closing `}` of `impl AppState`:
```rust
pub struct RescanInputs {
    roots: Vec<PathBuf>,
    rule_enabled: HashMap<&'static str, bool>,
}

impl RescanInputs {
    pub fn run(&self) -> Vec<Group> {
        let home = dirs::home_dir().expect("no home directory");
        let rules: Vec<_> = crate::config::rules(&home, &self.roots)
            .into_iter()
            .filter(|r| *self.rule_enabled.get(r.id).unwrap_or(&true))
            .collect();
        crate::scan::scan(&rules)
    }
}
```

- [ ] **Step 2: Spawn it from the Rescan handler**

In `gui/src/ui/main_view.rs`, replace the Rescan `Button`'s `on_click`:
```rust
                .child(Button::new("rescan").label("Rescan").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.scanning = true;
                        cx.notify();

                        let snapshot = view.state.rescan_inputs();
                        cx.spawn(async move |view, cx| {
                            let groups = cx
                                .background_spawn(async move { snapshot.run() })
                                .await;
                            view.update(cx, |view, cx| {
                                view.state.groups = groups;
                                view.state.scanning = false;
                                cx.notify();
                            })
                            .ok();
                        })
                        .detach();
                    },
                ))),
```

- [ ] **Step 3: Note on the spawn API**

The exact spawn API (`cx.spawn`, `background_spawn`, whether the closure
takes `async move |view, cx|` or a different shape, whether `view.update`
returns a `Result` you `.ok()` or something else) is, again, pinned-commit
specific. Build and adjust against the compiler and
`cargo doc -p gpui --open`, focusing on whatever `gpui::Context::spawn`
and `gpui::App::background_spawn` (or their current names) actually expose
in the pinned revision.

- [ ] **Step 4: Build and run**

Run: `cd gui && cargo build` (fix signatures per Step 3's note), then
`cargo run`.
Expected: clicking "Rescan" doesn't freeze the window, and the list
refreshes with current sizes once the background scan finishes. Close the
app.

- [ ] **Step 5: Commit**

```bash
git add gui/src/ui/main_view.rs gui/src/ui/state.rs
git commit -m "$(cat <<'EOF'
feat: run rescan on the background executor

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 13: `ui::settings_view`

**Files:**
- Create: `gui/src/ui/settings_view.rs`
- Modify: `gui/src/ui/mod.rs` (add `pub mod settings_view;`)
- Modify: `gui/src/ui/root.rs` (render it for `View::Settings`)
- Modify: `gui/src/ui/state.rs` (add a settings draft)

**Interfaces:**
- Consumes: `AppState`, `View` (Task 10); `crate::config::rules` (Task 5).
- Produces: `pub fn render_settings(state: &AppState, cx: &mut Context<RootView>) -> impl IntoElement`.

No automated test (UI). Deliverable: the Settings screen from the mockup,
roots as removable chips plus an add field, one switch per rule id,
Cancel/Save.

- [ ] **Step 1: Add a settings draft to `AppState`**

In `gui/src/ui/state.rs`, add fields to `AppState` and initialize them in
`new`:
```rust
pub struct AppState {
    pub view: View,
    pub scanning: bool,
    pub groups: Vec<Group>,
    pub selected: HashSet<&'static str>,
    pub roots: Vec<PathBuf>,
    pub rule_enabled: HashMap<&'static str, bool>,
    pub draft_roots: Vec<PathBuf>,
    pub draft_rule_enabled: HashMap<&'static str, bool>,
    pub new_root_input: String,
}
```
and in `new`, after `roots` and `rule_enabled` are built:
```rust
        let draft_roots = roots.clone();
        let draft_rule_enabled = rule_enabled.clone();

        Self {
            view: View::Main,
            scanning: false,
            groups: crate::scan::scan(&rules),
            selected,
            roots,
            rule_enabled,
            draft_roots,
            draft_rule_enabled,
            new_root_input: String::new(),
        }
```

- [ ] **Step 2: Write the Settings view**

`gui/src/ui/settings_view.rs`:
```rust
use gpui::{div, prelude::*, Context};
use gpui_component::{button::Button, input::TextInput, switch::Switch};

use super::root::RootView;
use super::state::{AppState, View};

fn root_chip(root: &std::path::Path, cx: &mut Context<RootView>) -> impl IntoElement {
    let path = root.to_path_buf();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .child(div().child(path.display().to_string()))
        .child(Button::new(("remove-root", path.display().to_string())).label("x").on_click(
            cx.listener(move |view, _event, _window, cx| {
                view.state.draft_roots.retain(|r| r != &path);
                cx.notify();
            }),
        ))
}

fn rule_toggle(id: &'static str, label: &str, enabled: bool, cx: &mut Context<RootView>) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .py_2()
        .child(div().child(label.to_string()))
        .child(
            Switch::new(id)
                .checked(enabled)
                .on_click(cx.listener(move |view, checked: &bool, _window, cx| {
                    view.state.draft_rule_enabled.insert(id, *checked);
                    cx.notify();
                })),
        )
}

pub fn render_settings(state: &AppState, cx: &mut Context<RootView>) -> impl IntoElement {
    let home = dirs::home_dir().expect("no home directory");
    let rules = crate::config::rules(&home, &[]);

    div()
        .flex()
        .flex_col()
        .size_full()
        .child(div().text_size(gpui::px(15.)).px_5().py_3().child("Search roots"))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .px_5()
                .children(state.draft_roots.iter().map(|r| root_chip(r, cx))),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .px_5()
                .py_2()
                .child(TextInput::new("new-root").value(state.new_root_input.clone()).on_change(
                    cx.listener(|view, value: &String, _window, cx| {
                        view.state.new_root_input = value.clone();
                        cx.notify();
                    }),
                ))
                .child(Button::new("add-root").label("Add").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        let input = view.state.new_root_input.trim();
                        if !input.is_empty() {
                            let expanded = if let Some(rest) = input.strip_prefix("~/") {
                                dirs::home_dir().expect("no home directory").join(rest)
                            } else {
                                std::path::PathBuf::from(input)
                            };
                            view.state.draft_roots.push(expanded);
                            view.state.new_root_input.clear();
                        }
                        cx.notify();
                    },
                ))),
        )
        .child(div().text_size(gpui::px(15.)).px_5().py_3().child("Rules"))
        .child(
            div()
                .flex()
                .flex_col()
                .px_5()
                .flex_1()
                .children(rules.iter().map(|r| {
                    let enabled = *state.draft_rule_enabled.get(r.id).unwrap_or(&true);
                    rule_toggle(r.id, r.description, enabled, cx)
                })),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap_2()
                .px_5()
                .py_3()
                .child(Button::new("settings-cancel").label("Cancel").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.draft_roots = view.state.roots.clone();
                        view.state.draft_rule_enabled = view.state.rule_enabled.clone();
                        view.state.view = View::Main;
                        cx.notify();
                    },
                )))
                .child(Button::new("settings-save").label("Save").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.roots = view.state.draft_roots.clone();
                        view.state.rule_enabled = view.state.draft_rule_enabled.clone();
                        view.state.view = View::Main;
                        cx.notify();
                    },
                ))),
        )
}
```

`TextInput`'s exact name and `.value()`/`.on_change()` methods are, once
more, whatever the pinned `gpui-component` commit calls them; check
`cargo doc -p gpui-component --open` for the real input widget if this
doesn't compile as written.

- [ ] **Step 3: Wire it into `RootView` and the module tree**

`gui/src/ui/mod.rs`:
```rust
pub mod main_view;
pub mod root;
pub mod settings_view;
pub mod state;
```

`gui/src/ui/root.rs`, update the `Render` match arm:
```rust
            super::state::View::Settings => super::settings_view::render_settings(&self.state, cx).into_any_element(),
```

Also add a way to reach Settings from Main: in `gui/src/ui/main_view.rs`,
add a settings button next to Rescan in the header:
```rust
                .child(Button::new("open-settings").label("Settings").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.view = super::state::View::Settings;
                        cx.notify();
                    },
                )))
```

- [ ] **Step 4: Build and run**

Run: `cd gui && cargo build` (fix signatures per Step 2's note), then
`cargo run`. Click Settings, remove a root chip, add a new one, toggle a
rule switch, click Save, confirm it returns to Main. Reopen Settings,
confirm the changes persisted for the session. Compare layout against the
mockup's Settings screen. Close the app.

- [ ] **Step 5: Commit**

```bash
git add gui/src/ui
git commit -m "$(cat <<'EOF'
feat: add the settings panel

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```

---

### Task 14: `ui::confirm_dialog` and wiring deletion

**Files:**
- Create: `gui/src/ui/confirm_dialog.rs`
- Modify: `gui/src/ui/mod.rs` (add `pub mod confirm_dialog;`)
- Modify: `gui/src/ui/root.rs` (render it over Main for `View::ConfirmDelete`)

**Interfaces:**
- Consumes: `AppState`, `View` (Task 10); `crate::clean::clean_group` (Task 9); `crate::format::human_bytes` (Task 2).
- Produces: `pub fn render_confirm(state: &AppState, cx: &mut Context<RootView>) -> impl IntoElement`. Terminal task: after this, the full flow (scan, select, settings, confirm, delete) is wired end to end.

No automated test (UI). Deliverable: the Confirm screen from the mockup,
listing only selected groups, a standalone warning line when `trash` is
selected, Cancel returns to Main, Delete Selected runs `clean_group` per
selected group on the background executor and returns to Main with
refreshed sizes.

- [ ] **Step 1: Write the Confirm dialog**

`gui/src/ui/confirm_dialog.rs`:
```rust
use gpui::{div, prelude::*, Context};
use gpui_component::button::Button;

use super::root::RootView;
use super::state::{AppState, View};
use crate::format::human_bytes;
use crate::types::Group;

fn selected_groups(state: &AppState) -> Vec<&Group> {
    state.groups.iter().filter(|g| state.selected.contains(g.rule.id)).collect()
}

pub fn render_confirm(state: &AppState, cx: &mut Context<RootView>) -> impl IntoElement {
    let groups = selected_groups(state);
    let total: u64 = groups.iter().map(|g| g.size).sum();
    let has_trash = groups.iter().any(|g| g.rule.id == "trash");

    div()
        .absolute()
        .inset_0()
        .flex()
        .items_start()
        .justify_center()
        .pt_16()
        .child(
            div()
                .w(gpui::px(400.))
                .flex()
                .flex_col()
                .child(div().text_size(gpui::px(15.)).px_4().pt_4().child(format!("Delete {} groups?", groups.len())))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .px_4()
                        .py_2()
                        .children(groups.iter().map(|g| {
                            div()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .child(div().child(g.rule.description.to_string()))
                                .child(div().child(human_bytes(g.size)))
                        })),
                )
                .when(has_trash, |el| {
                    el.child(
                        div()
                            .px_4()
                            .py_2()
                            .child("Trash is a permanent delete, those items will not be recoverable."),
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .px_4()
                        .py_2()
                        .child(div().child("Total"))
                        .child(div().child(human_bytes(total))),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .px_4()
                        .py_3()
                        .child(Button::new("confirm-cancel").label("Cancel").on_click(cx.listener(
                            |view, _event, _window, cx| {
                                view.state.view = View::Main;
                                cx.notify();
                            },
                        )))
                        .child(Button::new("confirm-delete").label("Delete Selected").on_click(cx.listener(
                            |view, _event, _window, cx| {
                                let home = dirs::home_dir().expect("no home directory");
                                let to_delete: Vec<Group> = view
                                    .state
                                    .groups
                                    .iter()
                                    .filter(|g| view.state.selected.contains(g.rule.id))
                                    .cloned()
                                    .collect();

                                cx.spawn(async move |view, cx| {
                                    let home = home;
                                    let remaining = cx
                                        .background_spawn(async move {
                                            for group in &to_delete {
                                                crate::clean::clean_group(&home, group);
                                            }
                                        })
                                        .await;
                                    let _ = remaining;
                                    view.update(cx, |view, cx| {
                                        view.state.view = View::Main;
                                        cx.notify();
                                    })
                                    .ok();
                                })
                                .detach();
                            },
                        ))),
                ),
        )
}
```

`.absolute()`, `.inset_0()`, and `.when()` are plain `gpui` layout
primitives (not `gpui-component`), used instead of a named `Modal` widget
so this task doesn't depend on guessing that widget's API: a positioned
`div` backdrop over Main is enough to match the mockup's dialog-over-main
look. If the pinned `gpui` commit names these differently, `cargo doc -p
gpui --open` has the current names for absolute positioning and
conditional children.

- [ ] **Step 2: Wire it into `RootView`**

`gui/src/ui/mod.rs`:
```rust
pub mod confirm_dialog;
pub mod main_view;
pub mod root;
pub mod settings_view;
pub mod state;
```

`gui/src/ui/root.rs`, render Confirm as an overlay on top of Main:
```rust
impl Render for RootView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let main = super::main_view::render_main(&self.state, cx).into_any_element();
        match self.state.view {
            super::state::View::Main => main,
            super::state::View::Settings => super::settings_view::render_settings(&self.state, cx).into_any_element(),
            super::state::View::ConfirmDelete => div()
                .relative()
                .size_full()
                .child(main)
                .child(super::confirm_dialog::render_confirm(&self.state, cx).into_any_element())
                .into_any_element(),
        }
    }
}
```

- [ ] **Step 3: Build and run the full flow**

Run: `cd gui && cargo build` (fix signatures per Step 1's note), then
`cargo run`. Rescan, check/uncheck a few groups, click "Clean Selected...",
confirm the dialog lists only the checked groups with the right total,
confirm the trash warning shows only when trash is checked, click Delete
Selected, confirm it returns to Main and the deleted groups are gone from
the list (or shrunk, for fixed caches that only had their contents
removed). Compare each screen against the approved mockup one more time.

- [ ] **Step 4: Commit**

```bash
git add gui/src/ui
git commit -m "$(cat <<'EOF'
feat: add confirm-delete dialog, wires the full clean flow

Claude-Session: https://claude.ai/code/session_01WuQFY1xHugPWUbf6MABb1t
EOF
)"
```
