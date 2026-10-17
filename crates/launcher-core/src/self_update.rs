//! Keeping the launcher itself current from its own GitHub releases.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sipper::{Sipper, Straw};

use crate::LAUNCHER_REPO;
use crate::error::{CoreError, IoContext};
use crate::fsutil::{remove_dir_quiet, remove_file_quiet, remove_quarantine, rename_replace, set_executable};
use crate::github;
use crate::http::Http;
use crate::progress::Progress;
use crate::version::{Channel, LauncherBuild};

pub const MANIFEST_NAME: &str = "manifest.json";
pub const SUPPORTED_SCHEMA: u32 = 1;
/// Set on relaunch after an update so the next start skips one check.
pub const UPGRADED_ENV: &str = "VOID_LAUNCHER_UPGRADED";

/// `manifest.json` published next to every launcher build.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Manifest {
    pub schema: u32,
    pub version: String,
    pub channel: Channel,
    pub commit: String,
    pub built_at: String,
    pub min_launcher_version: Option<String>,
    pub notes_url: Option<String>,
    pub files: BTreeMap<String, ManifestFile>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            schema: 0,
            version: String::new(),
            channel: Channel::Stable,
            commit: String::new(),
            built_at: String::new(),
            min_launcher_version: None,
            notes_url: None,
            files: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ManifestFile {
    pub name: String,
    pub sha256: String,
    pub size: u64,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LauncherUpdate {
    pub version: String,
    pub commit: String,
    pub channel: Channel,
    pub url: String,
    pub file: ManifestFile,
    pub notes_url: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    UpToDate,
    Available(LauncherUpdate),
    /// The manifest is newer than this launcher understands; send the user to the release page.
    ManualUpdateRequired {
        version: String,
        url: String,
    },
    NoBuildForPlatform,
    /// Installed through a package manager; updates come from there.
    Disabled(String),
    Offline(String),
    Skipped,
}

/// How this binary was installed, which decides how it can replace itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallKind {
    PlainBinary(PathBuf),
    WindowsExe(PathBuf),
    MacAppBundle { bundle: PathBuf, exe: PathBuf },
    AppImage(PathBuf),
    PackageManaged(String),
}

pub fn detect_install_kind() -> InstallKind {
    let exe = std::env::current_exe().unwrap_or_default();
    if std::env::var_os("FLATPAK_ID").is_some() {
        return InstallKind::PackageManaged("Flatpak".into());
    }
    if std::env::var_os("SNAP").is_some() {
        return InstallKind::PackageManaged("Snap".into());
    }
    if let Some(appimage) = std::env::var_os("APPIMAGE").filter(|v| !v.is_empty()) {
        return InstallKind::AppImage(PathBuf::from(appimage));
    }
    if cfg!(target_os = "macos") {
        let mut cursor = exe.as_path();
        while let Some(parent) = cursor.parent() {
            if parent.extension().is_some_and(|e| e == "app") {
                return InstallKind::MacAppBundle { bundle: parent.to_path_buf(), exe: exe.clone() };
            }
            cursor = parent;
        }
    }
    let text = exe.to_string_lossy();
    if text.starts_with("/usr/") || text.starts_with("/nix/") {
        return InstallKind::PackageManaged("your package manager".into());
    }
    if cfg!(windows) { InstallKind::WindowsExe(exe) } else { InstallKind::PlainBinary(exe) }
}

/// Manifest key for this binary, preferring the AppImage flavour when running as one.
pub fn target_keys(build: &LauncherBuild, kind: &InstallKind) -> Vec<String> {
    let mut keys = Vec::new();
    if matches!(kind, InstallKind::AppImage(_)) {
        keys.push(format!("{}-appimage", build.target));
    }
    keys.push(build.target.clone());
    if build.target.starts_with("macos-") {
        keys.push("macos-universal".into());
    }
    keys
}

pub async fn fetch_manifest(http: &Http, channel: Channel) -> Result<Manifest, CoreError> {
    http.get_json(&github::asset_url(LAUNCHER_REPO, channel, MANIFEST_NAME)).await
}

