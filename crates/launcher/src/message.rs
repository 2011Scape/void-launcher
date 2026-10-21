use std::path::PathBuf;

use launcher_core::client::{ClientState as InstalledClient, ClientStatus};
use launcher_core::java::runtime::RuntimeInfo;
use launcher_core::java::{JavaInstall, JavaProblem};
use launcher_core::launch::LaunchOutcome;
use launcher_core::progress::Progress;
use launcher_core::self_update::UpdateStatus;
use launcher_core::settings::{Extra, FpsCap, Msaa, RenderDistance, RenderScale, Renderer, Scale};
use launcher_core::version::Channel;

use crate::links::Link;
use crate::state::{HeapChoice, JavaSourceKind, Panel, ServerKind};

#[derive(Debug, Clone)]
pub enum Message {
    // boot / checks
    JavaResolved(Result<JavaInstall, JavaProblem>),
    ClientChecked(ClientStatus),
    LauncherChecked(UpdateStatus),

    // play panel
    ServerPicked(ServerKind),
    CustomHostChanged(String),
    CustomPortChanged(String),
    BrowseKeysFile,
    KeysFilePicked(Option<PathBuf>),
    ClearKeysFile,
    RendererPicked(Renderer),

    // game settings
    ExtraToggled(Extra, bool),
    WorldChanged(String),
    RenderDistancePicked(RenderDistance),
    FpsPicked(FpsCap),
    ScalePicked(Scale),
    MsaaPicked(Msaa),
    SmaaToggled(bool),
    RenderScalePicked(RenderScale),
    DebugToggled(bool),
    ExtraClientArgsChanged(String),

    // actions
    PlayPressed,
    CancelDownload,
    RetryClient,
    OpenLogsFolder,
    OpenPluginsFolder,
    OpenLink(Link),
    OpenUrl(String),
    CheckForUpdates,
    CopyText(String),

    // java
    InstallJava,
    JavaInstallProgress(Progress),
    JavaInstalled(Result<RuntimeInfo, String>),
    RecheckJava,
    JavaSourcePicked(JavaSourceKind),
    CustomJavaPathChanged(String),
    BrowseJavaPath,
    JavaPathPicked(Option<PathBuf>),

    // client download
    ClientDownloadProgress(Progress),
    ClientInstalled(Result<InstalledClient, String>),

    // launcher self-update
    StageLauncherUpdate,
    LauncherUpdateProgress(Progress),
    LauncherUpdateStaged(Result<PathBuf, String>),
    ApplyLauncherUpdate,
    DismissLauncherUpdate,

    // settings
    ChannelPicked(Channel),
    HeapPicked(HeapChoice),
    KeepOpenToggled(bool),
    ExtraArgsChanged(String),
    ShowPanel(Panel),
    ClosePanel,
    GetStarted,

    // launch
    ClientLaunched(Result<LaunchOutcome, String>),
    ClientExited(Option<i32>),
    RetryLaunch,

    // bug reports
    CopyReport,
    OpenIssue,
    ShowReportFile,
    ReportProblem,
    DismissReport,

    // misc
    SettingsSaved(Result<(), String>),
    DismissNotice,
    Keyboard(iced::keyboard::Event),
    Tick,
}
