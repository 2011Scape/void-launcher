//! Building the client command line and spawning it detached from the launcher.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::client::current_jar;
use crate::error::{CoreError, IoContext};
use crate::java::{JavaInstall, VmKind};
use crate::paths::{VoidPaths, plugins_dir_under};
use crate::settings::{DEFAULT_WORLD, Settings};

/// Everything needed to start the client, fully resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchPlan {
    pub exe: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub log_path: PathBuf,
    pub heap_mb: u64,
    /// The requested heap was reduced to fit physical memory.
    pub capped: bool,
    pub host: String,
    pub port: u16,
}

impl LaunchPlan {
    /// Shell-style rendering for logs and the "launch failed" dialog.
    pub fn command_line(&self) -> String {
        std::iter::once(self.exe.to_string_lossy().into_owned())
            .chain(self.args.iter().map(|a| a.to_string_lossy().into_owned()))
            .map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LaunchOutcome {
    Running { pid: u32, exit: ExitWatch },
    EarlyExit { code: Option<i32>, log_tail: String, log_path: PathBuf },
}

/// Resolves once the detached client exits, with its status code. Only one
/// caller can wait on it; compares equal to any other watch so [`LaunchOutcome`]
/// stays comparable.
#[derive(Debug, Clone)]
pub struct ExitWatch(Arc<Mutex<Option<tokio::sync::oneshot::Receiver<Option<i32>>>>>);

impl ExitWatch {
    pub async fn wait(self) -> Option<i32> {
        let receiver = self.0.lock().ok().and_then(|mut slot| slot.take());
        match receiver {
            Some(receiver) => receiver.await.unwrap_or(None),
            None => std::future::pending().await,
        }
    }
}

impl PartialEq for ExitWatch {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// Caps the heap at 60 % of physical memory, never below 1 GB.
pub fn effective_heap_mb(requested_mb: u64, physical_mb: Option<u64>) -> (u64, bool) {
    let requested = requested_mb.max(1024);
    match physical_mb {
        Some(phys) if phys > 0 => {
            let cap = (phys * 6 / 10).max(1024);
            if requested > cap { (cap, true) } else { (requested, false) }
        }
        _ => (requested, false),
    }
}

pub fn physical_memory_mb() -> Option<u64> {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};
    let system = System::new_with_specifics(RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()));
    let bytes = system.total_memory();
    (bytes > 0).then_some(bytes / (1024 * 1024))
}

/// Where the client started from this profile will load plugins, or `None` when
/// plugins are switched off. Mirrors the client (`RuneTek5.kt`, `PluginDirectory.kt`):
/// `--plugins <dir>` / `--no-plugins` among the extra client arguments win over
/// `-Dclient.plugins=<dir|none>` among the extra JVM arguments, which wins over the
/// default `<user.home>/.634-client/plugins`, where `user.home` honours a
/// `-Duser.home=<dir>` JVM argument. The last occurrence of each wins. A leading `~`
/// is the (possibly overridden) home; a relative folder is relative to the client's
/// working directory, which `build_plan` sets to `paths.root`.
pub fn plugins_dir(settings: &Settings, paths: &VoidPaths) -> Option<PathBuf> {
    let jvm = shlex::split(&settings.extra_jvm_args).unwrap_or_default();
    let client = shlex::split(&settings.extra_client_args).unwrap_or_default();
    let home = jvm.iter().rev().find_map(|a| a.strip_prefix("-Duser.home=")).filter(|h| !h.is_empty()).map_or_else(|| paths.home.clone(), PathBuf::from);
    let mut property: Option<String> = jvm.iter().rev().find_map(|a| a.strip_prefix("-Dclient.plugins=")).map(str::to_owned);
    let mut i = 0;
    while i < client.len() {
        match client[i].as_str() {
            "--plugins" => {
                i += 1;
                property = Some(client.get(i).cloned().unwrap_or_default());
            }
            "--no-plugins" => property = Some("none".into()),
            _ => {}
        }
        i += 1;
    }
    match property.as_deref() {
        Some(p) if p.is_empty() || p.eq_ignore_ascii_case("none") => None,
        Some(p) => Some(expand_client_path(p, &home, &paths.root)),
        None => Some(plugins_dir_under(&home)),
    }
}

/// `~`/`~/x` is the home; anything relative hangs off the client's working directory.
fn expand_client_path(value: &str, home: &Path, cwd: &Path) -> PathBuf {
    let expanded = if value == "~" {
        home.to_path_buf()
    } else if let Some(rest) = value.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(value)
    };
    if expanded.is_absolute() { expanded } else { cwd.join(expanded) }
}

pub fn build_plan(settings: &Settings, java: &JavaInstall, paths: &VoidPaths) -> LaunchPlan {
    build_plan_with_memory(settings, java, paths, physical_memory_mb())
}

