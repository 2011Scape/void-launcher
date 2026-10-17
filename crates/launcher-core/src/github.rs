//! GitHub release assets, without the REST API.
//!
//! Both projects publish every build as release assets, and GitHub serves
//! `releases/latest/download/<file>` and `releases/download/<tag>/<file>`
//! to anyone, with no rate limit. Versions come from the `SHA256SUMS` file
//! published next to the artifacts, so the launcher never needs to list a
//! release.

use crate::error::CoreError;
use crate::http::Http;
use crate::version::Channel;

pub const CHECKSUMS_NAME: &str = "SHA256SUMS";

pub fn repo_url(repo: &str) -> String {
    format!("https://github.com/{repo}")
}

pub fn releases_url(repo: &str) -> String {
    format!("https://github.com/{repo}/releases")
}

/// The release page for a version (`2.8.0`, or any dev version).
pub fn release_page(repo: &str, channel: Channel, version: &str) -> String {
    match channel {
        Channel::Stable => format!("https://github.com/{repo}/releases/tag/v{version}"),
        Channel::Development => format!("https://github.com/{repo}/releases/tag/{}", Channel::DEV_TAG),
    }
}

/// Download URL of a release asset on the channel's current release.
pub fn asset_url(repo: &str, channel: Channel, file: &str) -> String {
    match channel {
        Channel::Stable => format!("https://github.com/{repo}/releases/latest/download/{file}"),
        Channel::Development => format!("https://github.com/{repo}/releases/download/{}/{file}", Channel::DEV_TAG),
    }
}

/// A "new issue" page with the title and body filled in.
pub fn issue_url(repo: &str, title: &str, body: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new()).append_pair("title", title).append_pair("body", body).finish();
    format!("https://github.com/{repo}/issues/new?{query}")
}

/// One line of a `SHA256SUMS` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksum {
    pub sha256: String,
    pub name: String,
}

/// Parses `sha256sum` output: `<hex>  <name>` (or `<hex> *<name>` for binary mode).
pub fn parse_sha256sums(text: &str) -> Vec<Checksum> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (sha, name) = line.split_once(char::is_whitespace)?;
            let name = name.trim_start().trim_start_matches('*').trim();
            let sha = sha.trim().to_ascii_lowercase();
            (sha.len() == 64 && sha.chars().all(|c| c.is_ascii_hexdigit()) && !name.is_empty()).then(|| Checksum { sha256: sha, name: name.to_string() })
        })
        .collect()
}

/// The checksums published with the channel's current release.
pub async fn fetch_checksums(http: &Http, repo: &str, channel: Channel) -> Result<Vec<Checksum>, CoreError> {
    let url = asset_url(repo, channel, CHECKSUMS_NAME);
    let text = http.get_text(&url).await?;
    let sums = parse_sha256sums(&text);
    if sums.is_empty() {
        return Err(CoreError::BadResponse { url, reason: "SHA256SUMS is empty or malformed".into() });
    }
    Ok(sums)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_follow_the_release_scheme() {
        assert_eq!(asset_url("o/r", Channel::Stable, "SHA256SUMS"), "https://github.com/o/r/releases/latest/download/SHA256SUMS");
        assert_eq!(asset_url("o/r", Channel::Development, "a.jar"), "https://github.com/o/r/releases/download/dev/a.jar");
        assert_eq!(release_page("o/r", Channel::Stable, "2.8.0"), "https://github.com/o/r/releases/tag/v2.8.0");
        assert_eq!(release_page("o/r", Channel::Development, "dev-1-a"), "https://github.com/o/r/releases/tag/dev");
        let issue = issue_url("o/r", "Void client crash", "```\nline 1\n```");
        assert!(issue.starts_with("https://github.com/o/r/issues/new?title=Void+client+crash&body="));
        assert!(issue.contains("%0A"));
    }

    #[test]
    fn parses_sha256sums_variants() {
        let text = "0827bc5179b58950f426dd98dc2d2fbec51439cda806d64fdb3c5e8ab78d8de1  runetek5-634-2.8.0.jar\n\
                    7F804886D077F5F17245C29685C445A84E09B0FAF3EEE859750644ED3BFDDD6D *runetek5-634-browser-2.8.0.zip\n\
                    not a checksum line\n\
                    abc  short.jar\n";
        let sums = parse_sha256sums(text);
        assert_eq!(sums.len(), 2);
        assert_eq!(sums[0].name, "runetek5-634-2.8.0.jar");
        assert_eq!(sums[1].name, "runetek5-634-browser-2.8.0.zip");
        assert_eq!(sums[1].sha256, "7f804886d077f5f17245c29685c445a84e09b0faf3eee859750644ed3bfddd6d");
        assert!(parse_sha256sums("").is_empty());
    }
}
