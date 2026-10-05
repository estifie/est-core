//! Minimal structured logging to stderr. Levels from `EST_LOG`.

use std::sync::atomic::{AtomicU8, Ordering};

/// Verbosity, quietest first. The derived `Ord` keeps
/// `level <= get()` meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Level {
    /// Failures.
    Error = 0,
    /// Something smells.
    Warn = 1,
    /// Normal operation. The default.
    #[default]
    Info = 2,
    /// Debugging detail.
    Debug = 3,
}

impl Level {
    /// `error`, `warn`, `info`, `debug` — ASCII case-insensitive, padded
    /// whitespace tolerated. Anything else is None.
    pub fn parse(text: &str) -> Option<Level> {
        match text.trim().to_ascii_lowercase().as_str() {
            "error" => Some(Level::Error),
            "warn" => Some(Level::Warn),
            "info" => Some(Level::Info),
            "debug" => Some(Level::Debug),
            _ => None,
        }
    }

    /// The lowercase name, for records.
    pub const fn name(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
        }
    }
}

static LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);

/// Read `EST_LOG` into the global level. Missing or unknown values leave
/// it unchanged. Call once at startup; calling again re-reads.
pub fn init() {
    if let Ok(raw) = std::env::var("EST_LOG")
        && let Some(level) = Level::parse(&raw)
    {
        set(level);
    }
}

/// Set the global level outright. For tests and binaries that parse their
/// own verbosity flags.
pub fn set(level: Level) {
    LEVEL.store(level as u8, Ordering::Relaxed);
}

/// The current global level.
pub fn get() -> Level {
    match LEVEL.load(Ordering::Relaxed) {
        0 => Level::Error,
        1 => Level::Warn,
        3 => Level::Debug,
        _ => Level::Info,
    }
}

/// One record on stderr, unless `level` sits above the global one.
pub fn log(level: Level, msg: &str) {
    if level <= get() {
        eprintln!("{}", format_record(now_epoch(), level, msg));
    }
}

/// `log(Level::Error, msg)`.
pub fn error(msg: &str) {
    log(Level::Error, msg);
}

/// `log(Level::Warn, msg)`.
pub fn warn(msg: &str) {
    log(Level::Warn, msg);
}

/// `log(Level::Info, msg)`.
pub fn info(msg: &str) {
    log(Level::Info, msg);
}

/// `log(Level::Debug, msg)`.
pub fn debug(msg: &str) {
    log(Level::Debug, msg);
}

/// `{"ts":<epoch>,"level":"<name>","msg":"<escaped>"}`. Pure, so tests
/// pin the shape without touching stderr.
pub fn format_record(ts: u64, level: Level, msg: &str) -> String {
    format!(
        r#"{{"ts":{ts},"level":"{}","msg":"{}"}}"#,
        level.name(),
        crate::cli::esc(msg)
    )
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_levels() {
        assert_eq!(Level::parse("error"), Some(Level::Error));
        assert_eq!(Level::parse("warn"), Some(Level::Warn));
        assert_eq!(Level::parse("info"), Some(Level::Info));
        assert_eq!(Level::parse("debug"), Some(Level::Debug));
    }

    #[test]
    fn parse_tolerates_case_and_padding() {
        assert_eq!(Level::parse("  WARN "), Some(Level::Warn));
        assert_eq!(Level::parse("Info"), Some(Level::Info));
    }

    #[test]
    fn parse_rejects_the_rest() {
        assert_eq!(Level::parse(""), None);
        assert_eq!(Level::parse("verbose"), None);
        assert_eq!(Level::parse("warning"), None);
        assert_eq!(Level::parse("information"), None);
    }

    #[test]
    fn ordering_and_default() {
        assert!(Level::Error < Level::Warn);
        assert!(Level::Warn < Level::Info);
        assert!(Level::Info < Level::Debug);
        assert_eq!(Level::default(), Level::Info);
    }

    #[test]
    fn record_shape() {
        assert_eq!(
            format_record(1700000000, Level::Warn, "disk \"hot\""),
            r#"{"ts":1700000000,"level":"warn","msg":"disk \"hot\""}"#
        );
    }

    #[test]
    fn set_get_roundtrip() {
        set(Level::Debug);
        assert_eq!(get(), Level::Debug);
        set(Level::Info);
        assert_eq!(get(), Level::Info);
    }
}
