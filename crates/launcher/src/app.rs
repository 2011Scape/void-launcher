//! The launcher state machine.

use std::time::{Duration, Instant};

use iced::{Element, Subscription, Task, keyboard};
use launcher_core::client::ClientStatus;
use launcher_core::http::Http;
use launcher_core::java::JavaProblem;
use launcher_core::launch::{self, LaunchOutcome};
use launcher_core::logging::LogGuard;
use launcher_core::paths::{VoidPaths, open_folder};
use launcher_core::report::{self, BugReport, ReportContext};
use launcher_core::self_update::UpdateStatus;
use launcher_core::settings::{AfterLaunch, JavaSource, Server, Settings, validate_host, validate_port, validate_world};
use launcher_core::version::{Channel, LauncherBuild};
use launcher_core::{logging, startup};

use crate::links::open_url;
use crate::message::Message;
use crate::state::{
    ClientState, Form, JavaSourceKind, JavaState, LaunchState, Notice, Panel, ProgressView, ReportSource, ReportView, SelfUpdateState, ServerKind,
};
use crate::tasks;
use crate::theme::style::NoticeLevel;

/// How long a client check is trusted before Play asks GitHub again.
const CLIENT_CHECK_FRESH: Duration = Duration::from_secs(5 * 60);

pub const WINDOW_WIDTH: f32 = 960.0;
pub const WINDOW_HEIGHT: f32 = 600.0;

pub struct App {
    pub paths: VoidPaths,
    pub http: Http,
    pub settings: Settings,
    pub build: LauncherBuild,
    pub form: Form,
    pub java: JavaState,
    pub client: ClientState,
    pub self_update: SelfUpdateState,
    pub launch: LaunchState,
    pub panel: Panel,
    /// What the bug-report dialog is about, while it is open.
    pub report: Option<ReportView>,
    pub notice: Option<Notice>,
    /// Play was pressed while something was still in flight; continue when it lands.
    pub pending_launch: bool,
    /// The first-run "Get started" flow is driving installs.
    pub onboarding: bool,
    /// When the last client check landed, so a launcher left open across a
    /// release does not keep playing the jar it checked at startup.
    pub client_checked_at: Option<Instant>,
    pub tick: u32,
    _log: LogGuard,
}

pub fn build_identity() -> LauncherBuild {
    let channel = match env!("VOID_CHANNEL") {
        "stable" => Channel::Stable,
        _ => Channel::Development,
    };
    LauncherBuild {
        version: env!("VOID_VERSION").to_string(),
        channel,
        commit: env!("VOID_COMMIT").to_string(),
        built_at: env!("VOID_BUILT_AT").to_string(),
        target: env!("VOID_TARGET").to_string(),
    }
}

impl App {
    pub fn boot() -> (App, Task<Message>) {
        let paths = VoidPaths::resolve().unwrap_or_else(|_| VoidPaths::with_home(std::env::temp_dir()));
        let log = logging::init(&paths);
        let build = build_identity();
        tracing::info!("Void launcher {} ({}) {} on {}", build.version, build.commit, build.built_at, build.target);
        startup::prepare(&paths);
        let mut settings = Settings::load(&paths);
        tracing::info!(
            "settings: channel={} server={}:{} renderer={:?} world={} java={:?}",
            settings.channel,
            settings.server.host(),
            settings.server.port(),
            settings.renderer,
            settings.world,
            settings.java
        );
        let http = Http::new(&format!("void-launcher/{} ({})", build.version, build.target)).expect("http client");

        let form = Form {
            custom_host: settings.custom_host.clone(),
            custom_port: settings.custom_port.to_string(),
            custom_java_path: match &settings.java {
                JavaSource::Custom { path } => path.to_string_lossy().into_owned(),
                _ => String::new(),
            },
            world: settings.world.to_string(),
            server_error: None,
            world_error: None,
        };
        // A crash from the previous session outranks everything else on screen.
        let crash = report::newest_unseen(&paths, settings.last_crash_seen.as_deref());
        if let Some(crash) = &crash {
            tracing::info!("crash file from a previous run: {}", crash.path.display());
            settings.last_crash_seen = Some(crash.modified_rfc3339());
        }
        let mut panel = match (&crash, settings.first_run_completed) {
            (Some(_), _) => Panel::BugReport,
            (None, true) => Panel::None,
            (None, false) => Panel::FirstRun,
        };
        // Debug builds can open a panel at boot for headless UI checks (`VOID_START_PANEL=settings|java|update|report`).
        if cfg!(debug_assertions)
            && let Ok(name) = std::env::var("VOID_START_PANEL")
        {
            panel = match name.as_str() {
                "settings" => Panel::Settings,
                "java" => Panel::JavaHelp,
                "update" => Panel::LauncherUpdate,
                "report" => Panel::BugReport,
                "welcome" => Panel::FirstRun,
                _ => panel,
            };
        }
        let crash_shown = crash.is_some();
        let report = match crash {
            Some(file) => Some(ReportView { source: ReportSource::CrashFile(file), copied: false }),
            None if panel == Panel::BugReport => Some(ReportView { source: ReportSource::Logs, copied: false }),
            None => None,
        };
        let mut app = App {
            paths,
            http,
            settings,
            build,
            form,
            java: JavaState::Checking,
            client: ClientState::Checking,
            self_update: SelfUpdateState::Checking,
            launch: LaunchState::Idle,
            panel,
            report,
            notice: None,
            pending_launch: false,
            onboarding: false,
            client_checked_at: None,
            tick: 0,
            _log: log,
        };
        let checks = app.run_all_checks();
        let task = if crash_shown { Task::batch([app.save(), checks]) } else { checks };
        (app, task)
    }

