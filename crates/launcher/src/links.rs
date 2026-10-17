use launcher_core::java::copy::ADOPTIUM_URL;
use launcher_core::{CLIENT_SOURCE_URL, ISSUES_URL, LAUNCHER_REPO, LAUNCHER_SOURCE_URL, WEBSITE_URL, github};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Website,
    ClientSource,
    LauncherSource,
    Issues,
    Adoptium,
    LauncherReleases,
}

impl Link {
    pub fn url(self) -> String {
        match self {
            Link::Website => WEBSITE_URL.into(),
            Link::ClientSource => CLIENT_SOURCE_URL.into(),
            Link::LauncherSource => LAUNCHER_SOURCE_URL.into(),
            Link::Issues => ISSUES_URL.into(),
            Link::Adoptium => ADOPTIUM_URL.into(),
            Link::LauncherReleases => github::releases_url(LAUNCHER_REPO),
        }
    }
}

pub fn open_url(url: &str) {
    if let Err(e) = open::that_detached(url) {
        tracing::warn!("could not open {url}: {e}");
    }
}
