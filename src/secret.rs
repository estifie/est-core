//! Secrets in memory: hard to leak by accident.
//!
//! [`Secret`] redacts `Debug` and offers no `Display`; [`redact`] scrubs
//! known values out of text bound for logs; [`audit_reveal`] records every
//! value deliberately shown on screen.

use std::fmt;
use std::path::Path;

/// A value that refuses to print. `Debug` shows a fixed marker; there is
/// deliberately no `Display` impl, so `{}` on one does not compile.
#[derive(Clone)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// Wrap a value at the boundary where it enters memory.
    pub const fn new(value: T) -> Self {
        Secret(value)
    }

    /// Unwrap where the value is actually needed — and only there.
    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

/// Replace every occurrence of each known secret in `text` with `***`.
/// Empty needles are skipped, so a missing value cannot blank the text.
///
/// ```
/// # use est_core::secret::redact;
/// assert_eq!(redact("key=hunter2 ok", &["hunter2"]), "key=*** ok");
/// ```
#[must_use]
pub fn redact(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for s in secrets.iter().filter(|s| !s.is_empty()) {
        out = out.replace(s, "***");
    }
    out
}

/// Append one `epoch address` line to the reveal log, creating parent
/// directories as needed. Addresses only — values never land here.
pub fn audit_reveal(log_path: &Path, address: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    if let Some(parent) = log_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    writeln!(file, "{epoch} {address}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_the_value() {
        let s = Secret::new("hunter2".to_string());
        assert_eq!(format!("{s:?}"), "Secret([redacted])");
        assert_eq!(s.expose(), "hunter2");
    }

    #[test]
    fn redact_replaces_all() {
        assert_eq!(redact("a hunter2 b hunter2", &["hunter2"]), "a *** b ***");
        assert_eq!(redact("a x b y", &["x", "y"]), "a *** b ***");
    }

    #[test]
    fn redact_skips_empty_and_missing() {
        assert_eq!(redact("untouched", &["", "nope"]), "untouched");
    }

    #[test]
    fn audit_appends_address_lines() {
        let dir = std::env::temp_dir().join(format!("est-core-test-{}-audit", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let log = dir.join("sub").join("reveals.log");
        audit_reveal(&log, "TELEGRAM:BOT:DEMO").unwrap();
        audit_reveal(&log, "GITHUB:PAT:ME").unwrap();
        let text = std::fs::read_to_string(&log).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with(" TELEGRAM:BOT:DEMO"));
        assert!(lines[1].ends_with(" GITHUB:PAT:ME"));
    }
}