    pub fn title(&self) -> String {
        match &self.client {
            ClientState::Downloading { progress, .. } => match progress.percent() {
                Some(p) => format!("Void Launcher — Downloading {p}%"),
                None => "Void Launcher — Downloading".into(),
            },
            _ => "Void Launcher".into(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let busy = matches!(self.java, JavaState::Checking | JavaState::Installing(_))
            || matches!(self.client, ClientState::Checking | ClientState::Downloading { .. })
            || matches!(self.launch, LaunchState::Starting);
        let tick = if busy { iced::time::every(Duration::from_millis(400)).map(|_| Message::Tick) } else { Subscription::none() };
        Subscription::batch([keyboard::listen().map(Message::Keyboard), tick])
    }

    fn run_all_checks(&mut self) -> Task<Message> {
        self.java = JavaState::Checking;
        self.client = ClientState::Checking;
        let launcher_check = if self.settings.check_updates {
            self.self_update = SelfUpdateState::Checking;
            tasks::check_launcher(self.http.clone(), self.build.clone(), self.settings.channel)
        } else {
            self.self_update = SelfUpdateState::Skipped;
            Task::none()
        };
        Task::batch([
            tasks::check_java(self.settings.clone(), self.paths.clone()),
            tasks::check_client(self.http.clone(), self.paths.clone(), self.settings.channel),
            launcher_check,
        ])
    }

    fn save(&self) -> Task<Message> {
        tasks::save_settings(self.settings.clone(), self.paths.clone())
    }

    fn notify(&mut self, level: NoticeLevel, text: impl Into<String>) {
        self.notice = Some(Notice::new(level, text));
    }

    /// What the launcher knows about this machine, for the top of every bug report.
    pub fn report_context(&self) -> ReportContext {
        ReportContext {
            launcher: format!("{} ({}) {}", self.build.version, self.build.short_commit(), self.build.target),
            channel: self.settings.channel.label().into(),
            client: self.client.installed().map(|c| c.label()).unwrap_or_else(|| "not installed".into()),
            java: match &self.java {
                JavaState::Ready(java) => java.summary(),
                JavaState::Problem(problem) => format!("Java problem: {problem}"),
                _ => "Java unknown".into(),
            },
            os: report::os_description(),
            server: format!("{}:{} world {}", self.settings.server.host(), self.settings.server.port(), self.settings.world),
            renderer: self.settings.renderer.label().into(),
            heap_gb: self.settings.max_heap_gb,
        }
    }

    /// The report the dialog shows and copies, built from [`Self::report`] and the current context.
    pub fn current_report(&self) -> BugReport {
        let ctx = self.report_context();
        match self.report.as_ref().map(|view| &view.source) {
            Some(ReportSource::CrashFile(file)) => report::from_crash_file(file, &ctx),
            Some(ReportSource::LaunchFailure { code, log_tail, log_path }) => report::from_launch_failure(*code, log_tail, log_path, &ctx),
            Some(ReportSource::Logs) | None => report::from_logs(&self.paths, &ctx),
        }
    }

    fn open_report(&mut self, source: ReportSource) {
        self.report = Some(ReportView { source, copied: false });
        self.panel = Panel::BugReport;
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::JavaResolved(result) => {
                match result {
                    Ok(install) => {
                        tracing::info!("java: {}", install.summary());
                        self.java = JavaState::Ready(install);
                        return self.continue_flows();
                    }
                    Err(problem) => {
                        tracing::warn!("java: {problem}");
                        self.java = JavaState::Problem(problem);
                        if self.onboarding {
                            return self.update(Message::InstallJava);
                        }
                        if self.pending_launch {
                            self.pending_launch = false;
                            self.panel = Panel::JavaHelp;
                        }
                    }
                }
                Task::none()
            }
            Message::ClientChecked(status) => {
                self.client_checked_at = Some(Instant::now());
                self.client = match status {
                    ClientStatus::Current(state) => ClientState::Current(state),
                    ClientStatus::UpdateAvailable { installed, remote } => ClientState::UpdateAvailable { installed, remote },
                    ClientStatus::Offline { installed, reason } => {
                        if installed.is_some() {
                            self.notice = Some(
                                Notice::new(NoticeLevel::Warning, "Couldn't reach github.com to check for client updates. Playing the installed version.")
                                    .with_action("Retry", Message::RetryClient),
                            );
                        }
                        ClientState::Offline { installed, reason }
                    }
                };
                self.continue_flows()
            }
            Message::LauncherChecked(status) => {
                self.self_update = match status {
                    UpdateStatus::UpToDate => SelfUpdateState::UpToDate,
                    UpdateStatus::Available(update) => {
                        let should_prompt =
                            self.settings.channel == Channel::Development || update.required || prompt_due(self.settings.last_update_prompt.as_deref());
                        if should_prompt && self.panel == Panel::None {
                            self.panel = Panel::LauncherUpdate;
                            self.settings.last_update_prompt = Some(launcher_core::client::now_rfc3339());
                            let save = self.save();
                            self.self_update = SelfUpdateState::Available(update);
                            return save;
                        }
                        SelfUpdateState::Available(update)
                    }
                    UpdateStatus::ManualUpdateRequired { version, url } => SelfUpdateState::Manual { version, url },
                    UpdateStatus::NoBuildForPlatform => SelfUpdateState::UpToDate,
                    UpdateStatus::Disabled(by) => SelfUpdateState::Disabled(by),
                    UpdateStatus::Offline(reason) => SelfUpdateState::Offline(reason),
                    UpdateStatus::Skipped => SelfUpdateState::Skipped,
                };
                Task::none()
            }

            Message::ServerPicked(kind) => {
                self.form.server_error = None;
                self.settings.server = match kind {
                    ServerKind::Official => Server::Official,
                    ServerKind::Local => Server::Local,
                    ServerKind::Custom => Server::Custom { host: self.settings.custom_host.clone(), port: self.settings.custom_port },
                };
                self.save()
            }
            Message::CustomHostChanged(host) => {
                self.form.custom_host = host;
                self.form.server_error = None;
                Task::none()
            }
            Message::CustomPortChanged(port) => {
                self.form.custom_port = port;
                self.form.server_error = None;
                Task::none()
            }
            Message::BrowseKeysFile => tasks::pick_keys_file(),
            Message::KeysFilePicked(path) => {
                let Some(path) = path else { return Task::none() };
                self.settings.custom_keys = Some(path);
                self.save()
            }
            Message::ClearKeysFile => {
                self.settings.custom_keys = None;
                self.save()
            }
            Message::RendererPicked(renderer) => {
                self.settings.renderer = renderer;
                self.save()
            }
            Message::ExtraToggled(extra, on) => {
                self.settings.extras.set(extra, on);
                self.save()
            }
            Message::RenderDistancePicked(v) => {
                self.settings.graphics.render_distance = v;
                self.save()
            }
            Message::FpsPicked(v) => {
                self.settings.graphics.fps = v;
                self.save()
            }
            Message::ScalePicked(v) => {
                self.settings.graphics.scale = v;
                self.save()
            }
            Message::MsaaPicked(v) => {
                self.settings.graphics.msaa = v;
                self.save()
            }
            Message::SmaaToggled(on) => {
                self.settings.graphics.smaa = on;
                self.save()
            }
            Message::RenderScalePicked(v) => {
                self.settings.graphics.render_scale = v;
                self.save()
            }
            Message::DebugToggled(on) => {
                self.settings.debug = on;
                self.save()
            }
            Message::ExtraClientArgsChanged(args) => {
                self.settings.extra_client_args = args;
                self.save()
            }
            Message::WorldChanged(world) => {
                self.form.world = world;
                match validate_world(&self.form.world) {
                    Ok(w) => {
                        self.form.world_error = None;
                        if self.settings.world != w {
                            self.settings.world = w;
                            return self.save();
                        }
                        Task::none()
                    }
                    Err(e) => {
                        self.form.world_error = Some(e);
                        Task::none()
                    }
                }
            }

            Message::PlayPressed => {
                if self.panel != Panel::None {
                    return Task::none();
                }
                if let Err(e) = self.commit_custom_server() {
                    self.form.server_error = Some(e);
                    return Task::none();
                }
                self.notice = None;
                self.pending_launch = true;
                let save = self.save();
                let recheck = self.recheck_client(true);
                Task::batch([save, recheck, self.continue_flows()])
            }
            Message::CancelDownload => {
                if let ClientState::Downloading { remote, installed, handle, .. } = &self.client {
                    handle.abort();
                    self.client = ClientState::UpdateAvailable { installed: installed.clone(), remote: remote.clone() };
                }
                self.pending_launch = false;
                self.onboarding = false;
                Task::none()
            }
            Message::RetryClient => {
                self.notice = None;
                self.client = ClientState::Checking;
                tasks::check_client(self.http.clone(), self.paths.clone(), self.settings.channel)
            }
            Message::OpenLogsFolder => {
                if let Err(e) = open_folder(&self.paths.logs_dir()) {
                    self.notify(NoticeLevel::Error, format!("Couldn't open the logs folder: {e}"));
                }
                Task::none()
            }
            Message::OpenPluginsFolder => {
                match launch::plugins_dir(&self.settings, &self.paths) {
                    Some(dir) => {
                        if let Err(e) = open_folder(&dir) {
                            self.notify(NoticeLevel::Error, format!("Couldn't open the plugins folder: {e}"));
                        }
                    }
                    None => self.notify(NoticeLevel::Info, "Plugins are switched off by --no-plugins in the extra client arguments."),
                }
                Task::none()
            }
            Message::OpenLink(link) => {
                open_url(&link.url());
                Task::none()
            }
            Message::OpenUrl(url) => {
                open_url(&url);
                Task::none()
            }
            Message::CopyText(text) => iced::clipboard::write(text),
            Message::CheckForUpdates => {
                self.notice = None;
                self.settings.check_updates = true;
                self.client = ClientState::Checking;
                self.self_update = SelfUpdateState::Checking;
                Task::batch([
                    tasks::check_client(self.http.clone(), self.paths.clone(), self.settings.channel),
                    tasks::check_launcher(self.http.clone(), self.build.clone(), self.settings.channel),
                ])
            }

            Message::InstallJava => {
                if matches!(self.java, JavaState::Installing(_)) {
                    return Task::none();
                }
                self.settings.java = JavaSource::Managed;
                self.java = JavaState::Installing(ProgressView::new("Preparing"));
                Task::batch([self.save(), tasks::install_java(self.http.clone(), self.paths.clone())])
            }
            Message::JavaInstallProgress(p) => {
                if let JavaState::Installing(view) = &mut self.java {
                    view.apply(p);
                }
                Task::none()
            }
            Message::JavaInstalled(result) => match result {
                Ok(info) => {
                    tracing::info!("installed managed runtime {}", info.release_name);
                    self.notify(NoticeLevel::Success, format!("Java {} installed.", info.semver));
                    if self.panel == Panel::JavaHelp {
                        self.panel = Panel::None;
                    }
                    self.java = JavaState::Checking;
                    tasks::check_java(self.settings.clone(), self.paths.clone())
                }
                Err(e) => {
                    self.java = JavaState::InstallFailed(e);
                    self.onboarding = false;
                    self.pending_launch = false;
                    if self.panel == Panel::FirstRun {
                        self.panel = Panel::JavaHelp;
                    }
                    Task::none()
                }
            },
            Message::RecheckJava => {
                self.java = JavaState::Checking;
                tasks::check_java(self.settings.clone(), self.paths.clone())
            }
            Message::JavaSourcePicked(kind) => {
                self.settings.java = match kind {
                    JavaSourceKind::Managed => JavaSource::Managed,
                    JavaSourceKind::System => JavaSource::System,
                    JavaSourceKind::Custom => JavaSource::Custom { path: self.form.custom_java_path.clone().into() },
                };
                self.java = JavaState::Checking;
                Task::batch([self.save(), tasks::check_java(self.settings.clone(), self.paths.clone())])
            }
            Message::CustomJavaPathChanged(path) => {
                self.form.custom_java_path = path;
                Task::none()
            }
            Message::BrowseJavaPath => tasks::pick_java_path(),
            Message::JavaPathPicked(path) => {
                let Some(path) = path else { return Task::none() };
                self.form.custom_java_path = path.to_string_lossy().into_owned();
                self.update(Message::JavaSourcePicked(JavaSourceKind::Custom))
            }

            Message::ClientDownloadProgress(p) => {
                if let ClientState::Downloading { progress, .. } = &mut self.client {
                    progress.apply(p);
                }
                Task::none()
            }
            Message::ClientInstalled(result) => match result {
                Ok(state) => {
                    tracing::info!("installed client {}", state.label());
                    self.client = ClientState::Current(state);
                    self.continue_flows()
                }
                Err(reason) => {
                    let (installed, remote) = match &self.client {
                        ClientState::Downloading { installed, remote, .. } => (installed.clone(), Some(remote.clone())),
                        other => (other.installed().cloned(), None),
                    };
                    self.client = ClientState::Failed { installed, remote, reason: reason.clone() };
                    self.pending_launch = false;
                    self.onboarding = false;
                    self.notice =
                        Some(Notice::new(NoticeLevel::Error, format!("Couldn't download the client: {reason}")).with_action("Retry", Message::RetryClient));
                    Task::none()
                }
            },

            Message::StageLauncherUpdate => {
                let update = match &self.self_update {
                    SelfUpdateState::Available(u) => u.clone(),
                    _ => return Task::none(),
                };
                self.self_update = SelfUpdateState::Staging { update: update.clone(), progress: ProgressView::new("Downloading") };
                tasks::stage_launcher(self.http.clone(), update)
            }
            Message::LauncherUpdateProgress(p) => {
                if let SelfUpdateState::Staging { progress, .. } = &mut self.self_update {
                    progress.apply(p);
                }
                Task::none()
            }
            Message::LauncherUpdateStaged(result) => {
                if let SelfUpdateState::Staging { update, .. } = &self.self_update {
                    let update = update.clone();
                    self.self_update = match result {
                        Ok(path) => SelfUpdateState::Staged { update, path },
                        Err(e) => {
                            self.notify(NoticeLevel::Error, format!("Couldn't download the launcher update: {e}"));
                            SelfUpdateState::Available(update)
                        }
                    };
                }
                Task::none()
            }
            Message::ApplyLauncherUpdate => {
                if let SelfUpdateState::Staged { path, .. } = &self.self_update {
                    match launcher_core::self_update::apply_and_restart(path) {
                        Ok(never) => match never {},
                        Err(e) => {
                            tracing::error!("self-update failed: {e}");
                            self.notify(NoticeLevel::Error, format!("Couldn't install the update: {e}"));
                        }
                    }
                }
                Task::none()
            }
            Message::DismissLauncherUpdate => {
                if self.panel == Panel::LauncherUpdate {
                    self.panel = Panel::None;
                }
                Task::none()
            }

            Message::ChannelPicked(channel) => {
                if self.settings.channel == channel {
                    return Task::none();
                }
                self.settings.channel = channel;
                self.client = ClientState::Checking;
                self.self_update = SelfUpdateState::Checking;
                Task::batch([
                    self.save(),
                    tasks::check_client(self.http.clone(), self.paths.clone(), channel),
                    tasks::check_launcher(self.http.clone(), self.build.clone(), channel),
                ])
            }
            Message::HeapPicked(choice) => {
                self.settings.max_heap_gb = choice.0;
                self.save()
            }
            Message::KeepOpenToggled(on) => {
                self.settings.after_launch = if on { AfterLaunch::StayOpen } else { AfterLaunch::Close };
                self.save()
            }
            Message::ExtraArgsChanged(args) => {
                self.settings.extra_jvm_args = args;
                self.save()
            }
            Message::ShowPanel(panel) => {
                self.panel = panel;
                Task::none()
            }
            Message::ClosePanel => {
                if self.panel == Panel::FirstRun {
                    self.settings.first_run_completed = true;
                    self.panel = Panel::None;
                    return self.save();
                }
                if self.panel == Panel::BugReport {
                    return self.update(Message::DismissReport);
                }
                self.panel = Panel::None;
                Task::none()
            }
            Message::GetStarted => {
                self.onboarding = true;
                self.pending_launch = false;
                self.continue_flows()
            }

            Message::ClientLaunched(result) => match result {
                Ok(LaunchOutcome::Running { pid, exit }) => {
                    tracing::info!("client running, pid {pid}");
                    if self.settings.after_launch == AfterLaunch::Close {
                        return iced::exit();
                    }
                    self.launch = LaunchState::Running { pid };
                    self.notify(NoticeLevel::Success, format!("Client running (pid {pid}). You can start another one."));
                    tasks::await_client_exit(exit)
                }
                Ok(LaunchOutcome::EarlyExit { code, log_tail, log_path }) => {
                    tracing::warn!("client exited early with {code:?}");
                    self.launch = LaunchState::Idle;
                    self.open_report(ReportSource::LaunchFailure { code, log_tail, log_path });
                    Task::none()
                }
                Err(e) => {
                    self.launch = LaunchState::Idle;
                    self.open_report(ReportSource::LaunchFailure { code: None, log_tail: e, log_path: self.paths.launcher_log() });
                    Task::none()
                }
            },
            Message::ClientExited(code) => {
                tracing::info!("client exited with {code:?}");
                self.launch = LaunchState::Idle;
                // A release may have landed while they were playing.
                let recheck = self.recheck_client(false);
                if self.panel != Panel::None {
                    return recheck;
                }
                // The client writes its own report when it can; a native JVM crash
                // leaves only an hs_err file and a non-zero exit code.
                if let Some(file) = report::newest_unseen(&self.paths, self.settings.last_crash_seen.as_deref()) {
                    self.settings.last_crash_seen = Some(file.modified_rfc3339());
                    self.open_report(ReportSource::CrashFile(file));
                    return Task::batch([self.save(), recheck]);
                } else if code.is_some_and(|c| c != 0) {
                    let log_path = report::newest_client_log(&self.paths).unwrap_or_else(|| self.paths.launcher_log());
                    let log_tail = launch::log_tail(&log_path, 40);
                    self.open_report(ReportSource::LaunchFailure { code, log_tail, log_path });
                } else {
                    self.notice = None;
                }
                recheck
            }
            Message::RetryLaunch => {
                self.launch = LaunchState::Idle;
                self.report = None;
                self.panel = Panel::None;
                self.update(Message::PlayPressed)
            }

            Message::CopyReport => {
                let text = self.current_report().text;
                if let Some(view) = &mut self.report {
                    view.copied = true;
                }
                iced::clipboard::write(text)
            }
            Message::OpenIssue => {
                open_url(&self.current_report().issue_url());
                Task::none()
            }
            Message::ShowReportFile => {
                if let Some(file) = self.current_report().file {
                    let folder = file.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| self.paths.logs_dir());
                    if let Err(e) = open_folder(&folder) {
                        self.notify(NoticeLevel::Error, format!("Couldn't open the folder: {e}"));
                    }
                }
                Task::none()
            }
            Message::ReportProblem => {
                self.open_report(ReportSource::Logs);
                Task::none()
            }
            Message::DismissReport => {
                self.report = None;
                self.panel = Panel::None;
                Task::none()
            }

            Message::SettingsSaved(result) => {
                if let Err(e) = result {
                    tracing::warn!("could not save settings: {e}");
                }
                Task::none()
            }
            Message::DismissNotice => {
                self.notice = None;
                Task::none()
            }
            Message::Keyboard(event) => {
                if let keyboard::Event::KeyPressed { key, .. } = event {
                    match key {
                        keyboard::Key::Named(keyboard::key::Named::Escape) if self.panel != Panel::FirstRun => {
                            return self.update(Message::ClosePanel);
                        }
                        keyboard::Key::Named(keyboard::key::Named::Enter) if self.panel == Panel::None => {
                            return self.update(Message::PlayPressed);
                        }
                        keyboard::Key::Named(keyboard::key::Named::Enter) if self.panel == Panel::FirstRun && !self.onboarding => {
                            return self.update(Message::GetStarted);
                        }
                        _ => {}
                    }
                }
                Task::none()
            }
            Message::Tick => {
                self.tick = self.tick.wrapping_add(1);
                Task::none()
            }
        }
    }

