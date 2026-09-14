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
    pub deleting: bool,
    pub groups: Vec<Group>,
    pub selected: HashSet<&'static str>,
    pub roots: Vec<PathBuf>,
    pub rule_enabled: HashMap<&'static str, bool>,
    pub draft_roots: Vec<PathBuf>,
    pub draft_rule_enabled: HashMap<&'static str, bool>,
    pub new_root_input: String,
    pub last_clean_error: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("no home directory");
        let roots: Vec<PathBuf> = DEFAULT_SEARCH_ROOTS.iter().map(|r| home.join(r)).collect();

        let mut rule_enabled = HashMap::new();
        let rules = crate::config::rules(&home, &[]);
        for rule in &rules {
            rule_enabled.insert(rule.id, rule.id != "trash");
        }

        let enabled_rules: Vec<_> = rules
            .into_iter()
            .filter(|r| *rule_enabled.get(r.id).unwrap_or(&true))
            .collect();

        let selected: HashSet<&'static str> = enabled_rules.iter().map(|r| r.id).collect();

        let draft_roots = roots.clone();
        let draft_rule_enabled = rule_enabled.clone();

        Self {
            view: View::Main,
            scanning: false,
            deleting: false,
            groups: crate::scan::scan(&enabled_rules),
            selected,
            roots,
            rule_enabled,
            draft_roots,
            draft_rule_enabled,
            new_root_input: String::new(),
            last_clean_error: None,
        }
    }

    pub fn rescan_inputs(&self) -> RescanInputs {
        RescanInputs {
            roots: self.roots.clone(),
            rule_enabled: self.rule_enabled.clone(),
        }
    }
}

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
