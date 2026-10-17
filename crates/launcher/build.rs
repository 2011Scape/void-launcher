//! Stamps build identity into the binary and embeds Windows resources.
//!
//! CI sets `VOID_VERSION`, `VOID_CHANNEL`, `VOID_COMMIT` and `VOID_BUILT_AT`;
//! local builds fall back to the Cargo version, the git commit and "local".

use std::process::Command;

fn main() {
    let cargo_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let commit = std::env::var("VOID_COMMIT")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short=8", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into());
    let version = std::env::var("VOID_VERSION").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("{cargo_version}-local"));
    let channel = std::env::var("VOID_CHANNEL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "development".into());
    let built_at = std::env::var("VOID_BUILT_AT").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        rfc3339_utc(secs)
    });
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    println!("cargo:rustc-env=VOID_VERSION={version}");
    println!("cargo:rustc-env=VOID_CHANNEL={channel}");
    println!("cargo:rustc-env=VOID_COMMIT={commit}");
    println!("cargo:rustc-env=VOID_BUILT_AT={built_at}");
    println!("cargo:rustc-env=VOID_TARGET={target_os}-{target_arch}");
    for var in ["VOID_VERSION", "VOID_CHANNEL", "VOID_COMMIT", "VOID_BUILT_AT"] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../.git/HEAD");

    // build.rs runs on the host, so gate on the *target* OS (cross-builds from Linux included).
    if target_os == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/windows/void.ico");
        res.set_manifest_file("../../packaging/windows/void-launcher.manifest");
        // A complete version block is one of the cheapest ways to look like a
        // legitimate desktop program to antivirus heuristics; unsigned binaries
        // with sparse metadata are what they are trained to distrust.
        res.set("ProductName", "Void Launcher");
        res.set("FileDescription", "Void Launcher");
        res.set("CompanyName", "2011Scape");
        res.set("LegalCopyright", "Copyright 2011Scape contributors. AGPL-3.0.");
        res.set("OriginalFilename", "void-launcher.exe");
        res.set("InternalName", "void-launcher");
        res.set("ProductVersion", &cargo_version);
        res.set("FileVersion", &cargo_version);
        res.set("Comments", "https://greghib.github.io/void/");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows resources: {e}");
        }
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` for a Unix timestamp, without pulling a date crate into the build script.
fn rfc3339_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant), valid for the proleptic Gregorian calendar.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}
