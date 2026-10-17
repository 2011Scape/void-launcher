//! The one place that knows both iced and launcher-core.

use std::path::PathBuf;

use iced::Task;
use iced::task::Handle;
use launcher_core::client::RemoteClient;
use launcher_core::http::Http;
use launcher_core::java::runtime;
use launcher_core::launch::{ExitWatch, LaunchPlan};
use launcher_core::paths::VoidPaths;
use launcher_core::self_update::LauncherUpdate;
use launcher_core::settings::Settings;
use launcher_core::version::{Channel, LauncherBuild};
use launcher_core::{client, java, launch, self_update};

use crate::message::Message;

pub fn check_java(settings: Settings, paths: VoidPaths) -> Task<Message> {
    Task::perform(async move { java::find_java(&settings, &paths).await }, Message::JavaResolved)
}

pub fn check_client(http: Http, paths: VoidPaths, channel: Channel) -> Task<Message> {
    Task::perform(async move { client::check(&http, &paths, channel).await }, Message::ClientChecked)
}

pub fn check_launcher(http: Http, build: LauncherBuild, channel: Channel) -> Task<Message> {
    Task::perform(async move { self_update::check(&http, &build, channel).await }, Message::LauncherChecked)
}

pub fn install_client(http: Http, paths: VoidPaths, remote: RemoteClient) -> (Task<Message>, Handle) {
    Task::sip(client::install(http, paths, remote), Message::ClientDownloadProgress, |result| {
        Message::ClientInstalled(result.map_err(|e| {
            tracing::error!("client install failed: {e}");
            e.to_string()
        }))
    })
    .abortable()
}

pub fn install_java(http: Http, paths: VoidPaths) -> Task<Message> {
    Task::sip(runtime::install(http, paths), Message::JavaInstallProgress, |result| {
        Message::JavaInstalled(result.map_err(|e| {
            tracing::error!("java install failed: {e}");
            e.to_string()
        }))
    })
}

pub fn stage_launcher(http: Http, update: LauncherUpdate) -> Task<Message> {
    Task::sip(self_update::stage(http, update), Message::LauncherUpdateProgress, |result| {
        Message::LauncherUpdateStaged(result.map_err(|e| {
            tracing::error!("launcher update failed: {e}");
            e.to_string()
        }))
    })
}

pub fn spawn_client(plan: LaunchPlan) -> Task<Message> {
    Task::perform(async move { launch::launch(&plan).await.map_err(|e| e.to_string()) }, Message::ClientLaunched)
}

pub fn await_client_exit(exit: ExitWatch) -> Task<Message> {
    Task::perform(exit.wait(), Message::ClientExited)
}

pub fn save_settings(settings: Settings, paths: VoidPaths) -> Task<Message> {
    Task::perform(async move { settings.save(&paths).map_err(|e| e.to_string()) }, Message::SettingsSaved)
}

pub fn pick_java_path() -> Task<Message> {
    Task::perform(
        async move {
            rfd::AsyncFileDialog::new().set_title("Choose your Java folder (the one containing bin/java)").pick_folder().await.map(|h| PathBuf::from(h.path()))
        },
        Message::JavaPathPicked,
    )
}

pub fn pick_keys_file() -> Task<Message> {
    Task::perform(
        async move {
            rfd::AsyncFileDialog::new()
                .set_title("Choose the server's game.properties")
                .add_filter("Server keys (game.properties)", &["properties"])
                .add_filter("All files", &["*"])
                .pick_file()
                .await
                .map(|h| PathBuf::from(h.path()))
        },
        Message::KeysFilePicked,
    )
}
