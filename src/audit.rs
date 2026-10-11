//! The EST audit-event contract, as code.
//!
//! One timeline for the whole ecosystem: health flips, approvals,
//! device changes, balance moves, secret pushes, deploys — every
//! reporter emits the same shape and the hub stores it once.
//!
//! This module is dependency-free by policy (std only): `est-health`,
//! `est-vault`, `est-balance`, `est-cli`, and the bridge validate and
//! render locally before ever touching the network. The hub mirrors
//! these exact rules in its own `model` (it pins `est-core` by tag,
//! so the rules are copied, not imported — keep them in sync).
//!
//! Wire shape (JSON):
//! `{"kind":"health.flip","actor":"mac","subject":"site",
//!   "summary":"🔴 site is DOWN — refused","severity":"critical",
//!   "data":""}`
//! `data` is an opaque string (conventionally a JSON object, `""`
//! when none) so reporters never need a schema upgrade to add fields.

/// Contract version for the event shape.
pub const EVENT_V: u32 = 1;

/// System kinds the hub emits itself. Reporters SHOULD reuse these
/// when they mean the same thing, and MAY emit `group.name` kinds of
/// their own (e.g. `deploy.finished`, `vault.rotated`, `health.sync`).
pub const KIND_HEALTH_FLIP: &str = "health.flip";
/// A check definition was created.
pub const KIND_CHECK_ADD: &str = "check.add";
/// A check definition was replaced.
pub const KIND_CHECK_SET: &str = "check.set";
/// A check definition was deleted.
pub const KIND_CHECK_DELETE: &str = "check.delete";
/// A check's down page was quieted.
pub const KIND_CHECK_ACK: &str = "check.ack";
/// A check was muted both directions.
pub const KIND_CHECK_MUTE: &str = "check.mute";
/// A silence was lifted.
pub const KIND_CHECK_UNSILENCE: &str = "check.unsilence";
/// An approval was requested.
pub const KIND_APPROVAL_REQUESTED: &str = "approval.requested";
/// An approval was decided.
pub const KIND_APPROVAL_DECIDED: &str = "approval.decided";
/// An approval was withdrawn.
pub const KIND_APPROVAL_CANCELLED: &str = "approval.cancelled";
/// A device registered.
pub const KIND_DEVICE_ADD: &str = "device.add";
/// A device's push state changed.
pub const KIND_DEVICE_SET: &str = "device.set";
/// A device was revoked.
pub const KIND_DEVICE_REVOKE: &str = "device.revoke";
/// A balance row was written.
pub const KIND_BALANCE_SET: &str = "balance.set";
/// A balance row was deleted.
pub const KIND_BALANCE_DELETE: &str = "balance.delete";
/// A secret blob was pushed.
pub const KIND_SECRET_PUSH: &str = "secret.push";
/// A secret was deleted.
pub const KIND_SECRET_DELETE: &str = "secret.delete";
/// A pull token was minted or revoked.
pub const KIND_SECRET_TOKEN: &str = "secret.token";
/// A broadcast was queued.
pub const KIND_NOTIFY_SEND: &str = "notify.send";
/// A project was synced from a reporter (`est-health sync`).
pub const KIND_HEALTH_SYNC: &str = "health.sync";
/// A deploy finished somewhere (`est link send --topic deploy.finished` shape).
pub const KIND_DEPLOY_FINISHED: &str = "deploy.finished";

/// Severities: `info` (normal), `warn` (needs a look), `critical` (page).
pub const SEVERITIES: &[&str] = &["info", "warn", "critical"];

/// Limits, mirroring the hub's storage caps.
pub const KIND_MAX: usize = 64;
/// Actor names cap at 64 chars.
pub const ACTOR_MAX: usize = 64;
/// Subjects cap at 128 chars.
pub const SUBJECT_MAX: usize = 128;
/// Summaries cap at 512 chars.
pub const SUMMARY_MAX: usize = 512;
/// Opaque data caps at 4096 chars.
pub const DATA_MAX: usize = 4096;

/// True when `kind` is `group.name` segments of `[a-z0-9_-]`, 1–64 chars.
/// Examples: `health.flip`, `deploy.finished`, `vault.rotated`.
#[must_use]
pub fn valid_kind(s: &str) -> bool {
    if s.is_empty() || s.len() > KIND_MAX {
        return false;
    }
    if s.starts_with('.') || s.ends_with('.') || s.contains("..") {
        return false;
    }
    let mut seg_len = 0;
    for c in s.chars() {
        if c == '.' {
            if seg_len == 0 || seg_len > 32 {
                return false;
            }
            seg_len = 0;
        } else if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' {
            seg_len += 1;
            if seg_len > 32 {
                return false;
            }
        } else {
            return false;
        }
    }
    seg_len > 0
}

