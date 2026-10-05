//! Durable files: atomic writes, mode-correct dirs, locks, retention.
//!
//! Unix-only in v1 (file modes); see the README platform note.

use std::collections::HashSet;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Write `bytes` to `path` atomically (tmp sibling + rename) with `mode`,
/// creating parents. Readers never see a half-written file.
pub fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    use std::io::Write as _;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(format!(".tmp.{}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(&tmp)?;
    file.write_all(bytes)?;
    drop(file);
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// `create_dir_all` where every created directory gets `mode`. Existing
/// directories keep theirs; races on creation are fine.
pub fn create_dir_all_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        create_dir_all_mode(parent, mode)?;
    }
    match std::fs::DirBuilder::new().mode(mode).create(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

/// Mutual exclusion through an atomically created lockfile. The file holds
/// the owner's pid and is removed on drop.
///
/// A crashed holder leaves its file behind; `stale_after` bounds the
/// damage: a lockfile older than that is taken over (the takeover still
/// races on atomic create, so two takers cannot both win). Choose
/// `stale_after` well past the critical section, or a live holder can be
/// robbed. Pass zero to always take over (the test hook).
#[derive(Debug)]
pub struct FileLock {
    path: PathBuf,
}

impl FileLock {
    /// Acquire the lock at `path`, waiting up to `timeout`. Parents are
    /// created. Times out with [`std::io::ErrorKind::TimedOut`].
    pub fn acquire(path: &Path, timeout: Duration, stale_after: Duration) -> std::io::Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let start = Instant::now();
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(mut file) => {
                    use std::io::Write as _;
                    let _ = writeln!(file, "{}", std::process::id());
                    return Ok(FileLock {
                        path: path.to_path_buf(),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if is_stale(path, stale_after) {
                        let _ = std::fs::remove_file(path);
                        continue;
                    }
                    if start.elapsed() >= timeout {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            format!("lock held: {}", path.display()),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Where the lockfile lives.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn is_stale(path: &Path, stale_after: Duration) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    modified
        .elapsed()
        .map(|age| age > stale_after)
        .unwrap_or(false)
}

/// What one [`prune`] pass removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PruneStats {
    /// Files deleted.
    pub removed: usize,
    /// Bytes those files held.
    pub freed_bytes: u64,
}

/// Retention for a log-style directory: files ending in `suffix` whose
/// name starts with `own_prefix` keep their newest `keep` by name (stamps
/// must sort chronologically, `name-<stamp>.log` style), then the whole
/// directory is squeezed under `max_bytes`, oldest mtime first. Missing
/// directories prune to zero; per-file failures are skipped, never fatal.
pub fn prune(
    dir: &Path,
    suffix: &str,
    own_prefix: &str,
    keep: usize,
    max_bytes: u64,
) -> PruneStats {
    let mut stats = PruneStats::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return stats;
    };
    let mut mine: Vec<PathBuf> = Vec::new();
    let mut all: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !path.is_file() || !name.ends_with(suffix) {
            continue;
        }
        let meta = entry.metadata().ok();
        if name.starts_with(own_prefix) {
            mine.push(path.clone());
        }
        all.push((
            meta.as_ref()
                .and_then(|m| m.modified().ok())
                .unwrap_or(UNIX_EPOCH),
            meta.map(|m| m.len()).unwrap_or(0),
            path,
        ));
    }
    mine.sort();
    let mut gone = HashSet::new();
    while mine.len() > keep {
        let oldest = mine.remove(0);
        if let Ok(len) = removed_len(&oldest) {
            gone.insert(oldest);
            stats.removed += 1;
            stats.freed_bytes += len;
        }
    }
    all.retain(|(_, _, path)| !gone.contains(path));
    all.sort();
    let mut total: u64 = all.iter().map(|(_, size, _)| size).sum();
    for (_, size, path) in all {
        if total <= max_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
            stats.removed += 1;
            stats.freed_bytes += size;
        }
    }
    stats
}

fn removed_len(path: &Path) -> std::io::Result<u64> {
    let len = std::fs::metadata(path).map(|m| m.len())?;
    std::fs::remove_file(path)?;
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::scratch;
    use std::os::unix::fs::PermissionsExt;

    fn mode_of(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn atomic_write_roundtrip_with_mode() {
        let dir = scratch("store-write");
        let file = dir.join("sub").join("secret.bin");
        write_atomic(&file, b"one", 0o600).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"one");
        assert_eq!(mode_of(&file), 0o600);
        write_atomic(&file, b"two", 0o600).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"two");
        assert_eq!(mode_of(&file), 0o600);
    }

    #[test]
    fn mode_dirs_all_the_way_down() {
        let dir = scratch("store-dirs");
        let deep = dir.join("a").join("b").join("c");
        create_dir_all_mode(&deep, 0o700).unwrap();
        assert_eq!(mode_of(&dir.join("a")), 0o700);
        assert_eq!(mode_of(&deep), 0o700);
        create_dir_all_mode(&deep, 0o700).unwrap();
    }

    #[test]
    fn lock_releases_on_drop() {
        let dir = scratch("store-lock");
        let path = dir.join("job.lock");
        {
            let guard =
                FileLock::acquire(&path, Duration::from_secs(5), Duration::from_secs(60)).unwrap();
            assert_eq!(guard.path(), path.as_path());
            assert!(path.is_file());
        }
        assert!(!path.exists());
    }

    #[test]
    fn lock_times_out_against_a_holder() {
        let dir = scratch("store-lock-timeout");
        let path = dir.join("job.lock");
        let _held =
            FileLock::acquire(&path, Duration::from_secs(5), Duration::from_secs(60)).unwrap();
        let err = FileLock::acquire(&path, Duration::from_millis(200), Duration::from_secs(60))
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    }

    #[test]
    fn lock_waits_its_turn() {
        let dir = scratch("store-lock-wait");
        let path = dir.join("job.lock");
        let held =
            FileLock::acquire(&path, Duration::from_secs(5), Duration::from_secs(60)).unwrap();
        let probe = path.clone();
        let waiter = std::thread::spawn(move || {
            FileLock::acquire(&probe, Duration::from_secs(10), Duration::from_secs(60)).unwrap()
        });
        std::thread::sleep(Duration::from_millis(200));
        drop(held);
        let guard = waiter.join().unwrap();
        assert_eq!(guard.path(), path.as_path());
    }

    #[test]
    fn stale_lock_is_taken_over() {
        let dir = scratch("store-lock-stale");
        let path = dir.join("job.lock");
        std::fs::write(&path, "424242\n").unwrap();
        let _guard = FileLock::acquire(&path, Duration::from_secs(5), Duration::ZERO).unwrap();
    }

    #[test]
    fn prune_keeps_newest_by_name() {
        let dir = scratch("store-prune-keep");
        for n in 1..=5 {
            std::fs::write(dir.join(format!("job-2026010{n}.log")), vec![b'x'; 100]).unwrap();
        }
        let stats = prune(&dir, ".log", "job-", 2, u64::MAX);
        assert_eq!(
            stats,
            PruneStats {
                removed: 3,
                freed_bytes: 300
            }
        );
        assert!(!dir.join("job-20260101.log").exists());
        assert!(dir.join("job-20260104.log").exists());
        assert!(dir.join("job-20260105.log").exists());
    }

    #[test]
    fn prune_squeezes_under_budget() {
        let dir = scratch("store-prune-budget");
        for n in 1..=5 {
            std::fs::write(dir.join(format!("run-2026010{n}.log")), vec![b'x'; 100]).unwrap();
        }
        let stats = prune(&dir, ".log", "nomatch-", usize::MAX, 250);
        assert_eq!(stats.removed, 3);
        assert_eq!(stats.freed_bytes, 300);
        let left: u64 = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();
        assert!(left <= 250);
    }

    #[test]
    fn prune_missing_dir_is_zero() {
        let dir = scratch("store-prune-missing");
        assert_eq!(
            prune(&dir.join("nope"), ".log", "job-", 2, 100),
            PruneStats::default()
        );
    }
}