/// Pure comparison, unit-tested separately from the network.
pub fn evaluate(manifest: &Manifest, build: &LauncherBuild, channel: Channel, kind: &InstallKind) -> UpdateStatus {
    if let InstallKind::PackageManaged(name) = kind {
        return UpdateStatus::Disabled(name.clone());
    }
    let notes_url = manifest.notes_url.clone().unwrap_or_else(|| github::releases_url(LAUNCHER_REPO));
    if manifest.schema > SUPPORTED_SCHEMA {
        return UpdateStatus::ManualUpdateRequired { version: manifest.version.clone(), url: notes_url };
    }
    let newer = match channel {
        Channel::Stable => match (semver::Version::parse(&manifest.version), build.semver()) {
            (Ok(remote), Some(local)) => remote > local,
            (Ok(_), None) => true,
            (Err(_), _) => false,
        },
        Channel::Development => !manifest.commit.is_empty() && manifest.commit != build.commit && manifest.built_at > build.built_at,
    };
    if !newer {
        return UpdateStatus::UpToDate;
    }
    let Some((_, file)) = target_keys(build, kind).into_iter().find_map(|k| manifest.files.get(&k).map(|f| (k, f.clone()))) else {
        return UpdateStatus::NoBuildForPlatform;
    };
    let required =
        manifest.min_launcher_version.as_deref().and_then(|m| semver::Version::parse(m).ok()).zip(build.semver()).is_some_and(|(min, local)| local < min);
    UpdateStatus::Available(LauncherUpdate {
        version: manifest.version.clone(),
        commit: manifest.commit.clone(),
        channel,
        url: github::asset_url(LAUNCHER_REPO, channel, &file.name),
        file,
        notes_url,
        required,
    })
}

pub async fn check(http: &Http, build: &LauncherBuild, channel: Channel) -> UpdateStatus {
    if std::env::var_os(UPGRADED_ENV).is_some() {
        return UpdateStatus::Skipped;
    }
    let kind = detect_install_kind();
    match fetch_manifest(http, channel).await {
        Ok(manifest) => evaluate(&manifest, build, channel, &kind),
        // Nothing published for this channel yet (fresh project, a release without a
        // manifest, or a private repository whose assets GitHub hides behind a 404).
        Err(CoreError::HttpStatus { status: 404, .. }) => UpdateStatus::NoBuildForPlatform,
        Err(e) => {
            tracing::warn!("launcher update check failed: {e}");
            UpdateStatus::Offline(e.to_string())
        }
    }
}

/// Downloads the new build next to the current one and verifies it.
pub fn stage(http: Http, update: LauncherUpdate) -> impl Straw<PathBuf, Progress, CoreError> + Send + 'static {
    sipper::sipper(move |mut progress: sipper::Sender<Progress>| async move {
        let kind = detect_install_kind();
        let target_dir = staging_dir(&kind)?;
        std::fs::create_dir_all(&target_dir).context(format!("creating {}", target_dir.display()))?;
        let dest = target_dir.join(format!("{}.part", update.file.name));
        progress.send(Progress::phase(format!("Downloading launcher {}", update.version))).await;
        http.download(update.url.clone(), dest.clone(), Some(update.file.sha256.clone()), Some(update.file.size)).run(&progress).await?;
        let staged = target_dir.join(&update.file.name);
        rename_replace(&dest, &staged)?;
        Ok(staged)
    })
}

fn staging_dir(kind: &InstallKind) -> Result<PathBuf, CoreError> {
    let anchor = match kind {
        InstallKind::PlainBinary(p) | InstallKind::WindowsExe(p) | InstallKind::AppImage(p) => p.clone(),
        InstallKind::MacAppBundle { bundle, .. } => bundle.clone(),
        InstallKind::PackageManaged(name) => return Err(CoreError::other(format!("updates are handled by {name}"))),
    };
    let parent = anchor.parent().ok_or_else(|| CoreError::other("cannot determine install directory"))?;
    let dir = parent.join(".void-launcher-update");
    // Writability check up front so the user gets a clear message before downloading.
    std::fs::create_dir_all(&dir).map_err(|e| CoreError::other(format!("{} is not writable ({e}); download the update manually", parent.display())))?;
    Ok(dir)
}

