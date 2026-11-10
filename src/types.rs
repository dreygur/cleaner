use std::path::PathBuf;

/// Selects how a Rule locates the paths it is responsible for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Walks each search root for directories named `dir` that sit beside a
    /// `marker` file, so a build output is never confused with an unrelated
    /// source folder that happens to share the name.
    Artifact,

    /// Names absolute directories whose children are removed. The directory
    /// itself survives so tools that expect it keep working.
    Fixed,

    /// Names system temp directories. Only entries owned by the user,
    /// untouched for a while and holding no socket are removed, so running
    /// programs keep their live files.
    Temp,

    /// Names a container engine CLI. Space is reclaimed by running its
    /// prune `commands`, since images live inside a VM disk image that no
    /// file deletion can shrink.
    Prune,
}

/// Whether a rule runs when the CLI is not told which rules to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Safety {
    /// Runs by default.
    Safe,
    /// Runs only when named in `--only`, for data that is slow or
    /// impossible to get back.
    OptIn,
}

/// How deleted data comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Regen {
    /// A command the user runs, shown as code.
    Run(&'static str),
    /// Happens without the user doing anything, said as a sentence.
    Auto(&'static str),
}

impl Regen {
    pub fn text(self) -> &'static str {
        match self {
            Regen::Run(s) | Regen::Auto(s) => s,
        }
    }
}

impl std::fmt::Display for Regen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Regen::Run(cmd) => write!(f, "regenerate with: {cmd}"),
            Regen::Auto(sentence) => f.write_str(sentence),
        }
    }
}

/// One class of reclaimable disk space.
#[derive(Clone, Debug)]
pub struct Rule {
    /// Stable slug used on the command line to select this rule.
    pub id: &'static str,

    /// One line summary shown in the scan report.
    pub description: &'static str,

    /// Decides which of the locating fields below apply.
    pub kind: Kind,

    /// Directory name matched by an Artifact rule.
    pub dir: &'static str,

    /// File that must exist next to `dir` for a match to count. Empty
    /// matches on directory name alone.
    pub marker: &'static str,

    /// Directories an Artifact rule walks.
    pub roots: Vec<PathBuf>,

    /// Absolute directories an Artifact walk never enters, for trees too
    /// large to walk or holding data that only looks like a build.
    pub prune: Vec<PathBuf>,

    /// Absolute directories emptied by a Fixed rule or swept by a Temp
    /// rule, or the engine binaries a Prune rule runs.
    pub paths: Vec<PathBuf>,

    /// Argument lists a Prune rule runs, in order, against each binary.
    pub commands: &'static [&'static [&'static str]],

    /// `system df` row types whose reclaimable space a Prune rule frees.
    pub df: &'static [&'static str],

    /// Whether the CLI runs this rule without being asked by name.
    pub safety: Safety,

    /// How the deleted data comes back, so the report carries its own undo
    /// instructions.
    pub regen: Regen,
}

/// One concrete directory matched by a Rule, with its measured size.
#[derive(Clone, Debug)]
pub struct Target {
    pub path: PathBuf,
    pub size: u64,
}

/// Every Target found for a single Rule.
#[derive(Clone, Debug)]
pub struct Group {
    pub rule: Rule,
    pub targets: Vec<Target>,
    pub size: u64,
}
