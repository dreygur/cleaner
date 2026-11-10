//! Docker and Podman keep images, volumes and build cache inside a VM disk
//! image, so space comes back only through their own CLI. Both print the
//! same `system df` rows and take the same prune commands.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{CleanError, Result};

/// Asks for one JSON object per `system df` row.
const DF_ARGS: &[&str] = &["system", "df", "--format", "{{json .}}"];

/// Decimal units, as the engines print sizes with go-units HumanSize.
const UNITS: &[(&str, f64)] = &[
    ("B", 1.0),
    ("kB", 1e3),
    ("KB", 1e3),
    ("MB", 1e6),
    ("GB", 1e9),
    ("TB", 1e12),
];

/// Places the docker and podman CLIs land, since a GUI app starts with a
/// PATH that holds none of them.
const BIN_DIRS: &[&str] = &[
    "/usr/local/bin",
    "/opt/homebrew/bin",
    "/Applications/Docker.app/Contents/Resources/bin",
    "/opt/podman/bin",
];

#[derive(serde::Deserialize)]
struct DfRow {
    #[serde(rename = "Type")]
    kind: String,
    #[serde(rename = "Reclaimable")]
    reclaimable: String,
}

/// Bytes in "9.584GB (90%)".
fn parse_size(text: &str) -> Option<u64> {
    let token = text.split_whitespace().next()?;
    let split = token.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = token.split_at(split);
    let scale = UNITS.iter().find(|(u, _)| *u == unit)?.1;
    Some((num.parse::<f64>().ok()? * scale) as u64)
}

/// Reclaimable bytes of the given row types in `system df` output.
fn reclaimable_in(output: &str, kinds: &[&str]) -> u64 {
    output
        .lines()
        .filter_map(|l| serde_json::from_str::<DfRow>(l).ok())
        .filter(|r| kinds.contains(&r.kind.as_str()))
        .filter_map(|r| parse_size(&r.reclaimable))
        .sum()
}

/// First existing binary named name, looked up in BIN_DIRS then under home.
pub fn find(home: &Path, name: &str) -> Option<PathBuf> {
    BIN_DIRS
        .iter()
        .map(PathBuf::from)
        .chain([home.join(".orbstack/bin")])
        .map(|d| d.join(name))
        .find(|p| p.exists())
}

/// Reclaimable bytes of the given row types, or None when the engine is
/// not running.
pub fn reclaimable(bin: &Path, kinds: &[&str]) -> Option<u64> {
    let out = Command::new(bin).args(DF_ARGS).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(reclaimable_in(&String::from_utf8_lossy(&out.stdout), kinds))
}

/// Runs each argument list against bin, stopping at the first failure.
pub fn prune(bin: &Path, commands: &[&[&str]]) -> Result<()> {
    for args in commands {
        let out = Command::new(bin).args(*args).output().map_err(|source| CleanError::Remove {
            path: bin.to_path_buf(),
            source,
        })?;
        if !out.status.success() {
            return Err(CleanError::Command {
                path: bin.to_path_buf(),
                args: args.join(" "),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    const DF: &str = r#"{"Active":"2","Reclaimable":"9.584GB (90%)","Size":"10.58GB","TotalCount":"13","Type":"Images"}
{"Active":"1","Reclaimable":"196.6kB (50%)","Size":"393.2kB","TotalCount":"2","Type":"Containers"}
{"Active":"2","Reclaimable":"223.4MB (19%)","Size":"1.134GB","TotalCount":"3","Type":"Local Volumes"}
{"Active":"0","Reclaimable":"3.214GB","Size":"7.447GB","TotalCount":"26","Type":"Build Cache"}"#;

    fn mock_engine(dir: &Path, script: &str) -> PathBuf {
        let bin = dir.join("docker");
        fs::write(&bin, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    #[test]
    fn parse_size_reads_decimal_units() {
        assert_eq!(parse_size("9.584GB (90%)"), Some(9_584_000_000));
        assert_eq!(parse_size("196.6kB (50%)"), Some(196_600));
        assert_eq!(parse_size("0B"), Some(0));
        assert_eq!(parse_size("n/a"), None);
    }

    #[test]
    fn reclaimable_in_sums_requested_rows() {
        assert_eq!(reclaimable_in(DF, &["Images"]), 9_584_000_000);
        assert_eq!(reclaimable_in(DF, &["Containers", "Build Cache"]), 3_214_196_600);
        assert_eq!(reclaimable_in("not json", &["Images"]), 0);
    }

    #[test]
    fn reclaimable_is_none_when_engine_is_down() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = mock_engine(tmp.path(), "exit 1");
        assert_eq!(reclaimable(&bin, &["Images"]), None);
    }

    #[test]
    fn prune_runs_each_command_and_reports_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("log");
        let bin = mock_engine(tmp.path(), &format!("echo \"$@\" >> {}\n[ \"$1\" = bad ] && echo nope >&2 && exit 1\nexit 0", log.display()));

        prune(&bin, &[&["system", "prune", "-f"], &["builder", "prune", "-af"]]).unwrap();
        assert_eq!(fs::read_to_string(&log).unwrap(), "system prune -f\nbuilder prune -af\n");

        let err = prune(&bin, &[&["bad"], &["never"]]).unwrap_err();
        assert!(matches!(&err, CleanError::Command { stderr, .. } if stderr == "nope"), "{err:?}");
        assert!(!fs::read_to_string(&log).unwrap().contains("never"));
    }

    #[test]
    fn find_looks_under_home() {
        let home = tempfile::tempdir().unwrap();
        fs::create_dir_all(home.path().join(".orbstack/bin")).unwrap();
        let bin = mock_engine(&home.path().join(".orbstack/bin"), "exit 0");
        let name = "docker-cleaner-test";
        assert_eq!(find(home.path(), name), None);
        fs::rename(&bin, home.path().join(".orbstack/bin").join(name)).unwrap();
        assert_eq!(find(home.path(), name), Some(home.path().join(".orbstack/bin").join(name)));
    }
}
