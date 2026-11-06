use std::path::{Component, Path, PathBuf};

use anstyle::{AnsiColor, Style};

const UNIT_STEP: u64 = 1024;
const UNIT_NAMES: &[u8] = b"KMGT";

/// Severity of a status line.
#[derive(Clone, Copy, Debug)]
pub enum Level {
    /// A completed step.
    Info,
    /// A failure the caller could not recover from.
    Error,
    /// A recoverable problem worth surfacing.
    Warn,
}

/// Renders a byte count in binary units, matching what `du -h` prints.
pub fn human_size(b: u64) -> String {
    if b < UNIT_STEP {
        return format!("{b} B");
    }
    let (mut div, mut exp) = (UNIT_STEP, 0);
    let mut n = b / UNIT_STEP;
    while n >= UNIT_STEP {
        div *= UNIT_STEP;
        exp += 1;
        n /= UNIT_STEP;
    }
    format!("{:.1} {}iB", b as f64 / div as f64, UNIT_NAMES[exp] as char)
}

/// Writes a status prefixed, colored line to stdout. Color is dropped when
/// stdout is not a terminal.
pub fn print_log(text: &str, level: Level) {
    let (color, prefix) = match level {
        Level::Info => (AnsiColor::Green, "[+]"),
        Level::Error => (AnsiColor::Red, "[-]"),
        Level::Warn => (AnsiColor::Yellow, "[!]"),
    };
    let style = Style::new().fg_color(Some(color.into()));
    anstream::println!("{style}{prefix} {text}{style:#}");
}

/// Lexical path cleanup, the same as Go's filepath.Clean: drops `.`,
/// resolves `..` against the preceding component, and removes trailing
/// separators. Never touches the filesystem.
pub fn clean_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                let at_parent = matches!(out.components().next_back(), Some(Component::ParentDir));
                if out.parent().is_some() && !at_parent {
                    out.pop();
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_size_matches_du() {
        let cases = [
            (0, "0 B"),
            (1023, "1023 B"),
            (1024, "1.0 KiB"),
            (1536, "1.5 KiB"),
            (1 << 20, "1.0 MiB"),
            (5 << 30, "5.0 GiB"),
            (3 << 40, "3.0 TiB"),
        ];
        for (input, want) in cases {
            assert_eq!(human_size(input), want, "human_size({input})");
        }
    }

    #[test]
    fn clean_path_resolves_dots() {
        assert_eq!(clean_path(Path::new("/a/./b/../c/")), PathBuf::from("/a/c"));
        assert_eq!(clean_path(Path::new("/a/b/../../..")), PathBuf::from("/"));
    }
}
