//! Typed configuration for the `[core]` section, plus duration syntax.
//!
//! The narrow TOML this needs and nothing more: `[section]` headers,
//! `key = value` lines, `#` comments, quoted strings (`\" \\ \n`
//! escapes), bare numbers and bools. Unknown sections and keys warn and
//! are ignored; malformed lines fail with a line number. Products with
//! bigger configs should use the `toml` crate instead of growing this.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::log::Level;

/// Shared `[core]` settings. Products embed these beside their own tables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoreConfig {
    /// Log verbosity. Default `Info`.
    pub log_level: Level,
    /// State directory override. Default: the product decides.
    pub state_dir: Option<PathBuf>,
    /// Log directory override. Default: the product decides.
    pub log_dir: Option<PathBuf>,
    /// Dashboard port. Default: no listener.
    pub port: Option<u16>,
}

/// Load `path`, or defaults with a warning when the file is absent. Other
/// I/O failures — and malformed content — are errors. Errors and warnings
/// read `path:line: message`.
pub fn load(path: &Path) -> Result<(CoreConfig, Vec<String>), String> {
    let name = path.display().to_string();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((
                CoreConfig::default(),
                vec![format!("{name}: file not found, using defaults")],
            ));
        }
        Err(e) => return Err(format!("{name}: cannot read: {e}")),
    };
    parse_named(&name, &text)
}

/// Parse without a file: forms and tests validate this way.
pub fn parse_str(text: &str) -> Result<(CoreConfig, Vec<String>), String> {
    parse_named("config", text)
}

fn parse_named(name: &str, text: &str) -> Result<(CoreConfig, Vec<String>), String> {
    let (config, warns) = parse(text).map_err(|(line, msg)| format!("{name}:{line}: {msg}"))?;
    let warns = warns
        .into_iter()
        .map(|(line, msg)| format!("{name}:{line}: {msg}"))
        .collect();
    Ok((config, warns))
}

#[derive(Default)]
struct CoreBuilder {
    log_level: Option<Level>,
    state_dir: Option<PathBuf>,
    log_dir: Option<PathBuf>,
    port: Option<u16>,
}

#[derive(PartialEq, Eq)]
enum Section {
    None,
    Core,
    Other,
}

/// A warning or error pinned to its line.
type Issue = (usize, String);

fn parse(text: &str) -> Result<(CoreConfig, Vec<Issue>), Issue> {
    let mut core = CoreBuilder::default();
    let mut warns = Vec::new();
    let mut warned_sections = HashSet::new();
    let mut section = Section::None;
    for (n, raw) in text.lines().enumerate() {
        let at = n + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = section_header(line) {
            if name != "core" && warned_sections.insert(name.clone()) {
                warns.push((at, format!("unknown section [{name}], ignored")));
            }
            section = if name == "core" {
                Section::Core
            } else {
                Section::Other
            };
            continue;
        }
        if line.starts_with('[') {
            return Err((at, "bad section header".to_string()));
        }
        let Some((key, value)) = split_key(line) else {
            return Err((at, "expected key = value".to_string()));
        };
        match section {
            Section::None => return Err((at, "key outside any section".to_string())),
            Section::Other => {}
            Section::Core => set_core(&mut core, key, value, at, &mut warns)?,
        }
    }
    Ok((
        CoreConfig {
            log_level: core.log_level.unwrap_or_default(),
            state_dir: core.state_dir,
            log_dir: core.log_dir,
            port: core.port,
        },
        warns,
    ))
}