    /// Asks GitHub again what the channel offers, so Play never starts a jar that a
    /// release has since replaced. `stale_only` keeps a burst of Play presses from
    /// re-checking every time; an install or a check already in flight is left alone.
    fn recheck_client(&mut self, stale_only: bool) -> Task<Message> {
        if !matches!(self.client, ClientState::Current(_) | ClientState::Offline { .. } | ClientState::Failed { .. }) {
            return Task::none();
        }
        if stale_only && self.client_checked_at.is_some_and(|at| at.elapsed() < CLIENT_CHECK_FRESH) {
            return Task::none();
        }
        self.client = ClientState::Checking;
        tasks::check_client(self.http.clone(), self.paths.clone(), self.settings.channel)
    }

    /// Moves the "Play" and "Get started" flows forward whenever a check lands.
    fn continue_flows(&mut self) -> Task<Message> {
        if !self.pending_launch && !self.onboarding {
            return Task::none();
        }
        // Java first.
        match &self.java {
            JavaState::Checking | JavaState::Installing(_) => return Task::none(),
            JavaState::Ready(_) => {}
            JavaState::Problem(problem) => {
                if self.onboarding
                    && matches!(problem, JavaProblem::ManagedNotInstalled | JavaProblem::NotFound | JavaProblem::TooOld { .. } | JavaProblem::Headless { .. })
                {
                    return self.update(Message::InstallJava);
                }
                self.pending_launch = false;
                self.onboarding = false;
                self.panel = Panel::JavaHelp;
                return Task::none();
            }
            JavaState::InstallFailed(_) => {
                self.pending_launch = false;
                self.onboarding = false;
                self.panel = Panel::JavaHelp;
                return Task::none();
            }
        }
        // Then the client.
        match &self.client {
            ClientState::Checking | ClientState::Downloading { .. } => Task::none(),
            ClientState::UpdateAvailable { installed, remote } => {
                let (task, handle) = tasks::install_client(self.http.clone(), self.paths.clone(), remote.clone());
                self.client = ClientState::Downloading {
                    remote: remote.clone(),
                    installed: installed.clone(),
                    progress: ProgressView::new(format!("Downloading client {}", remote.version)),
                    handle,
                };
                task
            }
            ClientState::Current(_) | ClientState::Offline { .. } | ClientState::Failed { .. } if self.client.playable() => {
                if self.onboarding {
                    self.onboarding = false;
                    self.settings.first_run_completed = true;
                    self.panel = Panel::None;
                    self.notify(NoticeLevel::Success, "You're all set. Press Play to start the game.");
                    return self.save();
                }
                self.pending_launch = false;
                self.spawn()
            }
            _ => {
                self.pending_launch = false;
                self.onboarding = false;
                self.notice = Some(
                    Notice::new(NoticeLevel::Error, "The client isn't downloaded yet and github.com couldn't be reached.")
                        .with_action("Retry", Message::RetryClient),
                );
                Task::none()
            }
        }
    }

