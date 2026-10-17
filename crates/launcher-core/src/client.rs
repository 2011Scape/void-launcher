//! Installing and updating the client jar from 634-client's GitHub releases.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sipper::{Sipper, Straw};

use crate::CLIENT_REPO;
use crate::error::{CoreError, IoContext};
use crate::fsutil::{atomic_write_json, remove_file_quiet, rename_replace};
use crate::github::{self, Checksum};
use crate::hash::{same_digest, sha256_file};
use crate::http::Http;
use crate::paths::VoidPaths;
use crate::progress::Progress;
use crate::version::{Channel, commit_from_dev_version};

/// Release assets are `runetek5-634-<version>.jar` (and a browser zip the launcher ignores).
pub const JAR_PREFIX: &str = "runetek5-634-";
/// Every jar the launcher owns starts with this (installed versions and the hand-dropped fallback).
const JAR_STEM: &str = "runetek5-634";
const BROWSER_MARKER: &str = "-browser-";

/// `~/.void/client/client.json`: what is installed and how it was verified.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientState {
    pub schema: u32,
    pub channel: Channel,
    pub version: String,
    pub commit: Option<String>,
    pub sha256: String,
    pub size: u64,
    pub mtime_ms: u64,
    pub installed_at: String,
    pub source_url: String,
    pub jar: String,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            schema: 1,
            channel: Channel::Stable,
            version: String::new(),
            commit: None,
            sha256: String::new(),
            size: 0,
            mtime_ms: 0,
            installed_at: String::new(),
            source_url: String::new(),
            jar: String::new(),
        }
    }
}

