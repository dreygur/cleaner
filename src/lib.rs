//! Cleaner engine. The `cleaner` binary is the only thing that deletes; the
//! UI drives it through `--json` and shares the wire types in `report`.

pub mod clean;
pub mod config;
pub mod error;
pub mod remove;
pub mod report;
pub mod scan;
pub mod settings;
pub mod stream;
pub mod types;
pub mod util;
pub mod volumes;
pub mod walker;