    fn spawn(&mut self) -> Task<Message> {
        let JavaState::Ready(java) = &self.java else { return Task::none() };
        let plan = launch::build_plan(&self.settings, java, &self.paths);
        if plan.capped {
            self.notify(NoticeLevel::Warning, format!("Max heap reduced to {} GB to fit this computer's memory.", plan.heap_mb / 1024));
        }
        self.launch = LaunchState::Starting;
        tasks::spawn_client(plan)
    }

    /// Validates and applies the custom server fields when Custom is selected.
    fn commit_custom_server(&mut self) -> Result<(), String> {
        if let Some(error) = &self.form.world_error {
            return Err(error.clone());
        }
        if !self.settings.server.is_custom() {
            return Ok(());
        }
        let host = validate_host(&self.form.custom_host)?;
        let port = validate_port(&self.form.custom_port)?;
        self.settings.custom_host = host.clone();
        self.settings.custom_port = port;
        self.settings.server = Server::Custom { host, port };
        Ok(())
    }

    pub fn view(&self) -> Element<'_, Message> {
        crate::view::view(self)
    }
}

/// Stable-channel update prompts are shown at most once a day.
fn prompt_due(last: Option<&str>) -> bool {
    let Some(last) = last else { return true };
    let Ok(last) = time::OffsetDateTime::parse(last, &time::format_description::well_known::Rfc3339) else { return true };
    time::OffsetDateTime::now_utc() - last > time::Duration::hours(24)
}
