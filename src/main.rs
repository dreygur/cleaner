use std::collections::BTreeSet;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;

use cleaner::clean::{self, Scope};
use cleaner::settings::{CONFIG_PATH, Settings};
use cleaner::stream::{self, Emitter, OnGone, ScanPlan};
use cleaner::types::{Group, Rule};
use cleaner::util::{Level, human_size, print_log};
use cleaner::{config, scan, volumes};

/// Hides targets too small to be worth a decision.
const MIN_REPORT_SIZE: u64 = 1 << 20;

/// Disk reclaimer for macOS development machines.
#[derive(Parser)]
#[command(name = "cleaner", version)]
struct Cli {
    /// Delete every matched group without prompting
    #[arg(long)]
    yes: bool,

    /// Report what would be deleted, then exit
    #[arg(long)]
    dry_run: bool,

    /// Comma separated rule ids to run exclusively
    #[arg(long, default_value = "")]
    only: String,

    /// Comma separated rule ids to leave alone
    #[arg(long, default_value = "")]
    skip: String,

    /// Comma separated directories to search for build artifacts
    #[arg(long, default_value = "")]
    roots: String,

    /// Print known rule ids and what they remove, then exit
    #[arg(long)]
    list_rules: bool,

    /// Print one JSON event per line instead of the report. Never deletes
    /// unless --delete is also given
    #[arg(long)]
    json: bool,

    /// With --json: delete the paths read from stdin, one per line. Each
    /// path must be one a scan would offer
    #[arg(long, requires = "json", conflicts_with_all = ["yes", "dry_run"])]
    delete: bool,
}

fn parse_list(raw: &str) -> BTreeSet<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

fn unknown_ids(all: &[Rule], names: &BTreeSet<String>) -> Vec<String> {
    names
        .iter()
        .filter(|n| !all.iter().any(|r| r.id == n.as_str()))
        .cloned()
        .collect()
}

fn check_ids(all: &[Rule], flag: &str, names: &BTreeSet<String>) -> Result<(), String> {
    let unknown = unknown_ids(all, names);
    if unknown.is_empty() {
        return Ok(());
    }
    let mut ids: Vec<&str> = all.iter().map(|r| r.id).collect();
    ids.sort_unstable();
    Err(format!(
        "unknown rule id in --{flag}: {} (known: {})",
        unknown.join(", "),
        ids.join(", ")
    ))
}

fn select_rules(all: Vec<Rule>, only: &BTreeSet<String>, skip: &BTreeSet<String>) -> Vec<Rule> {
    all.into_iter()
        .filter(|r| only.is_empty() || only.contains(r.id))
        .filter(|r| !skip.contains(r.id))
        .collect()
}

/// Defaults to the whole home directory plus every attached local disk, so
/// projects outside a conventional folder are still found, adjusted by the
/// config file. --roots overrides both.
fn resolve_roots(home: &Path, mounts: &[PathBuf], settings: &Settings, raw: &str) -> Vec<PathBuf> {
    if raw.is_empty() {
        let defaults: Vec<PathBuf> = std::iter::once(home.to_path_buf()).chain(mounts.iter().cloned()).collect();
        return settings.roots(&defaults);
    }
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn confirm(question: &str) -> bool {
    print!("    {question} [y/N] ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().lock().read_line(&mut answer).unwrap_or(0) == 0 {
        return false;
    }
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Shortens paths under home to ~/..., and leaves paths on other disks
/// absolute.
fn display_path(home: &Path, path: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rel) => format!("~/{}", rel.display()),
        Err(_) => path.display().to_string(),
    }
}

fn report(groups: &[Group], home: &Path) -> u64 {
    let mut total = 0;
    for g in groups {
        total += g.size;
        println!("  {:<16} {:>10}  {}", g.rule.id, human_size(g.size), g.rule.description);
        for t in g.targets.iter().filter(|t| t.size >= MIN_REPORT_SIZE) {
            println!("  {:<16} {:>10}    {}", "", human_size(t.size), display_path(home, &t.path));
        }
        println!("  {:<16} {:>10}    {}\n", "", "", g.rule.regen);
    }
    total
}

fn list_rules(rules: &[Rule]) {
    let mut sorted: Vec<&Rule> = rules.iter().collect();
    sorted.sort_by_key(|r| r.id);
    for r in sorted {
        println!("  {:<16} {:<40} {}", r.id, r.description, r.regen);
    }
}

