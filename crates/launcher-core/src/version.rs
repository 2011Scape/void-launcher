use std::fmt;

use serde::{Deserialize, Serialize};

/// Which release stream to follow for both the launcher and the client.
///
/// Both projects publish the same way: a `vX.Y.Z` tag is a stable release
/// (GitHub's "latest"), and every push to `main` rebuilds one pre-release
/// tagged `dev`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    Development,
}

impl Channel {
    pub const ALL: [Channel; 2] = [Channel::Stable, Channel::Development];

    pub fn label(self) -> &'static str {
        match self {
            Channel::Stable => "Stable",
            Channel::Development => "Development",
        }
    }

    /// The git tag the rolling development pre-release lives under.
    pub const DEV_TAG: &'static str = "dev";
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Identity of the running launcher, stamped in at build time by the GUI
/// crate's `build.rs` (CI sets the `VOID_*` variables; local builds fall
/// back to the Cargo version and the git commit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherBuild {
    /// `1.2.0` for releases, `1.2.0-dev.abc1234` for dev builds, `1.2.0-local` otherwise.
    pub version: String,
    pub channel: Channel,
    pub commit: String,
    /// RFC 3339 UTC timestamp of the build.
    pub built_at: String,
    /// Target key used in launcher manifests, e.g. `linux-x86_64`.
    pub target: String,
}

impl LauncherBuild {
    pub fn semver(&self) -> Option<semver::Version> {
        semver::Version::parse(&self.version).ok()
    }

    pub fn is_release(&self) -> bool {
        self.semver().is_some_and(|v| v.pre.is_empty())
    }

    pub fn short_commit(&self) -> &str {
        let end = self.commit.len().min(8);
        &self.commit[..end]
    }
}

/// Strips a leading `v` from a git tag.
pub fn version_from_tag(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

/// The commit in a development version such as `dev-20261017-fd94175`
/// (the client's) or `dev-fd94175`.
pub fn commit_from_dev_version(version: &str) -> Option<&str> {
    let rest = version.strip_prefix("dev-")?;
    rest.rsplit('-').next().filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Native target key for launcher manifests.
pub fn target_key() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags_and_dev_versions() {
        assert_eq!(version_from_tag("v2.1.0"), "2.1.0");
        assert_eq!(version_from_tag("2.1.0"), "2.1.0");
        assert_eq!(commit_from_dev_version("dev-20261017-fd94175"), Some("fd94175"));
        assert_eq!(commit_from_dev_version("dev-74022382"), Some("74022382"));
        assert_eq!(commit_from_dev_version("2.0.0"), None);
        assert_eq!(commit_from_dev_version("dev-"), None);
    }

    #[test]
    fn release_detection() {
        let b = |v: &str| LauncherBuild { version: v.into(), channel: Channel::Stable, commit: "abc".into(), built_at: String::new(), target: String::new() };
        assert!(b("1.0.0").is_release());
        assert!(!b("1.0.0-dev.abc1234").is_release());
        assert!(!b("1.0.0-local").is_release());
        assert!(semver::Version::parse("1.0.0-dev.abc").unwrap() < semver::Version::parse("1.0.0").unwrap());
    }
}
