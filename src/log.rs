//! Application log: file (with rotation) + echo on the terminal when there is one.
//! It is meant for diagnostics: `logs` command, application console, export for a bug report.
//! The log contains the searches made and the errors; it never contains a token.
use directories::{ProjectDirs, UserDirs};
use serde::Deserialize;
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{IsTerminal, Write},
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn tag(self) -> &'static str {
        match self {
            Level::Info => "INFO ",
            Level::Warn => "WARN ",
            Level::Error => "ERROR",
        }
    }
    fn from_tag(t: &str) -> Level {
        match t.trim() {
            "ERROR" => Level::Error,
            "WARN" => Level::Warn,
            _ => Level::Info,
        }
    }
}

const FILE_MAX: u64 = 1_000_000; // beyond this: rotation (printsearch.log -> printsearch.log.1)
const RING_MAX: usize = 1000; // fallback if the file is unusable
const TS_LEN: usize = 20; // "2026-11-21 09:27:00Z"

struct Logger {
    file: Option<File>,
    size: u64,
    ring: VecDeque<String>,
    echo: bool,
}

static LOGGER: Mutex<Logger> = Mutex::new(Logger { file: None, size: 0, ring: VecDeque::new(), echo: false });

fn lock() -> std::sync::MutexGuard<'static, Logger> {
    LOGGER.lock().unwrap_or_else(|e| e.into_inner())
}

macro_rules! log_info { ($($a:tt)*) => { $crate::log::write($crate::log::Level::Info, &format!($($a)*)) }; }
macro_rules! log_warn { ($($a:tt)*) => { $crate::log::write($crate::log::Level::Warn, &format!($($a)*)) }; }
macro_rules! log_error { ($($a:tt)*) => { $crate::log::write($crate::log::Level::Error, &format!($($a)*)) }; }
pub(crate) use {log_error, log_info, log_warn};

pub fn dir() -> Option<PathBuf> {
    ProjectDirs::from("dev", "printsearch", "printsearch").map(|d| d.data_local_dir().join("logs"))
}

fn file_path() -> Option<PathBuf> {
    dir().map(|d| d.join("printsearch.log"))
}

fn rotated_path() -> Option<PathBuf> {
    dir().map(|d| d.join("printsearch.log.1"))
}

/// Turns the log echo on the terminal on or off (useful when the program prints its own messages).
#[cfg(all(unix, has_tray))]
pub fn set_echo(on: bool) {
    lock().echo = on;
}

/// Call once at start-up.
pub fn init() {
    let mut g = lock();
    g.echo = std::io::stderr().is_terminal();
    open_file(&mut g);
}

fn open_file(g: &mut Logger) {
    let Some(path) = file_path() else { return };
    let _ = fs::create_dir_all(path.parent().unwrap_or(&path));
    if fs::metadata(&path).map(|m| m.len() > FILE_MAX).unwrap_or(false) {
        if let Some(old) = rotated_path() {
            let _ = fs::rename(&path, old);
        }
    }
    g.size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    g.file = OpenOptions::new().create(true).append(true).open(&path).ok();
}

pub fn write(level: Level, msg: &str) {
    let line = format!("{} {} {}", timestamp(), level.tag(), msg.replace(['\r', '\n'], " | "));
    let mut g = lock();
    let echo = g.echo;
    if g.size > FILE_MAX {
        g.file = None;
        open_file(&mut g);
        if g.size > FILE_MAX {
            // rotation could not happen: start from a fresh file rather than growing endlessly
            if let Some(p) = file_path() {
                let _ = fs::write(p, b"");
            }
            open_file(&mut g);
        }
    }
    let wrote = match g.file.as_mut() {
        Some(f) => writeln!(f, "{line}").is_ok(),
        None => false,
    };
    if wrote {
        g.size += line.len() as u64 + 1;
    } else {
        if g.ring.len() >= RING_MAX {
            g.ring.pop_front();
        }
        g.ring.push_back(line.clone());
    }
    drop(g); // a blocked terminal must not prevent other threads from logging
    if echo {
        // No `eprintln!`: it panics if the terminal has just been closed, which would stop the program abruptly.
        let _ = writeln!(std::io::stderr(), "{line}");
    }
}

