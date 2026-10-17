//! Small filesystem helpers: atomic writes, executable bits, quarantine.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use crate::error::{CoreError, IoContext};

/// Writes `bytes` to `path` via a sibling temp file and an atomic rename.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    let parent = path.parent().ok_or_else(|| CoreError::other("path has no parent"))?;
    std::fs::create_dir_all(parent).context(format!("creating {}", parent.display()))?;
    let tmp = path.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&tmp).context(format!("creating {}", tmp.display()))?;
        file.write_all(bytes).context(format!("writing {}", tmp.display()))?;
        file.sync_all().context(format!("syncing {}", tmp.display()))?;
    }
    rename_replace(&tmp, path)
}

pub fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| CoreError::other(format!("serialising {}: {e}", path.display())))?;
    atomic_write(path, &json)
}

/// Renames `from` over `to`, retrying briefly on Windows where antivirus or
/// a closing process can hold a transient lock.
pub fn rename_replace(from: &Path, to: &Path) -> Result<(), CoreError> {
    let mut attempt = 0;
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if cfg!(windows) && attempt < 5 => {
                attempt += 1;
                tracing::debug!("rename {} -> {} failed ({e}); retry {attempt}", from.display(), to.display());
                std::thread::sleep(Duration::from_millis(400));
            }
            Err(e) => return Err(CoreError::io(format!("moving {} to {}", from.display(), to.display()), e)),
        }
    }
}

/// Marks a file executable on Unix; no-op elsewhere.
pub fn set_executable(path: &Path) -> Result<(), CoreError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).context(format!("stat {}", path.display()))?.permissions();
        perms.set_mode(perms.mode() | 0o755);
        std::fs::set_permissions(path, perms).context(format!("chmod {}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Strips the macOS quarantine attribute recursively. Best effort.
pub fn remove_quarantine(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/xattr").args(["-dr", "com.apple.quarantine"]).arg(path).output();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
    }
}

/// Deletes a file if it exists, ignoring errors (used for cleanup paths).
pub fn remove_file_quiet(path: &Path) {
    if let Err(e) = std::fs::remove_file(path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::debug!("could not remove {}: {e}", path.display());
    }
}

pub fn remove_dir_quiet(path: &Path) {
    if let Err(e) = std::fs::remove_dir_all(path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::debug!("could not remove {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("a.json");
        atomic_write(&target, b"{}").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"{}");
        assert!(!target.with_extension("tmp").exists());
    }
}