impl ClientState {
    pub fn label(&self) -> String {
        match &self.commit {
            Some(c) if !self.version.contains(c) => format!("{} ({c})", self.version),
            _ => self.version.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteClient {
    pub channel: Channel,
    /// `2.8.0` or `dev-20261017-fd94175`.
    pub version: String,
    pub commit: Option<String>,
    pub sha256: String,
    /// From `Content-Length`; 0 when the size could not be learned up front.
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClientStatus {
    Current(ClientState),
    UpdateAvailable {
        installed: Option<ClientState>,
        remote: RemoteClient,
    },
    /// Could not reach GitHub; play with what is installed, if anything.
    Offline {
        installed: Option<ClientState>,
        reason: String,
    },
}

/// Version and commit read from a jar's manifest.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JarInfo {
    pub version: Option<String>,
    pub commit: Option<String>,
}

/// Reads `Implementation-Version` (and `Git-Commit`, if present) from `META-INF/MANIFEST.MF`.
pub fn read_jar_manifest(jar: &Path) -> Result<JarInfo, CoreError> {
    let file = std::fs::File::open(jar).context(format!("opening {}", jar.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| CoreError::Archive(format!("{}: {e}", jar.display())))?;
    let mut entry = archive.by_name("META-INF/MANIFEST.MF").map_err(|e| CoreError::Archive(format!("{} has no manifest: {e}", jar.display())))?;
    let mut text = String::new();
    entry.read_to_string(&mut text).context("reading manifest")?;
    Ok(parse_manifest(&text))
}

/// Manifest lines longer than 72 bytes continue on the next line with a leading space.
pub fn parse_manifest(text: &str) -> JarInfo {
    let mut joined: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(cont) = line.strip_prefix(' ') {
            if let Some(last) = joined.last_mut() {
                last.push_str(cont);
            }
        } else {
            joined.push(line.to_string());
        }
    }
    let mut info = JarInfo::default();
    for line in joined {
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            match key.trim() {
                "Implementation-Version" if !value.is_empty() => info.version = Some(value.to_string()),
                "Git-Commit" if !value.is_empty() && value != "unknown" => info.commit = Some(value.to_string()),
                _ => {}
            }
        }
    }
    if info.commit.is_none() {
        info.commit = info.version.as_deref().and_then(commit_from_dev_version).map(str::to_string);
    }
    info
}

/// The desktop jar in a release's checksum list, with its version.
pub fn pick_jar(sums: &[Checksum]) -> Option<(&Checksum, String)> {
    sums.iter().find_map(|c| {
        let version = c.name.strip_prefix(JAR_PREFIX)?.strip_suffix(".jar")?;
        (!c.name.contains(BROWSER_MARKER) && !version.is_empty()).then(|| (c, version.to_string()))
    })
}

/// Resolves what the channel currently offers from its `SHA256SUMS` (no jar download).
pub async fn resolve_remote(http: &Http, channel: Channel) -> Result<RemoteClient, CoreError> {
    let sums = github::fetch_checksums(http, CLIENT_REPO, channel).await?;
    let (jar, version) = pick_jar(&sums).ok_or_else(|| CoreError::BadResponse {
        url: github::asset_url(CLIENT_REPO, channel, github::CHECKSUMS_NAME),
        reason: format!("no {JAR_PREFIX}*.jar in SHA256SUMS"),
    })?;
    let url = github::asset_url(CLIENT_REPO, channel, &jar.name);
    let size = http.content_length(&url).await.unwrap_or(0);
    Ok(RemoteClient { channel, commit: commit_from_dev_version(&version).map(str::to_string), version, sha256: jar.sha256.clone(), size, url })
}

fn file_identity(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64;
    Some((meta.len(), mtime))
}

/// Each install gets its own file, so an update never replaces a jar a running client holds.
fn versioned_jar_name(sha256: &str) -> String {
    format!("{JAR_PREFIX}{}.jar", &sha256[..12])
}

pub fn current_jar(paths: &VoidPaths) -> PathBuf {
    std::fs::read(paths.client_json())
        .ok()
        .and_then(|b| serde_json::from_slice::<ClientState>(&b).ok())
        .map(|state| state.jar)
        .filter(|name| !name.is_empty() && Path::new(name).file_name().is_some_and(|n| n == name.as_str()))
        .map(|name| paths.client_dir().join(name))
        .filter(|jar| jar.is_file())
        .unwrap_or_else(|| paths.client_jar())
}

fn prune_old_jars(paths: &VoidPaths) {
    let current = current_jar(paths);
    if !current.is_file() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(paths.client_dir()) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path != current && name.starts_with(JAR_STEM) && name.ends_with(".jar") {
            remove_file_quiet(&path);
        }
    }
}

/// The installed client, re-hashing the jar when it changed underneath `client.json`.
pub fn installed(paths: &VoidPaths) -> Result<Option<ClientState>, CoreError> {
    let jar = current_jar(paths);
    let Some((size, mtime_ms)) = file_identity(&jar) else {
        return Ok(None);
    };
    let cached: Option<ClientState> = std::fs::read(paths.client_json()).ok().and_then(|b| serde_json::from_slice(&b).ok());
    if let Some(state) = cached
        && state.size == size
        && state.mtime_ms == mtime_ms
        && state.sha256.len() == 64
    {
        return Ok(Some(state));
    }
    tracing::info!("hashing {} (client.json missing or stale)", jar.display());
    let sha256 = sha256_file(&jar).context(format!("hashing {}", jar.display()))?;
    let info = read_jar_manifest(&jar).unwrap_or_default();
    let mut state = cached_or_default(paths);
    state.version = info.version.clone().unwrap_or_else(|| "unknown".into());
    state.commit = info.commit;
    state.sha256 = sha256;
    state.size = size;
    state.mtime_ms = mtime_ms;
    state.jar = jar.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let _ = atomic_write_json(&paths.client_json(), &state);
    Ok(Some(state))
}

fn cached_or_default(paths: &VoidPaths) -> ClientState {
    std::fs::read(paths.client_json()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Never fails: network trouble degrades to [`ClientStatus::Offline`].
pub async fn check(http: &Http, paths: &VoidPaths, channel: Channel) -> ClientStatus {
    let paths_clone = paths.clone();
    let installed = match tokio::task::spawn_blocking(move || {
        let state = installed(&paths_clone);
        prune_old_jars(&paths_clone);
        state
    })
    .await
    {
        Ok(Ok(state)) => state,
        Ok(Err(e)) => {
            tracing::warn!("could not read installed client: {e}");
            None
        }
        Err(e) => {
            tracing::warn!("hash task failed: {e}");
            None
        }
    };
    match resolve_remote(http, channel).await {
        Ok(remote) => match &installed {
            Some(state) if state.channel == channel && same_digest(&state.sha256, &remote.sha256) => ClientStatus::Current(state.clone()),
            _ => ClientStatus::UpdateAvailable { installed, remote },
        },
        Err(e) => {
            tracing::warn!("client update check failed: {e}");
            ClientStatus::Offline { installed, reason: e.to_string() }
        }
    }
}

/// Downloads and installs `remote`, verifying its checksum before touching the old jar.
pub fn install(http: Http, paths: VoidPaths, remote: RemoteClient) -> impl Straw<ClientState, Progress, CoreError> + Send + 'static {
    sipper::sipper(move |mut progress: sipper::Sender<Progress>| async move {
        paths.ensure_dirs().context("creating launcher directories")?;
        progress.send(Progress::phase(format!("Downloading client {}", remote.version))).await;
        let part = paths.tmp_dir().join("client.jar.part");
        let size = (remote.size > 0).then_some(remote.size);
        let sha256 = http.download(remote.url.clone(), part.clone(), Some(remote.sha256.clone()), size).run(&progress).await?;
        progress.send(Progress::phase("Installing client")).await;
        let info = read_jar_manifest(&part)?;
        let jar_name = versioned_jar_name(&sha256);
        let jar = paths.client_dir().join(&jar_name);
        if jar.is_file() && sha256_file(&jar).is_ok_and(|existing| same_digest(&existing, &sha256)) {
            remove_file_quiet(&part);
        } else {
            rename_replace(&part, &jar)?;
        }
        let (size, mtime_ms) = file_identity(&jar).unwrap_or((remote.size, 0));
        let state = ClientState {
            schema: 1,
            channel: remote.channel,
            version: info.version.unwrap_or(remote.version.clone()),
            commit: info.commit.or(remote.commit.clone()),
            sha256,
            size,
            mtime_ms,
            installed_at: now_rfc3339(),
            source_url: remote.url.clone(),
            jar: jar_name,
        };
        atomic_write_json(&paths.client_json(), &state)?;
        prune_old_jars(&paths);
        Ok(state)
    })
}

pub fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::parse_sha256sums;

    #[test]
    fn picks_the_desktop_jar_from_checksums() {
        let sums = parse_sha256sums(
            "7f80  runetek5-634-browser-2.8.0.zip\n\
             0827bc5179b58950f426dd98dc2d2fbec51439cda806d64fdb3c5e8ab78d8de1  runetek5-634-2.8.0.jar\n\
             7f804886d077f5f17245c29685c445a84e09b0faf3eee859750644ed3bfddd6d  runetek5-634-browser-2.8.0.zip\n",
        );
        let (jar, version) = pick_jar(&sums).unwrap();
        assert_eq!(jar.name, "runetek5-634-2.8.0.jar");
        assert_eq!(version, "2.8.0");
        assert_eq!(commit_from_dev_version(&version), None);

        let dev = parse_sha256sums("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  runetek5-634-dev-20261017-fd94175.jar\n");
        let (_, version) = pick_jar(&dev).unwrap();
        assert_eq!(version, "dev-20261017-fd94175");
        assert_eq!(commit_from_dev_version(&version), Some("fd94175"));
        assert!(pick_jar(&parse_sha256sums("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  other.jar\n")).is_none());
    }

    #[test]
    fn current_jar_follows_client_json_and_prunes_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let paths = VoidPaths::with_home(dir.path());
        paths.ensure_dirs().unwrap();
        assert_eq!(current_jar(&paths), paths.client_jar());

        std::fs::write(paths.client_jar(), b"old").unwrap();
        let name = versioned_jar_name(&"ab".repeat(32));
        let new = paths.client_dir().join(&name);
        std::fs::write(&new, b"new").unwrap();
        assert_eq!(current_jar(&paths), paths.client_jar());

        atomic_write_json(&paths.client_json(), &ClientState { jar: name.clone(), ..ClientState::default() }).unwrap();
        assert_eq!(current_jar(&paths), new);
        prune_old_jars(&paths);
        assert!(new.is_file());
        assert!(!paths.client_jar().exists());
        assert!(paths.client_json().is_file());

        atomic_write_json(&paths.client_json(), &ClientState { jar: "../escape.jar".into(), ..ClientState::default() }).unwrap();
        assert_eq!(current_jar(&paths), paths.client_jar());
        prune_old_jars(&paths);
        assert!(new.is_file());
    }