/// True when `severity` is one of `info`, `warn`, `critical`.
#[must_use]
pub fn valid_severity(s: &str) -> bool {
    SEVERITIES.contains(&s)
}

/// True when `actor` names who did it: 1–64 chars, no control chars.
#[must_use]
pub fn valid_actor(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.len() <= ACTOR_MAX && !t.chars().any(|c| c.is_control())
}

/// True when `subject` is 0–128 chars with no control chars.
/// Empty means "no subject" (fleet-wide events).
#[must_use]
pub fn valid_subject(s: &str) -> bool {
    s.len() <= SUBJECT_MAX && !s.chars().any(|c| c.is_control())
}

/// True when `summary` is the human line: 1–512 chars, no control chars.
#[must_use]
pub fn valid_summary(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.len() <= SUMMARY_MAX && !t.chars().any(|c| c.is_control())
}

/// True when `data` is 0–4096 chars (opaque; conventionally JSON).
#[must_use]
pub fn valid_data(s: &str) -> bool {
    s.len() <= DATA_MAX && !s.contains('\0')
}

/// Every rule in one place, or the first refusal as a human line.
/// Empty `actor` reads `"hub"`; empty `severity` reads `"info"`.
/// Returns the normalized `(kind, actor, subject, summary, severity, data)`.
pub fn validate_event(
    kind: &str,
    actor: &str,
    subject: &str,
    summary: &str,
    severity: &str,
    data: &str,
) -> Result<(String, String, String, String, String, String), String> {
    let kind = kind.trim();
    if !valid_kind(kind) {
        return Err("kind is group.name segments of [a-z0-9_-] (1-64 chars)".to_string());
    }
    let actor = if actor.trim().is_empty() {
        "hub".to_string()
    } else {
        if !valid_actor(actor) {
            return Err("actor is 1-64 chars, no control characters".to_string());
        }
        actor.trim().to_string()
    };
    if !valid_subject(subject) {
        return Err("subject is 128 max, no control characters".to_string());
    }
    if !valid_summary(summary) {
        return Err("summary is the human line (1-512 chars)".to_string());
    }
    let severity = if severity.trim().is_empty() {
        "info".to_string()
    } else {
        if !valid_severity(severity.trim()) {
            return Err("severity is info, warn, or critical".to_string());
        }
        severity.trim().to_string()
    };
    if !valid_data(data) {
        return Err("data is 4096 max".to_string());
    }
    Ok((
        kind.to_string(),
        actor,
        subject.to_string(),
        summary.trim().to_string(),
        severity,
        data.to_string(),
    ))
}

/// One timeline row for text output: `severity KIND subject — summary`.
/// Empty subject reads as fleet-wide (no `—` split).
#[must_use]
pub fn event_line(kind: &str, severity: &str, subject: &str, summary: &str) -> String {
    let mark = match severity {
        "critical" => "🔴",
        "warn" => "⚠",
        _ => "·",
    };
    if subject.trim().is_empty() {
        format!("{mark} [{severity}] {kind} {summary}")
    } else {
        format!("{mark} [{severity}] {kind} {} — {summary}", subject.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_dotted_lowercase() {
        assert!(valid_kind("health.flip"));
        assert!(valid_kind("deploy.finished"));
        assert!(valid_kind("vault.rotated"));
        assert!(valid_kind("a"));
        assert!(!valid_kind(""));
        assert!(!valid_kind("Health.flip"));
        assert!(!valid_kind("health flip"));
        assert!(!valid_kind(".health"));
        assert!(!valid_kind("health."));
        assert!(!valid_kind("health..flip"));
        assert!(!valid_kind("a".repeat(65).as_str()));
    }

    #[test]
    fn severities_are_three() {
        assert!(valid_severity("info"));
        assert!(valid_severity("warn"));
        assert!(valid_severity("critical"));
        assert!(!valid_severity("low"));
        assert!(!valid_severity("INFO"));
    }

    #[test]
    fn validate_fills_actor_and_severity() {
        let (k, a, s, sum, sev, d) =
            validate_event("health.flip", "", "site", "down", "", "").unwrap();
        assert_eq!(
            (k.as_str(), a.as_str(), sev.as_str()),
            ("health.flip", "hub", "info")
        );
        assert_eq!(s, "site");
        assert_eq!(sum, "down");
        assert_eq!(d, "");
    }

    #[test]
    fn validate_refuses_in_order() {
        assert!(validate_event("NOPE", "mac", "", "x", "info", "").is_err());
        assert!(validate_event("health.flip", "mac", "", "", "info", "").is_err());
        assert!(validate_event("health.flip", "mac", "", "x", "LOUD", "").is_err());
    }

    #[test]
    fn lines_mark_severity() {
        assert!(event_line("health.flip", "critical", "site", "down").starts_with("🔴"));
        assert!(event_line("deploy.finished", "info", "", "ok").contains("deploy.finished ok"));
    }
}
