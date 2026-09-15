use std::path::PathBuf;

/// Selects how a [`Rule`] locates the paths it is responsible for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Walks each root for a directory named `dir` that sits beside `marker`,
    /// so a build output is never confused with an unrelated source folder
    /// that happens to share the name.
    Artifact,
    /// Names fixed, absolute directories whose children are removed. The
    /// directory itself survives so tools that expect it keep working.
    Fixed,
}

/// Describes one class of reclaimable disk space.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Stable slug used to select this rule.
    pub id: &'static str,
    /// One-line summary shown in the scan report.
    pub description: &'static str,
    /// Decides which of the locating fields below apply.
    pub kind: Kind,
    /// Directory name matched by a [`Kind::Artifact`] rule.
    pub dir: &'static str,
    /// File that must exist next to `dir` for a match to count. An empty
    /// marker matches on directory name alone.
    pub marker: &'static str,
    /// Directories a [`Kind::Artifact`] rule walks.
    pub roots: Vec<PathBuf>,
    /// Absolute directories emptied by a [`Kind::Fixed`] rule.
    pub paths: Vec<PathBuf>,
    /// Tells the user how the deleted data comes back, so the report carries
    /// its own undo instructions.
    pub regen: &'static str,
}

/// One concrete directory matched by a [`Rule`], with its measured size.
#[derive(Debug, Clone)]
pub struct Target {
    pub path: PathBuf,
    pub size: u64,
}

/// Every [`Target`] found for a single [`Rule`].
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
