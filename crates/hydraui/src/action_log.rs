//! `<HYDRA_HOME>/hydraui/hydraui.log`: what hydraui did and why, one timestamped line per
//! step, for finding out why something didn't happen (a card not sent, a session skipped).
//! Every hydraui window appends to the same file, each line tagged with its process id. A log
//! over 1 MB at startup is moved to `hydraui.log.old`, replacing the one before.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// A log bigger than this at startup is set aside.
const MAX_BYTES: u64 = 1024 * 1024;

static FILE: OnceLock<PathBuf> = OnceLock::new();
static WRITING: Mutex<()> = Mutex::new(());

/// Where the log goes for `hydra_root` (`HydraPaths::root`).
pub fn file_for(hydra_root: &Path) -> PathBuf {
    hydra_root.join("hydraui").join("hydraui.log")
}

/// Logs to `path` from now on, first setting aside a log over 1 MB. Called once, at startup.
pub fn init(path: PathBuf) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    set_aside_if_big(&path, MAX_BYTES);
    let _ = FILE.set(path);
}

/// The log file, once `init` has run.
pub fn file() -> Option<&'static Path> {
    FILE.get().map(PathBuf::as_path)
}

/// Moves `path` to `<path>.old` when it is bigger than `max` bytes.
fn set_aside_if_big(path: &Path, max: u64) {
    if std::fs::metadata(path).is_ok_and(|m| m.len() > max) {
        let mut old = path.as_os_str().to_owned();
        old.push(".old");
        let _ = std::fs::rename(path, old);
    }
}

/// Appends `text` to the log, stamped with the local time and this process's id. Does nothing
/// before `init`; a log that can't be written is skipped silently.
/// What a panic leaves in the log: the thread, where, the message, then the backtrace.
pub fn panic_entry(message: &str, location: Option<&str>, thread: &str, backtrace: &str) -> String {
    let at = location.map_or(String::new(), |l| format!(" at {l}"));
    let mut text = format!("PANIC in thread {thread}{at}: {message}");
    if !backtrace.trim().is_empty() {
        text.push('\n');
        text.push_str(backtrace.trim_end());
    }
    text
}

/// Writes every panic to the log with its backtrace, then lets the default hook print it too:
/// a crash otherwise leaves its reason only in whichever terminal started hydraui.
pub fn record_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".to_string());
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        let current = std::thread::current();
        let thread = current.name().unwrap_or("unnamed");
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        write(panic_entry(
            &message,
            location.as_deref(),
            thread,
            &backtrace,
        ));
        previous(info);
    }));
}

pub fn write(text: impl AsRef<str>) {
    let Some(path) = file() else { return };
    let entry = entry(&now(), std::process::id(), text.as_ref());
    let _guard = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = f.write_all(entry.as_bytes());
    }
}

/// One log entry: `<stamp> [<pid>] <text>`, each further line of `text` indented under it.
fn entry(stamp: &str, pid: u32, text: &str) -> String {
    let mut lines = text.trim_end().lines();
    let mut out = format!("{stamp} [{pid}] {}\n", lines.next().unwrap_or(""));
    for line in lines {
        out.push_str("    ");
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// The local time as `2026-10-21 08:45:12.345`.
#[cfg(windows)]
pub(crate) fn now() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: GetLocalTime only fills in the struct it is given.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

/// The local time as `2026-10-21 08:45:12.345`.
#[cfg(unix)]
pub(crate) fn now() -> String {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = libc::time_t::try_from(since.as_secs()).unwrap_or(0);
    // SAFETY: localtime_r only reads `secs` and fills in the struct it is given.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        libc::localtime_r(&secs, &mut t);
        t
    };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.tm_year + 1900,
        t.tm_mon + 1,
        t.tm_mday,
        t.tm_hour,
        t.tm_min,
        t.tm_sec,
        since.subsec_millis()
    )
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_panic_is_written_with_where_it_happened() {
        let text = panic_entry(
            "index out of bounds: the len is 2 but the index is 3",
            Some("crates/hydraui/src/runs.rs:42:9"),
            "main",
            "   0: hydraui::runs::Runs::begin",
        );
        assert!(
            text.starts_with(
                "PANIC in thread main at crates/hydraui/src/runs.rs:42:9: index out of bounds"
            ),
            "{text}"
        );
        assert!(text.contains("hydraui::runs::Runs::begin"), "{text}");
        let bare = panic_entry("boom", None, "hydraui-mcp", "");
        assert_eq!(bare, "PANIC in thread hydraui-mcp: boom");
    }

    use super::*;

    #[test]
    fn an_entry_is_one_stamped_line() {
        assert_eq!(
            entry("2026-10-21 08:45:12.345", 42, "pickup: offer"),
            "2026-10-21 08:45:12.345 [42] pickup: offer\n"
        );
    }

    #[test]
    fn further_lines_are_indented_under_the_first() {
        assert_eq!(
            entry("T", 1, "Jev was sent:\n  first row  \nsecond row\n\n"),
            "T [1] Jev was sent:\n      first row\n    second row\n"
        );
    }

    #[test]
    fn the_time_has_its_fixed_shape() {
        let t = now();
        assert_eq!(t.len(), "2026-10-21 08:45:12.345".len(), "{t}");
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[10..11], " ");
    }

    #[test]
    fn a_big_log_is_set_aside_and_a_small_one_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hydraui.log");
        std::fs::write(&path, "0123456789").unwrap();
        set_aside_if_big(&path, 20);
        assert!(path.exists());
        set_aside_if_big(&path, 5);
        assert!(!path.exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("hydraui.log.old")).unwrap(),
            "0123456789"
        );
    }
}
