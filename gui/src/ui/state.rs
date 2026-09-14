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
    pub groups: Vec<Group>,
    pub selected: HashSet<&'static str>,
    pub roots: Vec<PathBuf>,
    pub rule_enabled: HashMap<&'static str, bool>,
}

impl AppState {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("no home directory");
        let roots = DEFAULT_SEARCH_ROOTS.iter().map(|r| home.join(r)).collect();

        let mut rule_enabled = HashMap::new();
        for rule in crate::config::rules(&home, &[]) {
            rule_enabled.insert(rule.id, rule.id != "trash");
        }

        Self {
            view: View::Main,
            scanning: false,
            groups: Vec::new(),
            selected: HashSet::new(),
            roots,
            rule_enabled,
        }
    }
}