/// Installs a crash handler: without a terminal window, a fatal error would be invisible.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        write(Level::Error, &format!("panic: {info}"));
    }));
}

fn read_all() -> String {
    let mut s = String::new();
    if let Some(p) = rotated_path() {
        s.push_str(&fs::read_to_string(p).unwrap_or_default());
    }
    if let Some(p) = file_path() {
        s.push_str(&fs::read_to_string(p).unwrap_or_default());
    }
    let g = lock();
    for l in &g.ring {
        s.push_str(l);
        s.push('\n');
    }
    s
}

fn filter_lines(text: &str, n: usize, min: Level) -> Vec<String> {
    let all: Vec<&str> = text
        .lines()
        .filter(|l| l.len() > TS_LEN + 6 && Level::from_tag(l.get(TS_LEN + 1..TS_LEN + 6).unwrap_or("")) >= min)
        .collect();
    all[all.len().saturating_sub(n)..].iter().map(|s| s.to_string()).collect()
}

/// The last `n` lines (level `min` or more severe), all sessions combined.
pub fn tail(n: usize, min: Level) -> Vec<String> {
    filter_lines(&read_all(), n, min)
}

pub fn clear() {
    for p in [file_path(), rotated_path()].into_iter().flatten() {
        let _ = fs::write(p, b"");
    }
    let mut g = lock();
    g.ring.clear();
    g.size = 0;
}

/// Full text to export: diagnostic header then the whole log.
pub fn export_text(header: &str) -> String {
    format!("{header}\n{}\n{}", "-".repeat(72), read_all())
}

pub fn stamp_for_filename() -> String {
    timestamp().replace(['-', ':'], "").replace(' ', "-").replace('Z', "")
}

/// Writes the export to `dest` (or, by default, the Downloads folder, otherwise the log folder).
pub fn export_to(dest: Option<PathBuf>, header: &str) -> std::io::Result<PathBuf> {
    let path = dest.unwrap_or_else(|| {
        let base = UserDirs::new().and_then(|u| u.download_dir().map(|d| d.to_path_buf())).filter(|d| d.is_dir()).or_else(dir).unwrap_or_else(|| PathBuf::from("."));
        base.join(format!("printsearch-logs-{}.txt", stamp_for_filename()))
    });
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, export_text(header))?;
    Ok(path)
}

pub fn file_location() -> String {
    file_path().map(|p| p.display().to_string()).unwrap_or_else(|| "(no log file available)".into())
}

fn timestamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format_utc(secs)
}

/// Unix seconds -> "YYYY-MM-DD HH:MM:SSZ" (UTC), without an external dependency.
fn format_utc(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z", rem / 3600, (rem % 3600) / 60, rem % 60)
}

// Howard Hinnant's algorithm ("civil_from_days").
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (y + (m <= 2) as i64, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_timestamps() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00Z");
        assert_eq!(format_utc(1_700_000_000), "2023-11-14 22:13:20Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29 00:00:00Z"); // leap year
        assert_eq!(format_utc(1_795_000_000).len(), TS_LEN);
    }

    #[test]
    fn filters_by_level_and_count() {
        let text = "2026-11-21 09:00:00Z INFO  a\n2026-11-21 09:00:01Z WARN  b\n2026-11-21 09:00:02Z ERROR c\n2026-11-21 09:00:03Z INFO  d\n";
        assert_eq!(filter_lines(text, 10, Level::Info).len(), 4);
        assert_eq!(filter_lines(text, 10, Level::Warn), vec!["2026-11-21 09:00:01Z WARN  b", "2026-11-21 09:00:02Z ERROR c"]);
        assert_eq!(filter_lines(text, 2, Level::Info), vec!["2026-11-21 09:00:02Z ERROR c", "2026-11-21 09:00:03Z INFO  d"]);
        assert!(filter_lines("garbage\n\n", 5, Level::Info).is_empty());
    }

    #[test]
    fn multiline_messages_stay_on_one_line() {
        assert_eq!("a\nb\r\nc".replace(['\r', '\n'], " | "), "a | b |  | c");
    }
}
