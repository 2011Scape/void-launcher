//! Boot orchestration: housekeeping, then the three concurrent checks.

use crate::client::{self, ClientStatus};
use crate::http::Http;
use crate::java::{self, JavaInstall, JavaProblem};
use crate::logging::prune_client_logs;
use crate::paths::VoidPaths;
use crate::self_update::{self, UpdateStatus};
use crate::settings::Settings;
use crate::version::LauncherBuild;

pub struct StartupReport {
    pub java: Result<JavaInstall, JavaProblem>,
    pub client: ClientStatus,
    pub launcher: UpdateStatus,
}

/// Runs the housekeeping that must happen before any check.
pub fn prepare(paths: &VoidPaths) {
    self_update::finish_previous_update();
    if let Err(e) = paths.ensure_dirs() {
        tracing::warn!("could not create launcher directories: {e}");
    }
    prune_client_logs(paths, 20);
    // Partial downloads from an interrupted run.
    if let Ok(entries) = std::fs::read_dir(paths.tmp_dir()) {
        for entry in entries.flatten() {
            let _ = if entry.path().is_dir() { std::fs::remove_dir_all(entry.path()) } else { std::fs::remove_file(entry.path()) };
        }
    }
}

pub async fn run(http: Http, paths: VoidPaths, settings: Settings, build: LauncherBuild) -> StartupReport {
    let (java, client, launcher) =
        tokio::join!(java::find_java(&settings, &paths), client::check(&http, &paths, settings.channel), self_update::check(&http, &build, settings.channel),);
    StartupReport { java, client, launcher }
}
