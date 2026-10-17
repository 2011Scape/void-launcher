//! UI-side state derived from the core services.

use std::path::PathBuf;

use iced::task;
use launcher_core::client::{ClientState as InstalledClient, RemoteClient};
use launcher_core::java::{JavaInstall, JavaProblem};
use launcher_core::progress::{Progress, format_bytes};
use launcher_core::report::CrashFile;
use launcher_core::self_update::LauncherUpdate;
use launcher_core::settings::{JavaSource, Server};

use crate::message::Message;
use crate::theme::style::NoticeLevel;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProgressView {
    pub phase: String,
    pub downloaded: u64,
    pub total: Option<u64>,
}

impl ProgressView {
    pub fn new(phase: impl Into<String>) -> Self {
        Self { phase: phase.into(), downloaded: 0, total: None }
    }

    pub fn apply(&mut self, progress: Progress) {
        match progress {
            Progress::Phase(name) => {
                self.phase = name;
                self.downloaded = 0;
                self.total = None;
            }
            Progress::Bytes { downloaded, total } => {
                self.downloaded = downloaded;
                self.total = total;
            }
        }
    }

    pub fn fraction(&self) -> f32 {
        match self.total {
            Some(total) if total > 0 => (self.downloaded as f64 / total as f64).min(1.0) as f32,
            _ => 0.0,
        }
    }

    pub fn percent(&self) -> Option<u32> {
        self.total.filter(|t| *t > 0).map(|_| (self.fraction() * 100.0).round() as u32)
    }

    pub fn detail(&self) -> String {
        match self.total {
            Some(total) if total > 0 => format!("{} / {}", format_bytes(self.downloaded), format_bytes(total)),
            _ if self.downloaded > 0 => format_bytes(self.downloaded),
            _ => String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum JavaState {
    Checking,
    Ready(JavaInstall),
    Problem(JavaProblem),
    Installing(ProgressView),
    InstallFailed(String),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ClientState {
    Checking,
    Current(InstalledClient),
    UpdateAvailable { installed: Option<InstalledClient>, remote: RemoteClient },
    Downloading { remote: RemoteClient, installed: Option<InstalledClient>, progress: ProgressView, handle: task::Handle },
    Offline { installed: Option<InstalledClient>, reason: String },
    Failed { installed: Option<InstalledClient>, remote: Option<RemoteClient>, reason: String },
}

impl ClientState {
    pub fn installed(&self) -> Option<&InstalledClient> {
        match self {
            ClientState::Current(c) => Some(c),
            ClientState::UpdateAvailable { installed, .. }
            | ClientState::Downloading { installed, .. }
            | ClientState::Offline { installed, .. }
            | ClientState::Failed { installed, .. } => installed.as_ref(),
            ClientState::Checking => None,
        }
    }

    /// Whether Play can start the installed jar right now.
    pub fn playable(&self) -> bool {
        matches!(self, ClientState::Current(_)) || (matches!(self, ClientState::Offline { .. } | ClientState::Failed { .. }) && self.installed().is_some())
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SelfUpdateState {
    Checking,
    UpToDate,
    Available(LauncherUpdate),
    Staging { update: LauncherUpdate, progress: ProgressView },
    Staged { update: LauncherUpdate, path: PathBuf },
    Manual { version: String, url: String },
    Disabled(String),
    Offline(String),
    Skipped,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum LaunchState {
    Idle,
    Starting,
    Running { pid: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    None,
    Settings,
    JavaHelp,
    /// The crash / bug-report dialog; what it shows is in [`crate::app::App::report`].
    BugReport,
    LauncherUpdate,
    FirstRun,
}

/// What the bug-report dialog was opened for. The text itself is built on
/// demand from this plus whatever the launcher knows at that moment.
#[derive(Debug, Clone, PartialEq)]
pub enum ReportSource {
    /// A file the client, the JVM or an earlier launcher run left behind.
    CrashFile(CrashFile),
    /// The client died within seconds of Play.
    LaunchFailure { code: Option<i32>, log_tail: String, log_path: PathBuf },
    /// The player pressed "Report a problem"; nothing crashed.
    Logs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportView {
    pub source: ReportSource,
    pub copied: bool,
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub level: NoticeLevel,
    pub text: String,
    pub action: Option<(String, Message)>,
}

impl Notice {
    pub fn new(level: NoticeLevel, text: impl Into<String>) -> Self {
        Self { level, text: text.into(), action: None }
    }

    pub fn with_action(mut self, label: impl Into<String>, message: Message) -> Self {
        self.action = Some((label.into(), message));
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerKind {
    Official,
    Local,
    Custom,
}

impl ServerKind {
    pub const ALL: [ServerKind; 3] = [ServerKind::Official, ServerKind::Local, ServerKind::Custom];

    pub fn of(server: &Server) -> Self {
        match server {
            Server::Official => ServerKind::Official,
            Server::Local => ServerKind::Local,
            Server::Custom { .. } => ServerKind::Custom,
        }
    }
}

impl std::fmt::Display for ServerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerKind::Official => write!(f, "2011Scape  ·  {}", launcher_core::settings::OFFICIAL_HOST),
            ServerKind::Local => write!(f, "Local  ·  {}", launcher_core::settings::LOCAL_HOST),
            ServerKind::Custom => write!(f, "Custom server…"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaSourceKind {
    Managed,
    System,
    Custom,
}

impl JavaSourceKind {
    pub fn of(source: &JavaSource) -> Self {
        match source {
            JavaSource::Managed => JavaSourceKind::Managed,
            JavaSource::System => JavaSourceKind::System,
            JavaSource::Custom { .. } => JavaSourceKind::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapChoice(pub u8);

impl std::fmt::Display for HeapChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} GB", self.0)
    }
}

/// Text-field mirrors of the settings that need validation before they apply.
#[derive(Debug, Clone, Default)]
pub struct Form {
    pub custom_host: String,
    pub custom_port: String,
    pub custom_java_path: String,
    pub world: String,
    pub server_error: Option<String>,
    pub world_error: Option<String>,
}
