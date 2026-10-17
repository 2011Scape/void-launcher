//! The launcher-managed Temurin JRE in `~/.void/runtime`, fetched from the Adoptium API.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sipper::{Sipper, Straw};

use super::{JavaOrigin, MANAGED_FEATURE, probe};
use crate::error::{CoreError, IoContext};
use crate::fsutil::{atomic_write_json, remove_dir_quiet, remove_quarantine, rename_replace, set_executable};
use crate::http::Http;
use crate::paths::VoidPaths;
use crate::progress::Progress;

pub const ADOPTIUM_API: &str = "https://api.adoptium.net/v3";

/// `runtime/runtime.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeInfo {
    pub schema: u32,
    pub release_name: String,
    /// Relative to the runtime directory.
    pub java_home: PathBuf,
    pub major: u32,
    pub semver: String,
    pub os: String,
    pub arch: String,
    pub sha256: String,
    pub installed_at: String,
    pub source_url: String,
}

impl Default for RuntimeInfo {
    fn default() -> Self {
        Self {
            schema: 1,
            release_name: String::new(),
            java_home: PathBuf::new(),
            major: 0,
            semver: String::new(),
            os: String::new(),
            arch: String::new(),
            sha256: String::new(),
            installed_at: String::new(),
            source_url: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdoptiumAsset {
    pub release_name: String,
    pub semver: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub file_name: String,
}

#[derive(Deserialize)]
struct AssetsResponse {
    release_name: String,
    binary: Binary,
    version: VersionData,
}
#[derive(Deserialize)]
struct Binary {
    package: Package,
}
#[derive(Deserialize)]
struct Package {
    link: String,
    checksum: Option<String>,
    size: u64,
    name: String,
}
#[derive(Deserialize)]
struct VersionData {
    semver: String,
}

/// `(os, architecture)` query values for the Adoptium API.
pub fn adoptium_target() -> Result<(&'static str, &'static str), CoreError> {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "mac",
        "linux" => {
            if Path::new("/etc/alpine-release").exists() {
                "alpine-linux"
            } else {
                "linux"
            }
        }
        other => return Err(CoreError::other(format!("no managed Java runtime is published for {other}"))),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        other => return Err(CoreError::other(format!("no managed Java runtime is published for {other}"))),
    };
    Ok((os, arch))
}

pub async fn latest_asset(http: &Http) -> Result<AdoptiumAsset, CoreError> {
    let (os, arch) = adoptium_target()?;
    let url = format!("{ADOPTIUM_API}/assets/latest/{MANAGED_FEATURE}/hotspot?architecture={arch}&image_type=jre&os={os}&vendor=eclipse");
    let assets: Vec<AssetsResponse> = http.get_json(&url).await?;
    let asset = assets
        .into_iter()
        .next()
        .ok_or_else(|| CoreError::BadResponse { url: url.clone(), reason: format!("no Temurin {MANAGED_FEATURE} JRE for {os}/{arch} yet; try again later") })?;
    let sha256 = asset
        .binary
        .package
        .checksum
        .filter(|s| s.len() == 64)
        .ok_or_else(|| CoreError::BadResponse { url: url.clone(), reason: "asset has no checksum".into() })?;
    Ok(AdoptiumAsset {
        release_name: asset.release_name,
        semver: asset.version.semver,
        url: asset.binary.package.link,
        sha256,
        size: asset.binary.package.size,
        file_name: asset.binary.package.name,
    })
}

pub fn installed(paths: &VoidPaths) -> Option<RuntimeInfo> {
    let info: RuntimeInfo = serde_json::from_slice(&std::fs::read(paths.runtime_json()).ok()?).ok()?;
    let exe = probe::exe_for_home(&paths.runtime_dir().join(&info.java_home));
    exe.is_file().then_some(info)
}

/// Downloads, verifies, unpacks and activates the latest Temurin JRE.
pub fn install(http: Http, paths: VoidPaths) -> impl Straw<RuntimeInfo, Progress, CoreError> + Send + 'static {
    sipper::sipper(move |mut progress: sipper::Sender<Progress>| async move {
        paths.ensure_dirs().context("creating launcher directories")?;
        progress.send(Progress::phase("Finding the latest Java 21")).await;
        let asset = latest_asset(&http).await?;
        let (os, arch) = adoptium_target()?;

        progress.send(Progress::phase(format!("Downloading Java {}", asset.semver))).await;
        let archive = paths.tmp_dir().join(format!("{}.part", asset.file_name));
        let sha256 = http.download(asset.url.clone(), archive.clone(), Some(asset.sha256.clone()), Some(asset.size)).run(&progress).await?;

        progress.send(Progress::phase("Unpacking Java")).await;
        let extract_dir = paths.tmp_dir().join(format!("extract-{}", std::process::id()));
        remove_dir_quiet(&extract_dir);
        let archive_for_task = archive.clone();
        let extract_for_task = extract_dir.clone();
        let is_zip = asset.file_name.to_ascii_lowercase().ends_with(".zip");
        tokio::task::spawn_blocking(move || extract(&archive_for_task, &extract_for_task, is_zip))
            .await
            .map_err(|e| CoreError::other(format!("extraction task failed: {e}")))??;
        let _ = std::fs::remove_file(&archive);

        let top = single_child_dir(&extract_dir)?;
        let home_rel = if top.join("Contents").join("Home").join("bin").is_dir() { PathBuf::from("Contents").join("Home") } else { PathBuf::new() };
        if !probe::exe_for_home(&top.join(&home_rel)).is_file() {
            remove_dir_quiet(&extract_dir);
            return Err(CoreError::Archive("the Java archive did not contain bin/java".into()));
        }
        fix_permissions(&top.join(&home_rel));
        remove_quarantine(&top);

        let dir_name = asset.release_name.replace('+', "_");
        let target = paths.runtime_dir().join(&dir_name);
        remove_dir_quiet(&target);
        rename_replace(&top, &target)?;
        remove_dir_quiet(&extract_dir);

        progress.send(Progress::phase("Checking Java")).await;
        let install = probe::probe(&target.join(&home_rel), JavaOrigin::Managed, std::time::Duration::from_secs(20))
            .await
            .map_err(|e| CoreError::other(format!("the new runtime does not start: {e}")))?;

        let info = RuntimeInfo {
            schema: 1,
            release_name: asset.release_name.clone(),
            java_home: if home_rel.as_os_str().is_empty() { PathBuf::from(&dir_name) } else { PathBuf::from(&dir_name).join(&home_rel) },
            major: install.major,
            semver: asset.semver.clone(),
            os: os.into(),
            arch: arch.into(),
            sha256,
            installed_at: crate::client::now_rfc3339(),
            source_url: asset.url.clone(),
        };
        atomic_write_json(&paths.runtime_json(), &info)?;
        cleanup_others(&paths, &dir_name);
        Ok(info)
    })
}

fn extract(archive: &Path, into: &Path, is_zip: bool) -> Result<(), CoreError> {
    std::fs::create_dir_all(into).context(format!("creating {}", into.display()))?;
    let file = std::fs::File::open(archive).context(format!("opening {}", archive.display()))?;
    if is_zip {
        let mut zip = zip::ZipArchive::new(file).map_err(|e| CoreError::Archive(e.to_string()))?;
        zip.extract(into).map_err(|e| CoreError::Archive(e.to_string()))?;
    } else {
        let decoder = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(decoder);
        tar.set_preserve_permissions(true);
        tar.unpack(into).map_err(|e| CoreError::Archive(e.to_string()))?;
    }
    Ok(())
}

fn single_child_dir(dir: &Path) -> Result<PathBuf, CoreError> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .context(format!("listing {}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !p.file_name().map(|n| n.to_string_lossy().starts_with("__MACOSX")).unwrap_or(false))
        .collect();
    match dirs.len() {
        1 => Ok(dirs.remove(0)),
        0 => Err(CoreError::Archive("the Java archive was empty".into())),
        _ => Err(CoreError::Archive("the Java archive had an unexpected layout".into())),
    }
}

fn fix_permissions(home: &Path) {
    if let Ok(entries) = std::fs::read_dir(home.join("bin")) {
        for entry in entries.flatten() {
            let _ = set_executable(&entry.path());
        }
    }
    let _ = set_executable(&home.join("lib").join("jspawnhelper"));
}

fn cleanup_others(paths: &VoidPaths, keep: &str) {
    if let Ok(entries) = std::fs::read_dir(paths.runtime_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.file_name().map(|n| n != keep).unwrap_or(false) {
                remove_dir_quiet(&path);
            }
        }
    }
}

/// Removes the managed runtime entirely.
pub fn uninstall(paths: &VoidPaths) {
    remove_dir_quiet(&paths.runtime_dir());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_adoptium_response() {
        let json = r#"[{"binary":{"package":{"checksum":"ab","link":"https://x/jre.tar.gz","name":"OpenJDK21U-jre_x64_linux_hotspot_21.0.12_1.tar.gz","size":50}},"release_name":"jdk-21.0.12+1","version":{"semver":"21.0.12+1"}}]"#;
        let assets: Vec<AssetsResponse> = serde_json::from_str(json).unwrap();
        assert_eq!(assets[0].release_name, "jdk-21.0.12+1");
        assert_eq!(assets[0].binary.package.size, 50);
    }
}
