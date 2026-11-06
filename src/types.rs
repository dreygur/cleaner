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

    /// Absolute directories emptied by a Fixed rule.
    pub paths: Vec<PathBuf>,

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
