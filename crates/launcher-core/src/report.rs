//! Crash pickup and the bug-report text a player pastes into a GitHub issue.
//!
//! The JVM leaves `hs_err_pid*.log` in its working directory (`~/.void`) when
//! it crashes natively; the launcher's panic hook writes
//! `logs/launcher-crash-<time>.txt`. Both are found here and turned into one
//! short, fenced block of text, plus a headline, an optional hint and a
//! pre-filled "new issue" link.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::CLIENT_REPO;
use crate::github;
use crate::launch;
use crate::paths::VoidPaths;
use crate::version::LauncherBuild;

/// Keeps the pasted report short enough to read in an issue (and to fit in a
/// pre-filled issue URL); the full file stays on disk for when it is needed.
pub const TEXT_BUDGET: usize = 1800;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashKind {
    /// The Java VM crashed natively (graphics driver, out of native memory).
    Jvm,
    /// A previous run of this launcher panicked.
    Launcher,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashFile {
    pub path: PathBuf,
    pub kind: CrashKind,
    pub modified: SystemTime,
}

impl CrashFile {
    pub fn file_name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    pub fn modified_rfc3339(&self) -> String {
        time::OffsetDateTime::from(self.modified).format(&time::format_description::well_known::Rfc3339).unwrap_or_default()
    }
}

/// Every crash file on disk, newest first.
pub fn find_crash_files(paths: &VoidPaths) -> Vec<CrashFile> {
    let mut found = Vec::new();
    collect(&paths.logs_dir(), "launcher-crash-", ".txt", CrashKind::Launcher, &mut found);
    collect(&paths.root, "hs_err_pid", ".log", CrashKind::Jvm, &mut found);
    found.sort_by(|a, b| b.modified.cmp(&a.modified));
    found
}

/// The newest crash file the player has not been shown yet.
pub fn newest_unseen(paths: &VoidPaths, seen_until: Option<&str>) -> Option<CrashFile> {
    let seen_until = seen_until.and_then(|s| time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()).map(SystemTime::from);
    find_crash_files(paths).into_iter().find(|f| seen_until.is_none_or(|seen| f.modified > seen))
}

fn collect(dir: &Path, prefix: &str, suffix: &str, kind: CrashKind, out: &mut Vec<CrashFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with(prefix) && name.ends_with(suffix)) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else { continue };
        if meta.len() == 0 {
            continue;
        }
        out.push(CrashFile { path: entry.path(), kind, modified });
    }
}

/// What the launcher knows about the machine, prepended to every report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReportContext {
    pub launcher: String,
    pub channel: String,
    pub client: String,
    pub java: String,
    pub os: String,
    pub server: String,
    pub renderer: String,
    pub heap_gb: u8,
}

impl ReportContext {
    fn lines(&self) -> String {
        format!(
            "Launcher {} · {} channel · client {}\n{} · heap {} GB · renderer {}\n{} · {}",
            self.launcher, self.channel, self.client, self.java, self.heap_gb, self.renderer, self.os, self.server
        )
    }
}

/// Platform and architecture only. Querying the exact OS version would mean
/// reading the Windows registry, a behaviour antivirus heuristics score
/// against unsigned programs.
pub fn os_description() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Everything the bug-report dialog shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BugReport {
    pub headline: String,
    /// The issue title the "Open an issue" button pre-fills.
    pub title: String,
    /// One fenced block, at most [`TEXT_BUDGET`] characters.
    pub text: String,
    pub file: Option<PathBuf>,
    pub hint: Option<&'static str>,
}

impl BugReport {
    /// A "new issue" page on the client repository with the report filled in.
    pub fn issue_url(&self) -> String {
        github::issue_url(CLIENT_REPO, &self.title, &self.text)
    }
}

/// Builds the report for a crash file left by the JVM or the launcher.
pub fn from_crash_file(file: &CrashFile, ctx: &ReportContext) -> BugReport {
    let contents = std::fs::read_to_string(&file.path).unwrap_or_default();
    let (headline, body) = match file.kind {
        CrashKind::Jvm => ("The game closed unexpectedly last time it ran".to_string(), jvm_summary(&contents)),
        CrashKind::Launcher => ("The launcher crashed last time it ran".to_string(), launcher_summary(&contents)),
    };
    let title = match file.kind {
        CrashKind::Jvm => "Void client crash (Java VM crash)",
        CrashKind::Launcher => "Void launcher crash",
    };
    let hint = diagnose(&contents);
    BugReport { headline, title: title.into(), text: fence(title, ctx, &body), file: Some(file.path.clone()), hint }
}

