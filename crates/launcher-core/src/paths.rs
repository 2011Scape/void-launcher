use std::io;
use std::path::{Path, PathBuf};

use crate::error::CoreError;

/// Every location the launcher reads or writes.
///
/// `root` is `~/.void`. The game client keeps its own files elsewhere
/// (`~/.jagex_cache_32` for the cache, `~/.634hd-settings.txt`,
/// `~/.lwjgl/634-client` for natives) and the launcher never touches them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoidPaths {
    /// The user's home directory (what Java's `user.home` resolves to).
    pub home: PathBuf,
    /// `~/.void` unless overridden with `VOID_HOME`.
    pub root: PathBuf,
}

impl VoidPaths {
    pub fn resolve() -> Result<Self, CoreError> {
        let home = dirs::home_dir().ok_or(CoreError::NoHomeDir)?;
        let root = match std::env::var_os("VOID_HOME") {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => home.join(".void"),
        };
        Ok(Self { home, root })
    }

    /// Paths rooted at an arbitrary directory (tests, end-to-end runs).
    pub fn with_home(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        let root = home.join(".void");
        Self { home, root }
    }

    pub fn client_dir(&self) -> PathBuf {
        self.root.join("client")
    }
    /// Where a jar is looked for when `client.json` names none: a player can
    /// drop a `runetek5-634.jar` here by hand and Play will use it.
    pub fn client_jar(&self) -> PathBuf {
        self.client_dir().join("runetek5-634.jar")
    }
    pub fn client_json(&self) -> PathBuf {
        self.client_dir().join("client.json")
    }
    pub fn runtime_dir(&self) -> PathBuf {
        self.root.join("runtime")
    }
    pub fn runtime_json(&self) -> PathBuf {
        self.runtime_dir().join("runtime.json")
    }
    pub fn launcher_json(&self) -> PathBuf {
        self.root.join("launcher.json")
    }
    pub fn java_cache_json(&self) -> PathBuf {
        self.root.join("java-cache.json")
    }
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn launcher_log(&self) -> PathBuf {
        self.logs_dir().join("launcher.log")
    }
    pub fn tmp_dir(&self) -> PathBuf {
        self.root.join(".launcher-tmp")
    }
    /// The client's game cache. Owned by the client; never modified by the launcher.
    pub fn game_cache_dir(&self) -> PathBuf {
        self.home.join(".jagex_cache_32")
    }
    /// Where the client loads plugin jars from (`~/.634-client/plugins`, derived
    /// from Java's `user.home`, so it follows `home`, not `root`). Owned by the
    /// client; the launcher only opens it for the player.
    pub fn plugins_dir(&self) -> PathBuf {
        plugins_dir_under(&self.home)
    }

    /// Creates the launcher-owned directories. Idempotent.
    pub fn ensure_dirs(&self) -> io::Result<()> {
        for dir in [self.client_dir(), self.runtime_dir(), self.logs_dir(), self.tmp_dir()] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

/// The client's default plugin folder for a given Java `user.home`
/// (`PluginPaths.kt`: `<home>/.634-client/plugins`).
pub fn plugins_dir_under(home: &Path) -> PathBuf {
    home.join(".634-client").join("plugins")
}

/// Opens a folder in the platform file manager, creating it first.
pub fn open_folder(dir: &Path) -> Result<(), CoreError> {
    std::fs::create_dir_all(dir).map_err(|e| CoreError::io(format!("creating {}", dir.display()), e))?;
    open::that_detached(dir).map_err(|e| CoreError::io(format!("opening {}", dir.display()), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_rooted_at_dot_void() {
        let p = VoidPaths::with_home("/home/u");
        assert_eq!(p.root, PathBuf::from("/home/u/.void"));
        assert_eq!(p.client_jar(), PathBuf::from("/home/u/.void/client/runetek5-634.jar"));
        assert_eq!(p.launcher_json(), PathBuf::from("/home/u/.void/launcher.json"));
        assert_eq!(p.game_cache_dir(), PathBuf::from("/home/u/.jagex_cache_32"));
        assert_eq!(p.plugins_dir(), PathBuf::from("/home/u/.634-client/plugins"));
    }

    /// The client builds the same path from Java's `user.home`, which is the
    /// profile folder on Windows (`FOLDERID_Profile`, what `dirs::home_dir` returns).
    #[cfg(windows)]
    #[test]
    fn plugins_dir_follows_the_windows_profile() {
        let p = VoidPaths::with_home(r"C:\Users\u");
        assert_eq!(p.plugins_dir(), PathBuf::from(r"C:\Users\u\.634-client\plugins"));
        assert_eq!(p.root, PathBuf::from(r"C:\Users\u\.void"));
    }

    #[test]
    fn plugins_dir_is_under_home_not_void_home() {
        let p = VoidPaths { home: PathBuf::from("h"), root: PathBuf::from("elsewhere") };
        assert_eq!(p.plugins_dir(), PathBuf::from("h").join(".634-client").join("plugins"));
    }
}
