//! The EST Hub's address, resolved one way everywhere.
//!
//! Precedence: an explicit `--hub` value, then the [`HUB_URL_ENV`]
//! environment variable, then the compiled-in default. Empty values
//! never win — they fall through to the next source. The default is
//! the hub on the tailnet (see `docs/ports.md`); a binary that manages
//! a hub on this machine passes its own local default to
//! [`resolve_hub_url_with_default`].

/// The hub on the tailnet: what every fleet client talks to unless
/// told otherwise.
pub const DEFAULT_HUB_URL: &str = "http://100.120.126.23:18925";

/// Environment variable carrying a hub base URL override.
pub const HUB_URL_ENV: &str = "EST_HUB_URL";

/// Resolve the hub base URL with a caller-supplied default.
///
/// `explicit` is the `--hub` flag; the default should already be
/// normalized (no trailing slash).
#[must_use]
pub fn resolve_hub_url_with_default(explicit: Option<&str>, default: &str) -> String {
    let env = std::env::var(HUB_URL_ENV).ok();
    resolve_with(explicit, env.as_deref(), default)
}

/// Resolve the hub base URL against the shared fleet default.
#[must_use]
pub fn resolve_hub_url(explicit: Option<&str>) -> String {
    resolve_hub_url_with_default(explicit, DEFAULT_HUB_URL)
}

/// The precedence as a pure function: explicit, then env, then the
/// default. Empty values fall through; winners are trimmed and lose
/// their trailing slashes.
fn resolve_with(explicit: Option<&str>, env: Option<&str>, default: &str) -> String {
    if let Some(url) = explicit
        && !url.trim().is_empty()
    {
        return url.trim().trim_end_matches('/').to_string();
    }
    if let Some(url) = env
        && !url.trim().is_empty()
    {
        return url.trim().trim_end_matches('/').to_string();
    }
    default.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_wins_and_normalizes() {
        assert_eq!(
            resolve_with(Some("http://custom:1234/"), None, DEFAULT_HUB_URL),
            "http://custom:1234"
        );
        assert_eq!(
            resolve_with(Some("  http://custom:1234//  "), None, DEFAULT_HUB_URL),
            "http://custom:1234"
        );
    }

    #[test]
    fn empty_explicit_falls_through_to_the_default() {
        assert_eq!(resolve_with(None, None, DEFAULT_HUB_URL), DEFAULT_HUB_URL);
        assert_eq!(
            resolve_with(Some(""), None, DEFAULT_HUB_URL),
            DEFAULT_HUB_URL
        );
        assert_eq!(
            resolve_with(Some("   "), None, DEFAULT_HUB_URL),
            DEFAULT_HUB_URL
        );
    }

    #[test]
    fn env_beats_the_default_but_loses_to_explicit() {
        let env = Some("http://from-env:18925/");
        assert_eq!(
            resolve_with(None, env, DEFAULT_HUB_URL),
            "http://from-env:18925"
        );
        assert_eq!(
            resolve_with(Some("http://flag:18925"), env, DEFAULT_HUB_URL),
            "http://flag:18925"
        );
    }

    #[test]
    fn empty_env_falls_through_to_the_default() {
        assert_eq!(
            resolve_with(None, Some("   "), DEFAULT_HUB_URL),
            DEFAULT_HUB_URL
        );
    }

    #[test]
    fn caller_default_is_used_verbatim() {
        assert_eq!(
            resolve_with(None, None, "http://127.0.0.1:18925"),
            "http://127.0.0.1:18925"
        );
    }
}