fn set_core(
    core: &mut CoreBuilder,
    key: &str,
    value: &str,
    at: usize,
    warns: &mut Vec<Issue>,
) -> Result<(), Issue> {
    if !valid_key(key) {
        return Err((at, format!("bad key {key:?}")));
    }
    match key {
        "log_level" => {
            let raw = parse_string(value, at)?;
            let Some(level) = Level::parse(&raw) else {
                return Err((
                    at,
                    format!("log_level is one of: error warn info debug (got {raw:?})"),
                ));
            };
            dup(core.log_level.replace(level), "log_level", at, warns);
        }
        "state_dir" => {
            let dir = PathBuf::from(parse_string(value, at)?);
            dup(core.state_dir.replace(dir), "state_dir", at, warns);
        }
        "log_dir" => {
            let dir = PathBuf::from(parse_string(value, at)?);
            dup(core.log_dir.replace(dir), "log_dir", at, warns);
        }
        "port" => {
            let port: u16 = value
                .parse()
                .ok()
                .filter(|p| *p > 0)
                .ok_or_else(|| (at, format!("port is 1-65535 (got {value:?})")))?;
            dup(core.port.replace(port), "port", at, warns);
        }
        _ => warns.push((at, format!("unknown key {key:?}, ignored"))),
    }
    Ok(())
}

/// Last write wins, loudly.
fn dup<T>(old: Option<T>, key: &str, at: usize, warns: &mut Vec<Issue>) {
    if old.is_some() {
        warns.push((at, format!("duplicate {key}, last wins")));
    }
}

/// `[name]`, without the brackets. None when this is not a header line.
fn section_header(line: &str) -> Option<String> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?.trim();
    if inner.is_empty() || inner.contains(['[', ']', '#', '=']) {
        return None;
    }
    Some(inner.to_string())
}

/// The first `=` splits key from value: keys are bare words, so the
/// separator cannot hide inside one.
fn split_key(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim(), value.trim()))
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Cut the `#` comment, unless it sits inside a quoted string.
fn strip_comment(line: &str) -> &str {
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_str => escaped = true,
            '"' => in_str = !in_str,
            '#' if !in_str => return &line[..i],
            _ => {}
        }
    }
    line
}

fn parse_string(value: &str, at: usize) -> Result<String, Issue> {
    let inner = value
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or((at, "expected a \"quoted string\"".to_string()))?;
    unescape(inner, at)
}

fn unescape(inner: &str, at: usize) -> Result<String, Issue> {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            _ => return Err((at, "bad escape (use \\\" \\\\ \\n)".to_string())),
        }
    }
    Ok(out)
}

/// "30s", "15m", "24h", "7d", or bare seconds. Zero, negatives, unknown
/// suffixes, and overflow are all None.
pub fn parse_duration(text: &str) -> Option<u64> {
    let text = text.trim();
    let (number, mult) = if let Some(n) = text.strip_suffix('s') {
        (n, 1)
    } else if let Some(n) = text.strip_suffix('m') {
        (n, 60)
    } else if let Some(n) = text.strip_suffix('h') {
        (n, 3600)
    } else if let Some(n) = text.strip_suffix('d') {
        (n, 86400)
    } else {
        (text, 1)
    };
    number
        .trim()
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(mult))
        .filter(|&n| n > 0)
}

/// [`parse_duration`] as a [`Duration`].
pub fn duration(text: &str) -> Option<Duration> {
    parse_duration(text).map(Duration::from_secs)
}