fn sweep(scope: &Scope, groups: &[Group], assume_yes: bool, interrupted: &AtomicBool) -> u64 {
    let mut freed = 0;
    for g in groups {
        if interrupted.load(Ordering::SeqCst) {
            print_log(&format!("interrupted, stopping before {}", g.rule.id), Level::Warn);
            return freed;
        }
        if !assume_yes && !confirm(&format!("delete {} ({})?", g.rule.id, human_size(g.size))) {
            print_log(&format!("skipped {}", g.rule.id), Level::Warn);
            continue;
        }
        let (got, failures) = clean::group(scope, g);
        freed += got;
        for err in failures {
            print_log(&err.to_string(), Level::Error);
        }
        print_log(&format!("{} freed {}", g.rule.id, human_size(got)), Level::Info);
    }
    freed
}

fn free_text(home: &Path) -> String {
    match volumes::free_bytes(home) {
        Some(n) => human_size(n),
        None => {
            print_log(&format!("cannot read free space for {}", home.display()), Level::Warn);
            human_size(0)
        }
    }
}

fn on_interrupt() -> Result<Arc<AtomicBool>, String> {
    let interrupted = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&interrupted);
    ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst))
        .map_err(|e| format!("cannot install interrupt handler: {e}"))?;
    Ok(interrupted)
}

fn read_paths() -> Vec<PathBuf> {
    io::stdin()
        .lock()
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

fn run_json(cli: &Cli, home: &Path, mounts: Vec<PathBuf>, roots: &[PathBuf], rules: &[Rule], keep: &[PathBuf]) -> Result<(), String> {
    let free = || volumes::free_bytes(home).unwrap_or(0);
    if !cli.delete {
        let out = Emitter::new(io::stdout(), OnGone::Exit);
        let plan = ScanPlan { home, free: free(), roots, disks: &mounts };
        stream::scan_json(&plan, rules, keep, &out);
        return Ok(());
    }
    let out = Emitter::new(io::stdout(), OnGone::Continue);
    let interrupted = on_interrupt()?;
    let scope = Scope { home: home.to_path_buf(), volumes: mounts };
    stream::delete_json(&scope, rules, keep, &read_paths(), free, &interrupted, &out);
    Ok(())
}

fn run(cli: Cli) -> Result<(), String> {
    let home = std::env::home_dir().ok_or("cannot resolve home directory")?;
    let settings = Settings::load(&home.join(CONFIG_PATH)).map_err(|e| e.to_string())?;
    let mounts = volumes::list(Path::new(volumes::MOUNT_DIR));
    let roots = resolve_roots(&home, &mounts, &settings, &cli.roots);
    let all = config::rules(&home, &roots);

    if cli.list_rules {
        list_rules(&all);
        return Ok(());
    }

    let (only, skip) = (parse_list(&cli.only), parse_list(&cli.skip));
    check_ids(&all, "only", &only)?;
    check_ids(&all, "skip", &skip)?;

    let rules = select_rules(all, &only, &skip);
    if cli.json {
        return run_json(&cli, &home, mounts, &roots, &rules, &settings.keep);
    }
    if rules.is_empty() {
        print_log("no rules selected", Level::Warn);
        return Ok(());
    }

    print_log(&format!("scanning {} rules, {} free", rules.len(), free_text(&home)), Level::Info);
    for m in &mounts {
        print_log(&format!("including disk {}", m.display()), Level::Info);
    }

    let mut groups = scan::scan(&rules, &settings.keep);
    groups.sort_by_key(|g| std::cmp::Reverse(g.size));
    if groups.is_empty() {
        print_log("nothing to clean", Level::Info);
        return Ok(());
    }

    println!();
    let total = report(&groups, &home);
    print_log(&format!("reclaimable: {}", human_size(total)), Level::Info);

    if cli.dry_run {
        return Ok(());
    }

    let interrupted = on_interrupt()?;
    let scope = Scope { home: home.clone(), volumes: mounts };
    let freed = sweep(&scope, &groups, cli.yes, &interrupted);
    print_log(&format!("freed {}, now {} free", human_size(freed), free_text(&home)), Level::Info);
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            print_log(&msg, Level::Error);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_path_shortens_home_only() {
        let home = Path::new("/Users/me");
        let cases = [
            ("/Users/me/Code/app/target", "~/Code/app/target"),
            ("/Volumes/Disk/proj/target", "/Volumes/Disk/proj/target"),
            ("/Users/meow/Code/app/target", "/Users/meow/Code/app/target"),
        ];
        for (path, want) in cases {
            assert_eq!(display_path(home, Path::new(path)), want);
        }
    }

    #[test]
    fn select_rules_honors_only_and_skip() {
        let all = config::rules(Path::new("/h"), &[]);
        let only = parse_list("trash, go-build");
        let skip = parse_list("go-build");

        let chosen = select_rules(all.clone(), &only, &skip);
        let ids: Vec<&str> = chosen.iter().map(|r| r.id).collect();
        assert_eq!(ids, ["trash"]);
        assert!(check_ids(&all, "only", &parse_list("nope")).is_err());
    }
}
