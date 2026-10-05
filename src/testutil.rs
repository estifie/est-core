//! Test-only scratch directories. Never compiled into the lib.

use std::path::PathBuf;

/// A fresh empty directory under the system temp dir, unique per process
/// and `name`. Cleaned before creation, so reruns start empty.
pub(crate) fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("est-core-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}
