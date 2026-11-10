use std::io;
use std::path::PathBuf;

/// Why a target was refused or could not be removed.
#[derive(thiserror::Error, Debug)]
pub enum CleanError {
    /// The path is not under the home directory or an attached disk. System
    /// and other users' files are never this tool's business.
    #[error("{0:?}: path is outside the home directory")]
    OutsideHome(PathBuf),

    /// The home directory itself, a disk root, or a top level folder that
    /// holds a user's own documents.
    #[error("{0:?}: path is protected")]
    Protected(PathBuf),

    /// Files and symlinks are refused, so a symlinked target can never be
    /// followed into somewhere it should not reach.
    #[error("{0:?}: path is not a real directory")]
    NotDirectory(PathBuf),

    /// Reported apart from NotDirectory so the UI can say why.
    #[error("{0:?}: path is a symlink")]
    Symlink(PathBuf),

    #[error("stat {path:?}: {source}")]
    Stat { path: PathBuf, source: io::Error },

    #[error("remove {path:?}: {source}")]
    Remove { path: PathBuf, source: io::Error },

    #[error("empty {path:?}: {source}")]
    Empty { path: PathBuf, source: io::Error },

    /// A container engine prune command exited non zero.
    #[error("{path:?} {args}: {stderr}")]
    Command { path: PathBuf, args: String, stderr: String },
}

pub type Result<T> = std::result::Result<T, CleanError>;
