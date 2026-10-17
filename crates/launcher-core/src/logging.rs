//! `tracing` to `~/.void/logs/launcher.log` (rotated) and optionally stderr.

use std::path::Path;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::paths::VoidPaths;

const KEEP_GENERATIONS: usize = 5;

/// Keep this guard alive for the life of the process so buffered log lines are flushed.
pub struct LogGuard(#[allow(dead_code)] Option<tracing_appender::non_blocking::WorkerGuard>);

pub fn init(paths: &VoidPaths) -> LogGuard {
    let _ = std::fs::create_dir_all(paths.logs_dir());
    let log_path = paths.launcher_log();
    rotate(&log_path, KEEP_GENERATIONS);

    let filter = EnvFilter::try_from_env("VOID_LOG")
        .unwrap_or_else(|_| EnvFilter::new("warn,launcher_core=info,void_launcher=info,iced_winit=warn,wgpu_core=error,wgpu_hal=error,naga=error"));
    let verbose_stderr = cfg!(debug_assertions) || std::env::var_os("VOID_LOG").is_some();

    let file_layer_guard = match std::fs::File::create(&log_path) {
        Ok(file) => {
            let (writer, guard) = tracing_appender::non_blocking(file);
            Some((tracing_subscriber::fmt::layer().with_ansi(false).with_target(false).with_writer(writer), guard))
        }
        Err(e) => {
            eprintln!("could not open {}: {e}", log_path.display());
            None
        }
    };
    let (file_layer, guard) = match file_layer_guard {
        Some((layer, guard)) => (Some(layer), Some(guard)),
        None => (None, None),
    };
    let stderr_layer = verbose_stderr.then(|| tracing_subscriber::fmt::layer().with_writer(std::io::stderr).with_target(false));

    let _ = tracing_subscriber::registry().with(filter).with(file_layer).with(stderr_layer).try_init();
    LogGuard(guard)
}

/// `launcher.log` -> `launcher.log.1` -> ... -> `launcher.log.N`, dropping the oldest.
fn rotate(path: &Path, keep: usize) {
    let name = |n: usize| {
        let mut p = path.as_os_str().to_owned();
        p.push(format!(".{n}"));
        std::path::PathBuf::from(p)
    };
    let _ = std::fs::remove_file(name(keep));
    for n in (1..keep).rev() {
        let _ = std::fs::rename(name(n), name(n + 1));
    }
    if path.exists() {
        let _ = std::fs::rename(path, name(1));
    }
}

/// Deletes old `client-*.log` files, keeping the newest `keep`.
pub fn prune_client_logs(paths: &VoidPaths, keep: usize) {
    let Ok(entries) = std::fs::read_dir(paths.logs_dir()) else { return };
    let mut logs: Vec<_> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("client-"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    logs.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in logs.into_iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
}
