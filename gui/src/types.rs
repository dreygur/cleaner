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
