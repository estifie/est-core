//! The EST command-line contract, as code.
//!
//! Exit codes, the one JSON envelope, and the rules for destructive
//! commands. The full text lives in `docs/cli-contract-v1.md`.

/// Process exit codes. Every EST binary exits with one of these, except
/// `run`-style wrappers, which exit with the child's own status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    /// Done, as asked.
    Ok = 0,
    /// Tried and failed: missing secret, bad config, child failed.
    Failed = 1,
    /// Wrong usage: bad flags, unknown command, missing argument.
    Usage = 2,
}

impl ExitCode {
    /// The number for `std::process::exit`.
    #[must_use]
    pub const fn code(self) -> i32 {
        self as i32
    }

    /// Exit the process with this code. Never returns.
    pub fn exit(self) -> ! {
        std::process::exit(self.code())
    }
}

/// Contract version carried by every envelope.
pub const ENVELOPE_V: u32 = 1;

/// `{"ok":true,"v":1,...}` around caller-supplied `"key":value` pairs.
///
/// `body` holds the inner pairs without braces (`"secrets":26`); pass an
/// empty string for a bare acknowledgement.
#[must_use]
pub fn ok(body: &str) -> String {
    if body.is_empty() {
        format!(r#"{{"ok":true,"v":{ENVELOPE_V}}}"#)
    } else {
        format!(r#"{{"ok":true,"v":{ENVELOPE_V},{body}}}"#)
    }
}

/// `{"ok":false,"error":"..."}`. The message is escaped; it must never
/// carry secret bytes.
#[must_use]
pub fn err(message: &str) -> String {
    format!(r#"{{"ok":false,"error":"{}"}}"#, esc(message))
}

/// Escape a string for embedding in JSON output. Control characters take
/// short escapes where one exists, `\u00xx` otherwise; everything else —
/// including non-ASCII — passes through untouched.
#[must_use]
pub fn esc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let n = c as u32;
                out.push_str("\\u00");
                out.push(HEX[(n >> 4) as usize] as char);
                out.push(HEX[(n & 0xf) as usize] as char);
            }
            c => out.push(c),
        }
    }
    out
}

/// `prog: message` on stderr. Errors never go to stdout.
pub fn print_error(prog: &str, message: &str) {
    eprintln!("{prog}: {message}");
}

/// Where a destructive command stands: proceed, ask the person, or refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    /// `--yes` was passed: the caller already decided.
    Proceed,
    /// A terminal is attached: ask first.
    Ask,
    /// No `--yes` and no terminal: refuse rather than guess.
    Refuse,
}

/// Classify a destructive command. Agents always pass `--yes`.
#[must_use]
pub const fn confirm_destructive(yes: bool, stdin_is_tty: bool) -> Confirm {
    if yes {
        Confirm::Proceed
    } else if stdin_is_tty {
        Confirm::Ask
    } else {
        Confirm::Refuse
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_0_1_2() {
        assert_eq!(ExitCode::Ok.code(), 0);
        assert_eq!(ExitCode::Failed.code(), 1);
        assert_eq!(ExitCode::Usage.code(), 2);
    }

    #[test]
    fn ok_envelope_shapes() {
        assert_eq!(ok(""), r#"{"ok":true,"v":1}"#);
        assert_eq!(ok(r#""secrets":26"#), r#"{"ok":true,"v":1,"secrets":26}"#);
    }

    #[test]
    fn err_envelope_escapes() {
        assert_eq!(
            err("no job \"x\""),
            r#"{"ok":false,"error":"no job \"x\""}"#
        );
    }

    #[test]
    fn esc_short_forms() {
        assert_eq!(esc("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(esc("a\nb\rc\td"), "a\\nb\\rc\\td");
    }

    #[test]
    fn esc_low_controls_take_hex() {
        assert_eq!(esc("\u{1}\u{1f}"), "\\u0001\\u001f");
    }

    #[test]
    fn esc_leaves_the_rest_alone() {
        assert_eq!(esc("café ✓ \u{7f}~"), "café ✓ \u{7f}~");
    }

    #[test]
    fn confirm_matrix() {
        assert_eq!(confirm_destructive(true, true), Confirm::Proceed);
        assert_eq!(confirm_destructive(true, false), Confirm::Proceed);
        assert_eq!(confirm_destructive(false, true), Confirm::Ask);
        assert_eq!(confirm_destructive(false, false), Confirm::Refuse);
    }
}