/// `java -Xms.. -Xmx.. [-XX:+UseZGC] <extra JVM args> -jar <jar> --address <host> --port <port>
/// [--world n] [--keys file] [--toolkit hd|opengl|java] [gameplay flags] [graphics flags] [--debug] <extra client args>`
pub fn build_plan_with_memory(settings: &Settings, java: &JavaInstall, paths: &VoidPaths, physical_mb: Option<u64>) -> LaunchPlan {
    let (heap_mb, capped) = effective_heap_mb(settings.max_heap_mb(), physical_mb);
    let mut args: Vec<OsString> = Vec::new();
    args.push(format!("-Xms{}m", heap_mb.min(1024)).into());
    args.push(format!("-Xmx{heap_mb}m").into());
    if java.major >= 17 && java.vm == VmKind::HotSpot {
        args.push("-XX:+UseZGC".into());
    }
    if let Some(extra) = shlex::split(&settings.extra_jvm_args) {
        args.extend(extra.into_iter().map(OsString::from));
    }
    args.push("-jar".into());
    args.push(current_jar(paths).into());
    let host = settings.server.host().to_string();
    let port = settings.server.port();
    args.push("--address".into());
    args.push(host.clone().into());
    args.push("--port".into());
    args.push(port.to_string().into());
    if settings.world != DEFAULT_WORLD {
        args.push("--world".into());
        args.push(settings.world.to_string().into());
    }
    if let Some(keys) = settings.keys_file() {
        args.push("--keys".into());
        args.push(keys.clone().into());
    }
    if let Some(toolkit) = settings.renderer.toolkit_arg() {
        args.push("--toolkit".into());
        args.push(toolkit.into());
    }
    args.extend(settings.extras.flags().into_iter().map(OsString::from));
    args.extend(settings.graphics.args().into_iter().map(OsString::from));
    if settings.debug {
        args.push("--debug".into());
    }
    if let Some(extra) = shlex::split(&settings.extra_client_args) {
        args.extend(extra.into_iter().map(OsString::from));
    }

    let stamp = time::OffsetDateTime::now_utc()
        .format(&time::macros::format_description!("[year][month][day]-[hour][minute][second]"))
        .unwrap_or_else(|_| "now".into());
    LaunchPlan {
        exe: java.exe.clone(),
        args,
        cwd: paths.root.clone(),
        log_path: paths.logs_dir().join(format!("client-{stamp}.log")),
        heap_mb,
        capped,
        host,
        port,
    }
}

/// Spawns the client detached. Reports [`LaunchOutcome::EarlyExit`] when it
/// dies within three seconds (bad Java, missing natives, wrong host).
pub async fn launch(plan: &LaunchPlan) -> Result<LaunchOutcome, CoreError> {
    std::fs::create_dir_all(&plan.cwd).context("creating working directory")?;
    if let Some(parent) = plan.log_path.parent() {
        std::fs::create_dir_all(parent).context("creating logs directory")?;
    }
    let log = std::fs::File::create(&plan.log_path).context(format!("creating {}", plan.log_path.display()))?;
    let err_log = log.try_clone().context("cloning log handle")?;

    let mut command = tokio::process::Command::new(&plan.exe);
    command.args(&plan.args).current_dir(&plan.cwd).stdin(Stdio::null()).stdout(Stdio::from(log)).stderr(Stdio::from(err_log)).kill_on_drop(false);
    detach(&mut command);

    tracing::info!("launching: {}", plan.command_line());
    let mut child = command.spawn().map_err(|e| CoreError::io(format!("starting {}", plan.exe.display()), e))?;
    let pid = child.id().unwrap_or(0);

    match tokio::time::timeout(Duration::from_secs(3), child.wait()).await {
        Ok(Ok(status)) => {
            let tail = log_tail(&plan.log_path, 40);
            let hs_err = newest_hs_err(&plan.cwd).map(|p| format!("\n--- {} ---\n{}", p.display(), log_tail(&p, 20))).unwrap_or_default();
            Ok(LaunchOutcome::EarlyExit { code: status.code(), log_tail: format!("{tail}{hs_err}"), log_path: plan.log_path.clone() })
        }
        Ok(Err(e)) => Err(CoreError::io("waiting for client", e)),
        Err(_) => {
            // Still running after the grace period. Keep reaping it in the background
            // so it never becomes a zombie while the launcher stays open, and tell
            // whoever is interested how it ended.
            let (sender, receiver) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let code = child.wait().await.ok().and_then(|status| status.code());
                let _ = sender.send(code);
            });
            Ok(LaunchOutcome::Running { pid, exit: ExitWatch(Arc::new(Mutex::new(Some(receiver)))) })
        }
    }
}

#[cfg(windows)]
fn detach(command: &mut tokio::process::Command) {
    use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW};
    command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(unix)]