/// The client exited within seconds of starting.
pub fn from_launch_failure(code: Option<i32>, log_tail: &str, log_path: &Path, ctx: &ReportContext) -> BugReport {
    let body = match code {
        Some(c) => format!("Client exited with code {c} right after starting.\n\n{}", trim_lines(log_tail, 40)),
        None => format!("Client could not be started.\n\n{}", trim_lines(log_tail, 40)),
    };
    let title = "Void client failed to start";
    BugReport {
        headline: "The client closed unexpectedly".into(),
        title: title.into(),
        text: fence(title, ctx, &body),
        file: Some(log_path.to_path_buf()),
        hint: launch::diagnose(log_tail),
    }
}

/// Nothing crashed; the player wants to report something they saw.
pub fn from_logs(paths: &VoidPaths, ctx: &ReportContext) -> BugReport {
    let client_log = newest_client_log(paths);
    let mut body = String::new();
    match &client_log {
        Some(path) => {
            body.push_str(&format!("Last client log ({}):\n", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
            body.push_str(&trim_lines(&launch::log_tail(path, 30), 30));
        }
        None => body.push_str("No client log found yet."),
    }
    body.push_str("\n\nLauncher log:\n");
    body.push_str(&trim_lines(&launch::log_tail(&paths.launcher_log(), 8), 8));
    let title = "Void client bug report";
    BugReport { headline: "Report a problem".into(), title: title.into(), text: fence(title, ctx, &body), file: client_log, hint: None }
}

pub fn newest_client_log(paths: &VoidPaths) -> Option<PathBuf> {
    let entries = std::fs::read_dir(paths.logs_dir()).ok()?;
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with("client-") && name.ends_with(".log")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// Hints for signatures the launcher can recognise in any crash text.
pub fn diagnose(text: &str) -> Option<&'static str> {
    if let Some(hint) = launch::diagnose(text) {
        return Some(hint);
    }
    let lower = text.to_ascii_lowercase();
    let graphics = ["nvoglv", "libnvidia", "ig9icd", "ig11icd", "igdrcl", "amdvlk", "atio6axx", "atioglxx", "libgl.so", "libglx", "radeonsi", "mesa"];
    if graphics.iter().any(|s| lower.contains(s)) {
        return Some("This looks like a graphics driver crash. Update your graphics driver, or pick Safe mode under Renderer and try again.");
    }
    if lower.contains("out of memory") || lower.contains("native memory allocation") {
        return Some("The computer ran out of memory. Close other programs or lower Max heap in Settings > Performance.");
    }
    None
}

/// The useful part of a JVM `hs_err` file: the `#` header block and the top frames.
fn jvm_summary(contents: &str) -> String {
    let mut out = Vec::new();
    for line in contents.lines().take(40) {
        if line.starts_with('#') {
            let trimmed = line.trim_start_matches('#').trim();
            if !trimmed.is_empty() && !trimmed.starts_with("If you would like") && !trimmed.contains("bugreport") && !trimmed.contains("crash reporting") {
                out.push(trimmed.to_string());
            }
        }
    }
    let mut frames = 0;
    let mut in_frames = false;
    for line in contents.lines() {
        if line.starts_with("Native frames:") || line.starts_with("Java frames:") {
            in_frames = true;
            out.push(line.to_string());
            continue;
        }
        if in_frames {
            if line.trim().is_empty() {
                in_frames = false;
                continue;
            }
            out.push(line.trim_end().to_string());
            frames += 1;
            if frames >= 10 {
                break;
            }
        }
    }
    out.join("\n")
}

fn launcher_summary(contents: &str) -> String {
    trim_lines(contents, 30)
}

fn trim_lines(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].iter().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
}

/// Wraps the context and body in a code block and trims the body to the budget.
fn fence(title: &str, ctx: &ReportContext, body: &str) -> String {
    let head = format!("```\n{title}\n{}\n\n", ctx.lines());
    let tail = "\n```";
    let room = TEXT_BUDGET.saturating_sub(head.chars().count() + tail.len());
    let body = if body.chars().count() > room {
        let cut: String = body.chars().take(room.saturating_sub(2)).collect();
        // Never cut in the middle of a line.
        match cut.rfind('\n') {
            Some(at) if at > room / 2 => format!("{}\n…", &cut[..at]),
            _ => format!("{cut}…"),
        }
    } else {
        body.to_string()
    };
    format!("{head}{}{tail}", body.trim_end())
}

/// Writes a `launcher-crash-<time>.txt` for every panic so the next launcher run
/// can offer it to the player; the default hook still prints to stderr.
pub fn install_panic_hook(paths: VoidPaths, build: LauncherBuild) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let stamp = time::OffsetDateTime::now_utc()
            .format(&time::macros::format_description!("[year][month][day]-[hour][minute][second]"))
            .unwrap_or_else(|_| "now".into());
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let short_backtrace: String = backtrace.lines().take(24).collect::<Vec<_>>().join("\n");
        let report = format!(
            "Void launcher crash\nLauncher {} ({}) {} built {}\n{}\n\n{info}\n\nBacktrace:\n{short_backtrace}\n\nFull backtrace:\n{backtrace}\n",
            build.version,
            build.commit,
            build.target,
            build.built_at,
            os_description()
        );
        let _ = std::fs::create_dir_all(paths.logs_dir());
        let path = paths.logs_dir().join(format!("launcher-crash-{stamp}.txt"));
        let _ = std::fs::write(&path, report);
        tracing::error!("launcher panicked: {info}; report at {}", path.display());
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ReportContext {
        ReportContext {
            launcher: "1.0.0".into(),
            channel: "Stable".into(),
            client: "2.8.0".into(),
            java: "Java 21.0.4 (managed)".into(),
            os: "linux x86_64".into(),
            server: "play.2011.rs:43594".into(),
            renderer: "634HD".into(),
            heap_gb: 2,
        }
    }

    #[test]
    fn jvm_summary_keeps_header_and_frames() {
        let hs_err = "#\n# A fatal error has been detected by the Java Runtime Environment:\n#\n#  EXCEPTION_ACCESS_VIOLATION (0xc0000005) at pc=0x1\n#\n# Problematic frame:\n# C  [nvoglv64.dll+0x123]\n#\n# If you would like to submit a bug report, please visit:\n#   https://bugreport.java.com/\n\nStack: [0x1,0x2]\nNative frames: (J=compiled Java code)\nC  [nvoglv64.dll+0x123]\nj  org.lwjgl.opengl.GL11C.nglDrawElements\n\nJava frames:\n";
        let summary = jvm_summary(hs_err);
        assert!(summary.contains("EXCEPTION_ACCESS_VIOLATION"));
        assert!(summary.contains("Problematic frame"));
        assert!(summary.contains("nglDrawElements"));
        assert!(!summary.contains("bugreport.java.com"));
        assert!(diagnose(hs_err).unwrap().contains("graphics driver"));
    }

    #[test]
    fn fence_respects_budget_and_issue_url_carries_it() {
        let body: String = (0..400).map(|i| format!("line {i} with some padding text\n")).collect();
        let text = fence("Void client crash", &ctx(), &body);
        assert!(text.chars().count() <= TEXT_BUDGET, "{}", text.chars().count());
        assert!(text.starts_with("```\nVoid client crash\nLauncher 1.0.0"));
        assert!(text.ends_with("…\n```"));
        let report = BugReport { headline: "x".into(), title: "Void client crash".into(), text, file: None, hint: None };
        let url = report.issue_url();
        assert!(url.starts_with("https://github.com/2011Scape/634-client/issues/new?title=Void+client+crash&body=%60%60%60"));
    }

    #[test]
    fn finds_and_orders_crash_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = VoidPaths::with_home(dir.path());
        paths.ensure_dirs().unwrap();
        std::fs::write(paths.logs_dir().join("launcher-crash-1.txt"), "old").unwrap();
        std::fs::write(paths.root.join("hs_err_pid42.log"), "# A fatal error").unwrap();
        std::fs::write(paths.logs_dir().join("client-x.log"), "not a crash").unwrap();
        std::fs::write(paths.logs_dir().join("launcher-crash-empty.txt"), "").unwrap();
        let files = find_crash_files(&paths);
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.kind == CrashKind::Jvm));
        assert!(files.iter().any(|f| f.kind == CrashKind::Launcher));

        let newest = newest_unseen(&paths, None).unwrap();
        let seen = newest.modified_rfc3339();
        assert!(newest_unseen(&paths, Some(&seen)).is_none());
    }
}