/// The canonical rendering: exact days, hours, minutes, else seconds.
pub fn fmt_duration(secs: u64) -> String {
    if secs % 86400 == 0 {
        return format!("{}d", secs / 86400);
    }
    if secs % 3600 == 0 {
        return format!("{}h", secs / 3600);
    }
    if secs % 60 == 0 {
        return format!("{}m", secs / 60);
    }
    format!("{secs}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("30s"), Some(30));
        assert_eq!(parse_duration("15m"), Some(900));
        assert_eq!(parse_duration("24h"), Some(86400));
        assert_eq!(parse_duration("7d"), Some(604800));
        assert_eq!(parse_duration("45"), Some(45));
        assert_eq!(parse_duration("  5m "), Some(300));
        assert_eq!(parse_duration("5 s"), Some(5));
    }

    #[test]
    fn durations_rejected() {
        assert_eq!(parse_duration("0s"), None);
        assert_eq!(parse_duration("0"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("x"), None);
        assert_eq!(parse_duration("5x"), None);
        assert_eq!(parse_duration("-5s"), None);
        assert_eq!(parse_duration("1.5h"), None);
        assert_eq!(parse_duration("99999999999999999999d"), None);
    }

    #[test]
    fn durations_format() {
        assert_eq!(fmt_duration(45), "45s");
        assert_eq!(fmt_duration(120), "2m");
        assert_eq!(fmt_duration(3600), "1h");
        assert_eq!(fmt_duration(86400), "1d");
        assert_eq!(fmt_duration(90), "90s");
    }

    #[test]
    fn full_core_section() {
        let (config, warns) = parse_str(
            "# leading comment\n\
             [core]\n\
             log_level = \"debug\" # trailing comment\n\
             state_dir = \"/tmp/s\"\n\
             log_dir = \"/tmp/l\"\n\
             port = 18924\n",
        )
        .unwrap();
        assert!(warns.is_empty());
        assert_eq!(config.log_level, Level::Debug);
        assert_eq!(config.state_dir, Some(PathBuf::from("/tmp/s")));
        assert_eq!(config.log_dir, Some(PathBuf::from("/tmp/l")));
        assert_eq!(config.port, Some(18924));
    }

    #[test]
    fn defaults_when_bare() {
        let (config, warns) = parse_str("[core]\n").unwrap();
        assert!(warns.is_empty());
        assert_eq!(config, CoreConfig::default());
    }

    #[test]
    fn unknown_keys_and_sections_warn() {
        let (config, warns) =
            parse_str("[core]\nfuture = 1\n\n[agent]\nkey = \"x\"\n[agent]\n").unwrap();
        assert_eq!(config, CoreConfig::default());
        assert_eq!(warns.len(), 2);
        assert!(warns[0].ends_with("2: unknown key \"future\", ignored"));
        assert!(warns[1].ends_with("4: unknown section [agent], ignored"));
    }

    #[test]
    fn duplicate_key_warns_and_last_wins() {
        let (config, warns) = parse_str("[core]\nport = 1\nport = 2\n").unwrap();
        assert_eq!(config.port, Some(2));
        assert_eq!(warns.len(), 1);
        assert!(warns[0].ends_with("3: duplicate port, last wins"));
    }

    #[test]
    fn failures_carry_line_numbers() {
        assert_eq!(
            parse_str("port = 1\n").unwrap_err(),
            "config:1: key outside any section"
        );
        assert_eq!(
            parse_str("[core]\nlog_level = \"loud\"\n").unwrap_err(),
            "config:2: log_level is one of: error warn info debug (got \"loud\")"
        );
        assert_eq!(
            parse_str("[core]\nport = 0\n").unwrap_err(),
            "config:2: port is 1-65535 (got \"0\")"
        );
        assert_eq!(
            parse_str("[core]\nport = \"18924\"\n").unwrap_err(),
            "config:2: port is 1-65535 (got \"\\\"18924\\\"\")"
        );
        assert_eq!(
            parse_str("[core\n").unwrap_err(),
            "config:1: bad section header"
        );
        assert_eq!(
            parse_str("[core]\nlog_level = debug\n").unwrap_err(),
            "config:2: expected a \"quoted string\""
        );
    }

    #[test]
    fn hash_inside_strings_survives() {
        let (config, _) = parse_str("[core]\nstate_dir = \"/tmp/a#b\"\n").unwrap();
        assert_eq!(config.state_dir, Some(PathBuf::from("/tmp/a#b")));
    }

    #[test]
    fn missing_file_means_defaults_with_warning() {
        let dir = crate::testutil::scratch("config-missing");
        let (config, warns) = load(&dir.join("nope.toml")).unwrap();
        assert_eq!(config, CoreConfig::default());
        assert_eq!(warns.len(), 1);
        assert!(warns[0].ends_with("nope.toml: file not found, using defaults"));
    }
}