fn detach(command: &mut tokio::process::Command) {
    command.process_group(0);
}

pub fn log_tail(path: &std::path::Path, lines: usize) -> String {
    let Ok(text) = std::fs::read_to_string(path) else { return String::new() };
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

fn newest_hs_err(dir: &std::path::Path) -> Option<PathBuf> {
    let cutoff = std::time::SystemTime::now() - Duration::from_secs(30);
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("hs_err_pid"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .filter(|(t, _)| *t >= cutoff)
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// Hints for common failure signatures in a client log.
pub fn diagnose(log_tail: &str) -> Option<&'static str> {
    if log_tail.contains("UnsupportedClassVersionError") {
        Some("Your Java is older than 21. Install Java 21 from Settings > Java.")
    } else if log_tail.contains("OutOfMemoryError") {
        Some("The client ran out of memory. Raise Max heap in Settings > Performance.")
    } else if log_tail.contains("UnsatisfiedLinkError") || log_tail.contains("libawt") {
        Some("Your Java build has no desktop (AWT) support. Install a full JRE, not a headless one.")
    } else if log_tail.contains("UnknownHostException") {
        Some("The server address could not be resolved. Check the address and your connection.")
    } else if log_tail.contains("unknown option") || log_tail.contains("usage: RuneTek5") {
        Some("The client did not understand the launcher's options; update the client from Settings > Updates.")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::{Arch, JavaOrigin};
    use crate::settings::{Extras, FpsCap, Graphics, Msaa, Renderer, Scale, Server};

    fn java(major: u32, vm: VmKind) -> JavaInstall {
        JavaInstall {
            home: PathBuf::from("/jdk"),
            exe: PathBuf::from("/jdk/bin/java"),
            major,
            full_version: format!("{major}.0.1"),
            vendor: "Test".into(),
            arch: Arch::X86_64,
            vm,
            headless: false,
            origin: JavaOrigin::System("test".into()),
            warnings: vec![],
        }
    }

    fn args_of(plan: &LaunchPlan) -> Vec<String> {
        plan.args.iter().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn heap_capping() {
        assert_eq!(effective_heap_mb(4096, Some(16_384)), (4096, false));
        assert_eq!(effective_heap_mb(4096, Some(4096)), (2457, true));
        assert_eq!(effective_heap_mb(512, Some(1024)), (1024, false));
        assert_eq!(effective_heap_mb(4096, None), (4096, false));
    }

    #[test]
    fn argument_order_matches_client_contract() {
        let paths = VoidPaths::with_home("/home/u");
        let settings = Settings {
            server: Server::Custom { host: "10.0.0.5".into(), port: 1234 },
            custom_keys: Some("/srv/game.properties".into()),
            renderer: Renderer::Hd,
            extras: Extras { shift_drop: false, ..Extras::default() },
            graphics: Graphics { fps: FpsCap::Fps60, scale: Scale::X1, msaa: Msaa::X4, smaa: true, ..Graphics::default() },
            world: 2,
            debug: true,
            max_heap_gb: 4,
            extra_jvm_args: "-Dfoo=1 -Dwindow.size=1280x900".into(),
            extra_client_args: "--lang german".into(),
            ..Settings::default()
        };
        let plan = build_plan_with_memory(&settings, &java(21, VmKind::HotSpot), &paths, Some(32_000));
        // Rendered by the OS (`\` on Windows), so compare against the same rendering.
        let jar = current_jar(&paths).to_string_lossy().into_owned();
        assert_eq!(jar.replace('\\', "/"), "/home/u/.void/client/runetek5-634.jar");
        assert_eq!(
            args_of(&plan),
            vec![
                "-Xms1024m",
                "-Xmx4096m",
                "-XX:+UseZGC",
                "-Dfoo=1",
                "-Dwindow.size=1280x900",
                "-jar",
                jar.as_str(),
                "--address",
                "10.0.0.5",
                "--port",
                "1234",
                "--world",
                "2",
                "--keys",
                "/srv/game.properties",
                "--toolkit",
                "hd",
                "--no-shift-drop",
                "--fps",
                "60",
                "--scale",
                "1",
                "--msaa",
                "4",
                "--smaa",
                "--debug",
                "--lang",
                "german",
            ]
        );
        assert_eq!(plan.cwd, PathBuf::from("/home/u/.void"));
        assert_eq!((plan.host.as_str(), plan.port), ("10.0.0.5", 1234));
    }

    #[test]
    fn defaults_are_minimal() {
        let paths = VoidPaths::with_home("/home/u");
        let settings = Settings::default();
        let plan = build_plan_with_memory(&settings, &java(21, VmKind::OpenJ9), &paths, None);
        let args = args_of(&plan);
        assert!(!args.iter().any(|a| a == "-XX:+UseZGC"));
        assert!(!args.iter().any(|a| a == "--toolkit" || a == "--world" || a == "--keys" || a == "--debug" || a.starts_with("--no-") || a == "--fps"));
        assert_eq!(&args[args.len() - 4..], ["--address", "play.2011.rs", "--port", "43594"]);
        assert_eq!(args[0], "-Xms1024m");
        assert_eq!(args[1], "-Xmx2048m");
    }

    #[test]
    fn keys_are_ignored_for_known_servers() {
        let paths = VoidPaths::with_home("/home/u");
        let settings = Settings { server: Server::Local, custom_keys: Some("/k".into()), renderer: Renderer::Safe, ..Settings::default() };
        let args = args_of(&build_plan_with_memory(&settings, &java(21, VmKind::HotSpot), &paths, None));
        assert!(!args.iter().any(|a| a == "--keys"));
        assert!(args.windows(2).any(|w| w == ["--address", "localhost"]));
        assert!(args.windows(2).any(|w| w == ["--toolkit", "java"]));
    }

    fn plugins_with(jvm: &str, client: &str) -> Option<PathBuf> {
        let paths = VoidPaths::with_home("/home/u");
        let settings = Settings { extra_jvm_args: jvm.into(), extra_client_args: client.into(), ..Settings::default() };
        plugins_dir(&settings, &paths)
    }

    #[test]
    fn plugins_dir_defaults_to_the_client_folder_under_home() {
        assert_eq!(plugins_with("", ""), Some(PathBuf::from("/home/u/.634-client/plugins")));
        assert_eq!(plugins_with("-Dfoo=1 -Dclient.fps=60", "--lang german"), Some(PathBuf::from("/home/u/.634-client/plugins")));
    }

    #[test]
    fn plugins_dir_follows_a_user_home_override() {
        assert_eq!(plugins_with("-Duser.home=/tmp/scratch", ""), Some(PathBuf::from("/tmp/scratch/.634-client/plugins")));
        assert_eq!(plugins_with("-Duser.home=/a -Duser.home=/b", ""), Some(PathBuf::from("/b/.634-client/plugins")), "last wins");
        assert_eq!(plugins_with("-Duser.home=", ""), Some(PathBuf::from("/home/u/.634-client/plugins")), "empty is ignored");
        assert_eq!(plugins_with("-Duser.home=/tmp/scratch", "--plugins ~/mods"), Some(PathBuf::from("/tmp/scratch/mods")), "~ is the overridden home");
    }

    #[test]
    fn plugins_dir_honours_the_client_switches() {
        assert_eq!(plugins_with("", "--plugins /srv/mods"), Some(PathBuf::from("/srv/mods")));
        assert_eq!(plugins_with("", "--plugins ~/mods"), Some(PathBuf::from("/home/u/mods")));
        assert_eq!(plugins_with("", "--plugins ~"), Some(PathBuf::from("/home/u")));
        assert_eq!(plugins_with("", "--plugins mods"), Some(PathBuf::from("/home/u/.void/mods")), "relative to the client's cwd");
        assert_eq!(plugins_with("", "--plugins \"/my mods\""), Some(PathBuf::from("/my mods")), "quoted like a shell");
        assert_eq!(plugins_with("", "--no-plugins"), None);
        assert_eq!(plugins_with("", "--plugins /a --no-plugins"), None, "last wins");
        assert_eq!(plugins_with("", "--no-plugins --plugins /a"), Some(PathBuf::from("/a")), "last wins");
        assert_eq!(plugins_with("", "--plugins"), None, "a missing value starts nothing");
    }

    #[test]
    fn plugins_dir_honours_the_client_property() {
        assert_eq!(plugins_with("-Dclient.plugins=/srv/mods", ""), Some(PathBuf::from("/srv/mods")));
        assert_eq!(plugins_with("-Dclient.plugins=~/mods", ""), Some(PathBuf::from("/home/u/mods")));
        assert_eq!(plugins_with("-Dclient.plugins=none", ""), None);
        assert_eq!(plugins_with("-Dclient.plugins=NONE", ""), None, "case-insensitive like the client");
        assert_eq!(plugins_with("-Dclient.plugins=", ""), None, "empty means none");
        assert_eq!(plugins_with("-Dclient.plugins=none", "--plugins /a"), Some(PathBuf::from("/a")), "the command line beats the property");
        assert_eq!(plugins_with("-Dclient.plugins=/a", "--no-plugins"), None);
    }

    #[test]
    fn diagnoses_common_failures() {
        assert!(diagnose("java.lang.UnsupportedClassVersionError: x").unwrap().contains("older than 21"));
        assert!(diagnose("usage: RuneTek5 [--address host]").unwrap().contains("update the client"));
        assert!(diagnose("all good").is_none());
    }
}
