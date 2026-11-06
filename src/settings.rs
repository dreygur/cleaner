//! `~/.config/cleaner/config.toml`, shared by the CLI and the Settings
//! screen. A missing file means defaults: search home and every attached
//! disk, keep nothing.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Location under the home directory.
pub const CONFIG_PATH: &str = ".config/cleaner/config.toml";

/// User choices layered over the default search roots.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Settings {
    /// Default roots (home, attached disks) switched off by the user.
    #[serde(default)]
    pub disabled: Vec<PathBuf>,

    /// Folders added with "Add folder".
    #[serde(default)]
    pub extra_roots: Vec<PathBuf>,

    /// Matched paths that are never offered for deletion.
    #[serde(default)]
    pub keep: Vec<PathBuf>,
}

/// Why the config could not be read or written.
#[derive(thiserror::Error, Debug)]
pub enum SettingsError {
    #[error("read {path:?}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("parse {path:?}: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },
    #[error("write {path:?}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("encode config: {0}")]
    Encode(#[from] toml::ser::Error),
}

impl Settings {
    /// Reads path, treating a missing file as defaults.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => return Err(SettingsError::Read { path: path.to_path_buf(), source }),
        };
        toml::from_str(&text).map_err(|source| SettingsError::Parse { path: path.to_path_buf(), source })
    }

    /// Writes path, creating its directory.
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let text = toml::to_string_pretty(self)?;
        let write = |p: &Path| -> io::Result<()> {
            if let Some(dir) = p.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(p, text.as_bytes())
        };
        write(path).map_err(|source| SettingsError::Write { path: path.to_path_buf(), source })
    }

    /// Roots to search: the defaults plus added folders, minus the ones
    /// switched off.
    pub fn roots(&self, defaults: &[PathBuf]) -> Vec<PathBuf> {
        defaults
            .iter()
            .chain(self.extra_roots.iter())
            .filter(|r| !self.disabled.contains(r))
            .cloned()
            .collect()
    }

    pub fn is_kept(&self, path: &Path) -> bool {
        self.keep.iter().any(|k| k == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_default() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings::load(&dir.path().join("nope.toml")).unwrap();
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn round_trip_and_roots() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cleaner/config.toml");
        let s = Settings {
            disabled: vec!["/Volumes/T7".into()],
            extra_roots: vec!["/opt/src".into()],
            keep: vec!["/Users/me/Code/riku/target".into()],
        };
        s.save(&path).unwrap();
        let back = Settings::load(&path).unwrap();
        assert_eq!(back, s);

        let roots = back.roots(&["/Users/me".into(), "/Volumes/T7".into()]);
        assert_eq!(roots, [PathBuf::from("/Users/me"), PathBuf::from("/opt/src")]);
        assert!(back.is_kept(Path::new("/Users/me/Code/riku/target")));
    }

    #[test]
    fn bad_toml_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "keep = 3").unwrap();
        assert!(matches!(Settings::load(&path), Err(SettingsError::Parse { .. })));
    }
}
