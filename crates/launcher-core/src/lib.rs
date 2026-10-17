//! Void launcher core: everything that is not UI.
//!
//! Settings, filesystem layout, GitHub release resolution, client jar
//! installation, Java discovery and the managed Temurin runtime, launcher
//! self-update and detached client launching. The GUI crate maps the async
//! functions here onto iced tasks; nothing in this crate depends on iced.

pub mod client;
pub mod error;
pub mod fsutil;
pub mod github;
pub mod hash;
pub mod http;
pub mod java;
pub mod launch;
pub mod logging;
pub mod paths;
pub mod progress;
pub mod report;
pub mod self_update;
pub mod settings;
pub mod startup;
pub mod version;

pub use error::CoreError;
pub use paths::VoidPaths;
pub use progress::Progress;
pub use settings::Settings;
pub use version::{Channel, LauncherBuild};

/// GitHub repositories the launcher reads releases from (`owner/name`).
pub const CLIENT_REPO: &str = "2011Scape/634-client";
pub const LAUNCHER_REPO: &str = "2011Scape/void-launcher";

pub const WEBSITE_URL: &str = "https://greghib.github.io/void/";
pub const CLIENT_SOURCE_URL: &str = "https://github.com/2011Scape/634-client";
pub const LAUNCHER_SOURCE_URL: &str = "https://github.com/2011Scape/void-launcher";
/// Where players are sent with a bug report on their clipboard.
pub const ISSUES_URL: &str = "https://github.com/2011Scape/634-client/issues";
