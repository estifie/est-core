//! Supervised child processes: timeouts, output caps, one combined log.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Budgets for one supervised run.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Kill the child past this wall time.
    pub timeout: Duration,
    /// Kill the child past this many log bytes.
    pub max_bytes: u64,
}

impl Limits {
    /// Budgets for one supervised run.
    pub const fn new(timeout: Duration, max_bytes: u64) -> Self {
        Limits { timeout, max_bytes }
    }
}

/// How a supervised run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    /// Exit code when the child exited on its own (None when signaled).
    pub code: Option<i32>,
    /// True when the timeout killed it.
    pub timed_out: bool,
    /// True when the output cap killed it.
    pub capped: bool,
}

impl Outcome {
    /// Exited 0 within budget.
    pub fn success(&self) -> bool {
        !self.timed_out && !self.capped && self.code == Some(0)
    }
}

/// How often the poll looks at the child. `try_wait` is cheap; 50ms
/// notices a finished child fast without spinning.
const POLL_EVERY: Duration = Duration::from_millis(50);

/// Run `cmd` under `limits` in `workdir`, with stdout and stderr combined
/// into `log` (one shared file offset, terminal-style interleave) plus a
/// header naming the command and a footer naming the outcome. Parent
/// directories are created. A killed run is always reaped, so no grandchild
/// can hold a pipe open.
pub fn run_to_file(
    cmd: &mut Command,
    workdir: &Path,
    log: &Path,
    limits: &Limits,
) -> std::io::Result<Outcome> {
    use std::io::Write as _;
    if let Some(parent) = log.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(log)?;
    {
        let mut head = &file;
        let _ = writeln!(head, "$ cd {}", workdir.display());
        let mut parts = vec![cmd.get_program().to_string_lossy().into_owned()];
        parts.extend(cmd.get_args().map(|a| a.to_string_lossy().into_owned()));
        let _ = writeln!(head, "$ {}", parts.join(" "));
        let _ = writeln!(head, "---");
    }
    let err_clone = file.try_clone().ok();
    cmd.current_dir(workdir).stdout(Stdio::from(file));
    match err_clone {
        Some(f) => {
            cmd.stderr(Stdio::from(f));
        }
        None => {
            cmd.stderr(Stdio::null());
        }
    }
    let mut child = cmd.spawn()?;
    // std has no wait-with-timeout, so the wait is a poll. The same poll
    // watches the log size, so one chatty run can never fill the disk.
    let deadline = Instant::now() + limits.timeout;
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break Outcome {
                    code: status.code(),
                    timed_out: false,
                    capped: false,
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    append_footer(
                        log,
                        &format!("TIMEOUT after {}s (killed)", limits.timeout.as_secs()),
                    );
                    break Outcome {
                        code: None,
                        timed_out: true,
                        capped: false,
                    };
                }
                if std::fs::metadata(log).map(|m| m.len()).unwrap_or(0) > limits.max_bytes {
                    let _ = child.kill();
                    let _ = child.wait();
                    append_footer(
                        log,
                        &format!("LOG CAP past {} bytes (killed)", limits.max_bytes),
                    );
                    break Outcome {
                        code: None,
                        timed_out: false,
                        capped: true,
                    };
                }
                std::thread::sleep(POLL_EVERY);
            }
            Err(e) => {
                append_footer(log, &format!("wait failed: {e}"));
                break Outcome {
                    code: None,
                    timed_out: false,
                    capped: false,
                };
            }
        }
    };
    append_footer(
        log,
        &format!("exit: {}", if outcome.success() { "ok" } else { "FAILED" }),
    );
    Ok(outcome)
}

fn append_footer(path: &Path, line: &str) {
    use std::io::Write as _;
    if let Ok(f) = std::fs::OpenOptions::new().append(true).open(path) {
        let mut f = f;
        let _ = writeln!(f, "---\n{line}");
    }
}

/// The PATH of a login shell, for daemons born with a bare environment.
/// None when the probe fails or comes back empty.
pub fn login_shell_path() -> Option<String> {
    let out = Command::new("sh")
        .arg("-l")
        .arg("-c")
        .arg("echo $PATH")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::scratch;

    fn generous() -> Limits {
        Limits::new(Duration::from_secs(30), 10 * 1024 * 1024)
    }

    #[test]
    fn echo_succeeds_with_header_and_footer() {
        let dir = scratch("process-echo");
        let log = dir.join("run.log");
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("echo hi");
        let outcome = run_to_file(&mut cmd, &dir, &log, &generous()).unwrap();
        assert!(outcome.success());
        assert_eq!(outcome.code, Some(0));
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("$ cd "));
        assert!(text.contains("$ sh -c echo hi"));
        assert!(text.contains("hi\n"));
        assert!(text.contains("exit: ok"));
    }

    #[test]
    fn exit_code_and_stderr_land_in_the_log() {
        let dir = scratch("process-code");
        let log = dir.join("run.log");
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("echo out; echo err >&2; exit 3");
        let outcome = run_to_file(&mut cmd, &dir, &log, &generous()).unwrap();
        assert!(!outcome.success());
        assert_eq!(outcome.code, Some(3));
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("out"));
        assert!(text.contains("err"));
        assert!(text.contains("exit: FAILED"));
    }

    #[test]
    fn timeout_kills_and_reports() {
        let dir = scratch("process-timeout");
        let log = dir.join("run.log");
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("sleep 30");
        let limits = Limits::new(Duration::from_secs(1), 1024 * 1024);
        let start = Instant::now();
        let outcome = run_to_file(&mut cmd, &dir, &log, &limits).unwrap();
        assert!(start.elapsed() < Duration::from_secs(15));
        assert!(outcome.timed_out);
        assert!(!outcome.success());
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("TIMEOUT after 1s (killed)"));
    }

    #[test]
    fn output_cap_kills_and_reports() {
        let dir = scratch("process-cap");
        let log = dir.join("run.log");
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("while true; do echo spam; done");
        let limits = Limits::new(Duration::from_secs(30), 4096);
        let outcome = run_to_file(&mut cmd, &dir, &log, &limits).unwrap();
        assert!(outcome.capped);
        assert!(!outcome.success());
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("LOG CAP past 4096 bytes (killed)"));
    }

    #[test]
    fn spawn_failure_is_an_error() {
        let dir = scratch("process-spawn");
        let log = dir.join("run.log");
        let mut cmd = Command::new("/nonexistent/est-core-test-probe");
        assert!(run_to_file(&mut cmd, &dir, &log, &generous()).is_err());
    }

    #[test]
    fn login_shell_has_a_path() {
        let path = login_shell_path().unwrap();
        assert!(!path.trim().is_empty());
    }
}
