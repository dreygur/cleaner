//! Runs the `cleaner` binary and turns its `--json` lines into events on a
//! channel the UI thread drains.

use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;

use cleaner::report::{DeleteEvent, ScanEvent};

pub enum Event {
    Scan(ScanEvent),
    Delete(DeleteEvent),
}

/// The engine ships next to the UI binary; fall back to PATH.
fn engine() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("cleaner")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("cleaner"))
}

fn pump<T: serde::de::DeserializeOwned + 'static>(child: &mut Child, tx: Sender<Event>, wrap: fn(T) -> Event) {
    let out = child.stdout.take().expect("stdout is piped");
    thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Ok(event) = serde_json::from_str(&line)
                && tx.send(wrap(event)).is_err()
            {
                break;
            }
        }
    });
}

pub fn scan(tx: Sender<Event>) -> io::Result<Child> {
    let mut child = Command::new(engine()).arg("--json").stdout(Stdio::piped()).spawn()?;
    pump(&mut child, tx, Event::Scan);
    Ok(child)
}

/// Hands the engine an explicit path list; it checks each one again.
pub fn delete(paths: &[String], tx: Sender<Event>) -> io::Result<Child> {
    let mut child = Command::new(engine())
        .args(["--json", "--delete"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    stdin.write_all(paths.join("\n").as_bytes())?;
    drop(stdin);
    pump(&mut child, tx, Event::Delete);
    Ok(child)
}

/// The engine treats an interrupt as "finish this group, then stop".
pub fn interrupt(child: &Child) {
    let _ = Command::new("kill").args(["-INT", &child.id().to_string()]).status();
}
