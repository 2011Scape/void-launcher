//! Network-backed checks. Run with `VOID_NET_TESTS=1 cargo test -p launcher-core --test live_api -- --ignored`.

use launcher_core::http::Http;
use launcher_core::java::runtime::latest_asset;
use launcher_core::paths::VoidPaths;
use launcher_core::version::Channel;
use launcher_core::{LAUNCHER_REPO, client, github, java, self_update};

fn enabled() -> bool {
    std::env::var_os("VOID_NET_TESTS").is_some()
}

#[tokio::test]
#[ignore]
async fn resolves_both_client_channels() {
    if !enabled() {
        return;
    }
    let http = Http::new("void-launcher-tests").unwrap();
    for channel in Channel::ALL {
        let remote = client::resolve_remote(&http, channel).await.unwrap();
        println!("{channel}: {} {} {} bytes {}", remote.version, remote.sha256, remote.size, remote.url);
        assert_eq!(remote.sha256.len(), 64);
        assert!(remote.url.starts_with("https://github.com/2011Scape/634-client/releases/"));
        assert!(remote.url.ends_with(".jar"));
        assert!(remote.size > 10_000_000, "Content-Length should be known up front");
        if channel == Channel::Development {
            assert!(remote.version.starts_with("dev-"));
            assert!(remote.commit.is_some());
        } else {
            assert!(semver::Version::parse(&remote.version).is_ok());
        }
    }
}

#[tokio::test]
#[ignore]
async fn launcher_manifest_is_absent_or_valid() {
    if !enabled() {
        return;
    }
    let http = Http::new("void-launcher-tests").unwrap();
    for channel in Channel::ALL {
        match self_update::fetch_manifest(&http, channel).await {
            Ok(manifest) => {
                println!("{channel}: launcher {} ({}) {} files", manifest.version, manifest.commit, manifest.files.len());
                assert_eq!(manifest.schema, self_update::SUPPORTED_SCHEMA);
            }
            // Nothing published yet, or the repository is still private.
            Err(launcher_core::CoreError::HttpStatus { status: 404, .. }) => {
                println!("{channel}: no manifest at {}", github::asset_url(LAUNCHER_REPO, channel, "manifest.json"))
            }
            Err(e) => panic!("{channel}: {e}"),
        }
    }
}

#[tokio::test]
#[ignore]
async fn adoptium_offers_a_jre_for_this_host() {
    if !enabled() {
        return;
    }
    let http = Http::new("void-launcher-tests").unwrap();
    let asset = latest_asset(&http).await.unwrap();
    println!("{} {} {} {}", asset.release_name, asset.semver, asset.size, asset.url);
    assert!(asset.release_name.starts_with("jdk-21"));
}

#[tokio::test]
#[ignore]
async fn finds_system_java_when_present() {
    if !enabled() {
        return;
    }
    let paths = VoidPaths::resolve().unwrap();
    let installs = java::list_system(&paths).await;
    for i in &installs {
        println!("{} major={} arch={} vm={:?} headless={} score={} warnings={:?}", i.summary(), i.major, i.arch, i.vm, i.headless, i.score(), i.warnings);
    }
    if which::which("java").is_ok() {
        assert!(!installs.is_empty());
    }
}
