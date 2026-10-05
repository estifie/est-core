//! Layered directory resolution.
//!
//! Explicit environment first, then the installation at hand, then shared
//! pointers — the same order everywhere, so every product finds its files
//! the same way.

use std::path::{Path, PathBuf};

/// Resolve the product home: an explicit override wins, then a binary
/// sitting in a `bin/` directory resolves one level up, otherwise the
/// current directory. Pure, so tests pin the order without touching the
/// environment.
#[must_use]
pub fn resolve_base(explicit: Option<&str>, exe_file: Option<&Path>, cwd: &Path) -> PathBuf {
    if let Some(dir) = explicit.map(str::trim).filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(bin) = exe_file.and_then(|e| e.parent())
        && bin.file_name().is_some_and(|n| n == "bin")
        && let Some(base) = bin.parent()
    {
        return base.to_path_buf();
    }
    cwd.to_path_buf()
}

/// [`resolve_base`] wired to the real process: `env_var`, the current
/// executable (symlinks resolved), and the current directory.
#[must_use]
pub fn standard_base(env_var: &str) -> PathBuf {
    let explicit = std::env::var(env_var).ok();
    let exe = std::env::current_exe()
        .ok()
        .map(|e| std::fs::canonicalize(&e).unwrap_or(e));
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    resolve_base(explicit.as_deref(), exe.as_deref(), &cwd)
}

/// `$HOME/.config/est/<product>`: the shared config root. None when
/// `HOME` is missing or empty.
#[must_use]
pub fn product_config_dir(product: &str) -> Option<PathBuf> {
    product_config_dir_in(std::env::var("HOME").ok().as_deref(), product)
}

/// [`product_config_dir`] over an explicit home: pure, so tests pin the
/// shape without touching the environment.
#[must_use]
pub fn product_config_dir_in(home: Option<&str>, product: &str) -> Option<PathBuf> {
    let home = home.filter(|h| !h.trim().is_empty())?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("est")
            .join(product),
    )
}

/// First non-empty line of a pointer file (`~/.config/est/vault-home`
/// style). Missing or unreadable files read as absent, never as errors.
#[must_use]
pub fn pointer_target(path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(path).ok()?;
    let first = text.lines().next().map(str::trim).unwrap_or("");
    (!first.is_empty()).then(|| PathBuf::from(first))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("est-core-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn explicit_wins() {
        let cwd = Path::new("/cwd");
        let exe = Path::new("/opt/est/bin/est");
        assert_eq!(
            resolve_base(Some("/pinned"), Some(exe), cwd),
            PathBuf::from("/pinned")
        );
    }

    #[test]
    fn blank_explicit_falls_through() {
        let cwd = Path::new("/cwd");
        let exe = Path::new("/opt/est/bin/est");
        assert_eq!(
            resolve_base(Some("   "), Some(exe), cwd),
            PathBuf::from("/opt/est")
        );
    }

    #[test]
    fn bin_layout_resolves_one_up() {
        let cwd = Path::new("/cwd");
        assert_eq!(
            resolve_base(None, Some(Path::new("/opt/est/bin/est")), cwd),
            PathBuf::from("/opt/est")
        );
    }

    #[test]
    fn exe_outside_bin_falls_to_cwd() {
        let cwd = Path::new("/cwd");
        assert_eq!(
            resolve_base(None, Some(Path::new("/usr/local/est")), cwd),
            PathBuf::from("/cwd")
        );
        assert_eq!(resolve_base(None, None, cwd), PathBuf::from("/cwd"));
    }

    #[test]
    fn config_dir_shape() {
        assert_eq!(
            product_config_dir_in(Some("/tmp/est-core-test-home"), "vault"),
            Some(PathBuf::from("/tmp/est-core-test-home/.config/est/vault"))
        );
    }

    #[test]
    fn config_dir_absent_without_home() {
        assert_eq!(product_config_dir_in(None, "vault"), None);
        assert_eq!(product_config_dir_in(Some("  "), "vault"), None);
    }

    #[test]
    fn pointer_reads_first_line() {
        let dir = scratch("pointer");
        let file = dir.join("vault-home");
        std::fs::write(&file, "/data/vault\nignored\n").unwrap();
        let target = pointer_target(&file);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(target, Some(PathBuf::from("/data/vault")));
    }

    #[test]
    fn pointer_absent_when_missing_or_blank() {
        let dir = scratch("pointer-absent");
        assert_eq!(pointer_target(&dir.join("nope")), None);
        let blank = dir.join("blank");
        std::fs::write(&blank, "  \n").unwrap();
        let target = pointer_target(&blank);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(target, None);
    }
}