/// Replaces the running launcher with `staged` and restarts. Only returns on error.
pub fn apply_and_restart(staged: &Path) -> Result<std::convert::Infallible, CoreError> {
    let kind = detect_install_kind();
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    match &kind {
        InstallKind::PlainBinary(exe) | InstallKind::WindowsExe(exe) => {
            set_executable(staged)?;
            self_replace::self_replace(staged).map_err(|e| CoreError::io("replacing the launcher", e))?;
            remove_file_quiet(staged);
            relaunch(exe, &args)
        }
        InstallKind::AppImage(appimage) => {
            set_executable(staged)?;
            rename_replace(staged, appimage)?;
            relaunch(appimage, &args)
        }
        InstallKind::MacAppBundle { bundle, .. } => {
            let unpack = staged.parent().unwrap_or(Path::new(".")).join("unpacked");
            remove_dir_quiet(&unpack);
            std::fs::create_dir_all(&unpack).context("creating unpack directory")?;
            let status = std::process::Command::new("/usr/bin/ditto")
                .args(["-x", "-k"])
                .arg(staged)
                .arg(&unpack)
                .status()
                .map_err(|e| CoreError::io("running ditto", e))?;
            if !status.success() {
                return Err(CoreError::Archive("could not unpack the launcher update".into()));
            }
            let new_bundle = std::fs::read_dir(&unpack)
                .context("listing unpacked update")?
                .flatten()
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|e| e == "app"))
                .ok_or_else(|| CoreError::Archive("the update did not contain an .app bundle".into()))?;
            remove_quarantine(&new_bundle);
            let old = bundle.with_extension("app.old");
            remove_dir_quiet(&old);
            rename_replace(bundle, &old)?;
            rename_replace(&new_bundle, bundle)?;
            remove_dir_quiet(&unpack);
            remove_file_quiet(staged);
            let mut command = std::process::Command::new("/usr/bin/open");
            command.arg("-n").arg(bundle).env(UPGRADED_ENV, "1");
            if !args.is_empty() {
                command.arg("--args").args(&args);
            }
            command.spawn().map_err(|e| CoreError::io("relaunching", e))?;
            std::process::exit(0);
        }
        InstallKind::PackageManaged(name) => Err(CoreError::other(format!("updates are handled by {name}"))),
    }
}

fn relaunch(exe: &Path, args: &[std::ffi::OsString]) -> Result<std::convert::Infallible, CoreError> {
    let mut command = std::process::Command::new(exe);
    command.args(args).env(UPGRADED_ENV, "1");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn().map_err(|e| CoreError::io("relaunching the launcher", e))?;
    std::process::exit(0);
}