    #[test]
    fn parses_manifest_with_continuations_and_missing_fields() {
        let text = "Manifest-Version: 1.0\r\nMain-Class: RuneTek5\r\nImplementation-Version: dev-2026\r\n 1017-fd94175\r\nMulti-Release: true\r\n";
        let info = parse_manifest(text);
        assert_eq!(info.version.as_deref(), Some("dev-20261017-fd94175"));
        assert_eq!(info.commit.as_deref(), Some("fd94175"));
        assert_eq!(parse_manifest("Implementation-Version: 2.8.0\n"), JarInfo { version: Some("2.8.0".into()), commit: None });
        assert_eq!(parse_manifest("Manifest-Version: 1.0\n"), JarInfo::default());
        assert_eq!(parse_manifest("Git-Commit: unknown\n").commit, None);
    }

    #[test]
    fn reads_manifest_from_a_real_jar() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("t.jar");
        {
            let file = std::fs::File::create(&jar).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("META-INF/MANIFEST.MF", zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut zip, b"Implementation-Version: 2.8.0\nGit-Commit: abc1234\n").unwrap();
            zip.finish().unwrap();
        }
        let info = read_jar_manifest(&jar).unwrap();
        assert_eq!(info.version.as_deref(), Some("2.8.0"));
        assert_eq!(info.commit.as_deref(), Some("abc1234"));
    }
}
