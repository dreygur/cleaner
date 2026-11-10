//! The `--json` wire format. The UI spawns the CLI and reads one JSON object
//! per line from stdout, so both sides share these types.

use serde::{Deserialize, Serialize};

/// How a rule's data comes back is shown on every row, so the kind travels
/// with the group.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// Found by search, deleted whole.
    Artifact,
    /// A known cache, emptied but kept.
    Fixed,
    /// A system temp directory, cleared of stale entries.
    Temp,
    /// A container engine, pruned through its CLI.
    Prune,
}

/// One concrete directory a rule matched.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TargetReport {
    /// Absolute path.
    pub path: String,
    pub size: u64,
    /// Newest modification time of the project around the target, in unix
    /// seconds. Absent for fixed caches, which have no project.
    pub modified: Option<u64>,
}

/// Everything one rule matched.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GroupReport {
    pub id: String,
    pub description: String,
    pub kind: RuleKind,
    pub regen: String,
    /// True when regen is a command to run, false when it is a sentence.
    pub regen_is_command: bool,
    pub size: u64,
    pub targets: Vec<TargetReport>,
}

/// Lines printed by `cleaner --json`. A scan never deletes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ScanEvent {
    /// First line: what is about to be searched.
    Started {
        home: String,
        free: u64,
        roots: Vec<String>,
        disks: Vec<String>,
        rules: usize,
    },
    /// A search root has been walked completely.
    RootDone { root: String },
    /// A rule was measured and matched something.
    Group { group: GroupReport },
    /// A rule was measured and matched nothing.
    Empty { id: String },
    /// Last line.
    Finished { elapsed_ms: u64 },
}

/// Why one path was not deleted. The UI shows each as a reason tag.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailReason {
    PermissionDenied,
    SymlinkRejected,
    Gone,
    Protected,
    /// The path does not match any rule, or is on the keep list.
    NotAllowed,
    Other,
}

/// Lines printed by `cleaner --json --delete -`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DeleteEvent {
    Started { free: u64, targets: usize },
    /// A path passed its checks and is being removed now.
    Removing { rule: String, path: String },
    Removed { rule: String, path: String, freed: u64 },
    Failed {
        rule: Option<String>,
        path: String,
        reason: FailReason,
        message: String,
    },
    /// An interrupt arrived; the group in progress finished and the rest was
    /// left alone.
    Stopped,
    Finished { freed: u64, free_after: u64 },
}