/// Cleans up what a previous update left behind (`.app.old`, staging dirs).
pub fn finish_previous_update() {
    let kind = detect_install_kind();
    let anchor = match &kind {
        InstallKind::PlainBinary(p) | InstallKind::WindowsExe(p) | InstallKind::AppImage(p) => Some(p.clone()),
        InstallKind::MacAppBundle { bundle, .. } => {
            remove_dir_quiet(&bundle.with_extension("app.old"));
            Some(bundle.clone())
        }
        InstallKind::PackageManaged(_) => None,
    };
    if let Some(parent) = anchor.as_deref().and_then(Path::parent) {
        remove_dir_quiet(&parent.join(".void-launcher-update"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(version: &str, commit: &str, built_at: &str) -> LauncherBuild {
        LauncherBuild { version: version.into(), channel: Channel::Stable, commit: commit.into(), built_at: built_at.into(), target: "linux-x86_64".into() }
    }

    fn manifest(version: &str, commit: &str, built_at: &str) -> Manifest {
        let json = format!(
            r#"{{"schema":1,"version":"{version}","channel":"stable","commit":"{commit}","built_at":"{built_at}","future":true,
                "files":{{"linux-x86_64":{{"name":"void-launcher-linux-x86_64","sha256":"ab","size":1,"kind":"binary"}},
                          "linux-x86_64-appimage":{{"name":"void-launcher-linux-x86_64.AppImage","sha256":"cd","size":2,"kind":"appimage"}}}}}}"#
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn stable_uses_semver() {
        let kind = InstallKind::PlainBinary("/opt/void/launcher".into());
        let m = manifest("1.1.0", "bbb", "2026-09-05T00:00:00Z");
        match evaluate(&m, &build("1.0.0", "aaa", "2026-09-01T00:00:00Z"), Channel::Stable, &kind) {
            UpdateStatus::Available(u) => assert_eq!(u.url, "https://github.com/2011Scape/void-launcher/releases/latest/download/void-launcher-linux-x86_64"),
            other => panic!("expected update, got {other:?}"),
        }
        assert!(matches!(evaluate(&m, &build("1.1.0", "bbb", "2026-09-05T00:00:00Z"), Channel::Stable, &kind), UpdateStatus::UpToDate));
        assert!(matches!(evaluate(&m, &build("1.2.0", "ccc", "2026-09-06T00:00:00Z"), Channel::Stable, &kind), UpdateStatus::UpToDate));
        // A dev build always upgrades to the next release.
        assert!(matches!(evaluate(&m, &build("1.1.0-dev.abc", "abc", "2026-09-04T00:00:00Z"), Channel::Stable, &kind), UpdateStatus::Available(_)));
    }

    #[test]
    fn development_uses_commit_and_time() {
        let kind = InstallKind::PlainBinary("/opt/void/launcher".into());
        let m = manifest("1.1.0-dev.bbb", "bbb", "2026-09-05T00:00:00Z");
        assert!(matches!(evaluate(&m, &build("1.1.0-dev.aaa", "aaa", "2026-09-04T00:00:00Z"), Channel::Development, &kind), UpdateStatus::Available(_)));
        assert!(matches!(evaluate(&m, &build("1.1.0-dev.bbb", "bbb", "2026-09-05T00:00:00Z"), Channel::Development, &kind), UpdateStatus::UpToDate));
        assert!(matches!(evaluate(&m, &build("1.1.0-dev.ccc", "ccc", "2026-09-06T00:00:00Z"), Channel::Development, &kind), UpdateStatus::UpToDate));
        if let UpdateStatus::Available(u) = evaluate(&m, &build("1.0.0", "aaa", "2026-09-01T00:00:00Z"), Channel::Development, &kind) {
            assert_eq!(u.url, "https://github.com/2011Scape/void-launcher/releases/download/dev/void-launcher-linux-x86_64");
        } else {
            panic!("expected update");
        }
    }

    #[test]
    fn schema_platform_and_kind_rules() {
        let b = build("1.0.0", "aaa", "2026-09-01T00:00:00Z");
        let mut m = manifest("2.0.0", "bbb", "2026-09-05T00:00:00Z");
        m.schema = 2;
        assert!(matches!(evaluate(&m, &b, Channel::Stable, &InstallKind::PlainBinary("/x".into())), UpdateStatus::ManualUpdateRequired { .. }));
        let m = manifest("2.0.0", "bbb", "2026-09-05T00:00:00Z");
        assert!(matches!(evaluate(&m, &b, Channel::Stable, &InstallKind::PackageManaged("Snap".into())), UpdateStatus::Disabled(_)));
        if let UpdateStatus::Available(u) = evaluate(&m, &b, Channel::Stable, &InstallKind::AppImage("/x.AppImage".into())) {
            assert_eq!(u.file.kind, "appimage");
        } else {
            panic!("expected appimage update");
        }
        let mut other = build("1.0.0", "aaa", "");
        other.target = "windows-x86_64".into();
        assert!(matches!(evaluate(&m, &other, Channel::Stable, &InstallKind::WindowsExe("C:/x.exe".into())), UpdateStatus::NoBuildForPlatform));
    }
}
