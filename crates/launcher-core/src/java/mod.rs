//! Finding a Java that can run the client, or installing one.

pub mod copy;
pub mod discover;
pub mod probe;
pub mod runtime;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::paths::VoidPaths;
use crate::settings::{JavaSource, Settings};

/// The client needs Java 21 (it is built for it and uses `--enable-native-access`).
pub const MIN_JAVA: u32 = 21;
/// The newest Java the client is tested on; later ones get a warning, not a refusal.
pub const MAX_TESTED_JAVA: u32 = 25;
pub const MANAGED_FEATURE: u32 = 21;
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arch {
    X86_64,
    Aarch64,
    X86,
    Other(String),
}

impl Arch {
    pub fn host() -> Arch {
        match std::env::consts::ARCH {
            "x86_64" => Arch::X86_64,
            "aarch64" => Arch::Aarch64,
            "x86" => Arch::X86,
            other => Arch::Other(other.into()),
        }
    }

    pub fn parse(os_arch: &str) -> Arch {
        match os_arch.trim().to_ascii_lowercase().as_str() {
            "amd64" | "x86_64" | "x64" => Arch::X86_64,
            "aarch64" | "arm64" => Arch::Aarch64,
            "x86" | "i386" | "i686" => Arch::X86,
            other => Arch::Other(other.into()),
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arch::X86_64 => f.write_str("x86_64"),
            Arch::Aarch64 => f.write_str("arm64"),
            Arch::X86 => f.write_str("32-bit x86"),
            Arch::Other(s) => f.write_str(s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmKind {
    HotSpot,
    OpenJ9,
    Graal,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JavaWarning {
    /// Java 24+: the client's `sun.misc.Unsafe` use prints deprecation warnings.
    UnsafeDeprecated,
    /// Newer than [`MAX_TESTED_JAVA`]; the client has not been tested on it.
    TooNew,
    /// x86_64 Java on an arm64 Mac (runs under Rosetta, slower).
    Rosetta,
    /// Not HotSpot; ZGC is unavailable and the client is untested on it.
    NonHotSpot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JavaOrigin {
    Managed,
    Custom,
    /// A system install; the string says where it was found (`JAVA_HOME`, `PATH`, ...).
    System(String),
}

impl JavaOrigin {
    pub fn label(&self) -> String {
        match self {
            JavaOrigin::Managed => "managed runtime".into(),
            JavaOrigin::Custom => "custom path".into(),
            JavaOrigin::System(hint) => format!("system, {hint}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaInstall {
    pub home: PathBuf,
    pub exe: PathBuf,
    pub major: u32,
    pub full_version: String,
    pub vendor: String,
    pub arch: Arch,
    pub vm: VmKind,
    pub headless: bool,
    pub origin: JavaOrigin,
    pub warnings: Vec<JavaWarning>,
}

impl JavaInstall {
    pub fn is_usable(&self) -> bool {
        self.major >= MIN_JAVA && !self.headless && self.arch != Arch::X86
    }

    /// Ranking used to pick between several usable installs (higher wins).
    pub fn score(&self) -> i32 {
        let base = match self.major {
            21 => 100,
            25 => 80,
            m if m > MAX_TESTED_JAVA => 50,
            m if m >= MIN_JAVA => 70 - if m >= 24 { 5 } else { 0 },
            _ => 0,
        };
        let penalty = self
            .warnings
            .iter()
            .map(|w| match w {
                JavaWarning::Rosetta => 30,
                JavaWarning::NonHotSpot => 20,
                JavaWarning::UnsafeDeprecated | JavaWarning::TooNew => 0,
            })
            .sum::<i32>();
        base - penalty
    }

    pub fn summary(&self) -> String {
        format!("Java {} ({})", self.full_version, self.origin.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaProblem {
    NotFound,
    /// Something was found, but every candidate is older than [`MIN_JAVA`].
    TooOld {
        best: JavaInstall,
    },
    Headless {
        install: JavaInstall,
    },
    ArchMismatch {
        install: JavaInstall,
        host: Arch,
    },
    Broken {
        path: PathBuf,
        stderr: String,
    },
    ManagedNotInstalled,
    ManagedCorrupt {
        reason: String,
    },
    CustomPathInvalid {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for JavaProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JavaProblem::NotFound => write!(f, "no Java found"),
            JavaProblem::TooOld { best } => write!(f, "Java {} is too old (need {MIN_JAVA}+)", best.full_version),
            JavaProblem::Headless { install } => write!(f, "Java {} is a headless build", install.full_version),
            JavaProblem::ArchMismatch { install, host } => write!(f, "Java {} is {} but this machine is {host}", install.full_version, install.arch),
            JavaProblem::Broken { path, .. } => write!(f, "{} does not run", path.display()),
            JavaProblem::ManagedNotInstalled => write!(f, "managed Java runtime not installed"),
            JavaProblem::ManagedCorrupt { reason } => write!(f, "managed Java runtime is damaged: {reason}"),
            JavaProblem::CustomPathInvalid { path, .. } if path.as_os_str().is_empty() => write!(f, "no custom Java path set"),
            JavaProblem::CustomPathInvalid { path, reason } => write!(f, "{}: {reason}", path.display()),
        }
    }
}

/// Resolves the Java the client will run with, honouring `settings.java`.
pub async fn find_java(settings: &Settings, paths: &VoidPaths) -> Result<JavaInstall, JavaProblem> {
    match &settings.java {
        JavaSource::Managed => match runtime::installed(paths) {
            Some(info) => match probe::probe(&paths.runtime_dir().join(&info.java_home), JavaOrigin::Managed, PROBE_TIMEOUT).await {
                Ok(install) if install.is_usable() => Ok(install),
                Ok(install) => Err(JavaProblem::ManagedCorrupt { reason: format!("runtime is Java {} ({})", install.full_version, install.arch) }),
                Err(problem) => Err(JavaProblem::ManagedCorrupt { reason: problem.to_string() }),
            },
            // Nothing managed yet: quietly use a good system Java if there is one,
            // otherwise ask the UI to offer the install.
            None => match best_system(paths).await {
                Ok(install) => Ok(install),
                Err(_) => Err(JavaProblem::ManagedNotInstalled),
            },
        },
        JavaSource::System => best_system(paths).await,
        JavaSource::Custom { path } => {
            let home = probe::normalize_home(path);
            match probe::probe(&home, JavaOrigin::Custom, PROBE_TIMEOUT).await {
                Ok(install) if install.is_usable() => Ok(install),
                Ok(install) => Err(classify_unusable(install)),
                Err(JavaProblem::Broken { stderr, .. }) => Err(JavaProblem::CustomPathInvalid { path: path.clone(), reason: stderr }),
                Err(other) => Err(other),
            }
        }
    }
}

fn classify_unusable(install: JavaInstall) -> JavaProblem {
    if install.headless {
        JavaProblem::Headless { install }
    } else if install.arch == Arch::X86 {
        JavaProblem::ArchMismatch { host: Arch::host(), install }
    } else {
        JavaProblem::TooOld { best: install }
    }
}

/// Probes every system candidate and returns the best usable one.
pub async fn best_system(paths: &VoidPaths) -> Result<JavaInstall, JavaProblem> {
    let installs = list_system(paths).await;
    let mut usable: Vec<&JavaInstall> = installs.iter().filter(|i| i.is_usable()).collect();
    usable.sort_by_key(|i| std::cmp::Reverse(i.score()));
    if let Some(best) = usable.first() {
        return Ok((*best).clone());
    }
    // Pick the most informative failure.
    if let Some(i) = installs.iter().find(|i| i.headless && i.major >= MIN_JAVA) {
        return Err(JavaProblem::Headless { install: i.clone() });
    }
    if let Some(i) = installs.iter().find(|i| i.arch == Arch::X86 && i.major >= MIN_JAVA) {
        return Err(JavaProblem::ArchMismatch { install: i.clone(), host: Arch::host() });
    }
    if let Some(best) = installs.iter().max_by_key(|i| i.major) {
        return Err(JavaProblem::TooOld { best: best.clone() });
    }
    Err(JavaProblem::NotFound)
}

/// Every system Java that answered `java -version`, cached by executable identity.
pub async fn list_system(paths: &VoidPaths) -> Vec<JavaInstall> {
    let candidates = discover::candidates(paths);
    let mut cache = ProbeCache::load(paths);
    let mut results = Vec::new();
    let mut to_probe = Vec::new();
    for (home, hint) in candidates {
        let exe = probe::exe_for_home(&home);
        match cache.get(&exe) {
            Some(install) => results.push(JavaInstall { origin: JavaOrigin::System(hint), ..install }),
            None => to_probe.push((home, hint)),
        }
    }
    let mut probes = futures_util::stream::iter(to_probe.into_iter().map(|(home, hint)| async move {
        let result = probe::probe(&home, JavaOrigin::System(hint.clone()), PROBE_TIMEOUT).await;
        (home, hint, result)
    }))
    .buffer_unordered(4);
    while let Some((home, _hint, result)) = probes.next().await {
        match result {
            Ok(install) => {
                cache.put(&install);
                results.push(install);
            }
            Err(e) => tracing::debug!("{}: {e}", home.display()),
        }
    }
    cache.save(paths);
    results.sort_by_key(|i| std::cmp::Reverse(i.score()));
    results
}

#[derive(Default, Serialize, Deserialize)]
struct ProbeCache {
    #[serde(default)]
    entries: HashMap<String, CacheEntry>,
    #[serde(skip)]
    dirty: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct CacheEntry {
    size: u64,
    mtime_ms: u64,
    install: JavaInstall,
}

impl ProbeCache {
    fn load(paths: &VoidPaths) -> Self {
        std::fs::read(paths.java_cache_json()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn identity(exe: &Path) -> Option<(u64, u64)> {
        let meta = std::fs::metadata(exe).ok()?;
        let mtime = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64;
        Some((meta.len(), mtime))
    }

    fn get(&self, exe: &Path) -> Option<JavaInstall> {
        let (size, mtime_ms) = Self::identity(exe)?;
        let entry = self.entries.get(&exe.to_string_lossy().into_owned())?;
        (entry.size == size && entry.mtime_ms == mtime_ms).then(|| entry.install.clone())
    }

    fn put(&mut self, install: &JavaInstall) {
        if let Some((size, mtime_ms)) = Self::identity(&install.exe) {
            self.entries.insert(install.exe.to_string_lossy().into_owned(), CacheEntry { size, mtime_ms, install: install.clone() });
            self.dirty = true;
        }
    }

    fn save(&self, paths: &VoidPaths) {
        if self.dirty {
            let _ = crate::fsutil::atomic_write_json(&paths.java_cache_json(), self);
        }
    }
}
