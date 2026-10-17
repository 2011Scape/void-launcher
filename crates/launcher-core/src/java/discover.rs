//! Where Java installs live on each platform. Returns candidate homes with a hint.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::paths::VoidPaths;

/// Candidate Java homes in priority order, de-duplicated by canonical path.
pub fn candidates(paths: &VoidPaths) -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut push = |home: PathBuf, hint: &str| {
        let key = std::fs::canonicalize(&home).unwrap_or_else(|_| home.clone());
        if is_blind_mac_stub(&key) {
            return;
        }
        if seen.insert(key) {
            out.push((home, hint.to_string()));
        }
    };

    if let Some(java_home) = std::env::var_os("JAVA_HOME").filter(|v| !v.is_empty()) {
        push(PathBuf::from(java_home), "JAVA_HOME");
    }
    for exe in which::which_all("java").into_iter().flatten() {
        if let Some(home) = exe.parent().and_then(Path::parent) {
            push(home.to_path_buf(), "PATH");
        }
    }
    let home = &paths.home;
    for (dir, hint) in [
        (home.join(".sdkman/candidates/java"), "sdkman"),
        (home.join(".jdks"), "~/.jdks"),
        (home.join(".asdf/installs/java"), "asdf"),
        (home.join(".local/share/mise/installs/java"), "mise"),
    ] {
        for child in children(&dir) {
            push(child, hint);
        }
    }

    #[cfg(target_os = "linux")]
    {
        for (dir, hint) in [("/usr/lib/jvm", "/usr/lib/jvm"), ("/usr/lib64/jvm", "/usr/lib64/jvm"), ("/usr/java", "/usr/java"), ("/opt", "/opt")] {
            for child in children(Path::new(dir)) {
                let name = child.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                if dir != "/opt" || name.contains("jdk") || name.contains("java") || name.contains("jre") {
                    push(child, hint);
                }
            }
        }
        push(PathBuf::from("/snap/openjdk/current/jdk"), "snap");
    }

    #[cfg(target_os = "macos")]
    {
        for home_dir in mac_java_home_list() {
            push(home_dir, "java_home");
        }
        for dir in ["/Library/Java/JavaVirtualMachines", &format!("{}/Library/Java/JavaVirtualMachines", home.display())] {
            for child in children(Path::new(dir)) {
                push(child.join("Contents/Home"), "JavaVirtualMachines");
            }
        }
        for dir in ["/opt/homebrew/opt", "/usr/local/opt"] {
            for child in children(Path::new(dir)) {
                if child.file_name().map(|n| n.to_string_lossy().starts_with("openjdk")).unwrap_or(false) {
                    push(child.join("libexec/openjdk.jdk/Contents/Home"), "Homebrew");
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        for (home_dir, hint) in windows_registry_homes() {
            push(home_dir, &hint);
        }
        let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let vendors = ["Java", "Eclipse Adoptium", "Eclipse Foundation", "Microsoft", "Zulu", "BellSoft", "Amazon Corretto", "Semeru"];
        for vendor in vendors {
            for child in children(&program_files.join(vendor)) {
                push(child, "Program Files");
            }
            if let Some(local) = &local {
                for child in children(&local.join("Programs").join(vendor)) {
                    push(child, "LocalAppData");
                }
            }
        }
    }

    out.into_iter().filter(|(home, _)| super::probe::exe_for_home(&super::probe::normalize_home(home)).is_file()).collect()
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs
}

/// `/usr/bin/java` on macOS is an Apple stub that opens a "No Java runtime" dialog
/// when executed with no JDK installed; never probe it blindly.
fn is_blind_mac_stub(canonical: &Path) -> bool {
    cfg!(target_os = "macos") && (canonical == Path::new("/usr") || canonical == Path::new("/usr/bin"))
}

#[cfg(target_os = "macos")]
fn mac_java_home_list() -> Vec<PathBuf> {
    let Ok(output) = std::process::Command::new("/usr/libexec/java_home").arg("-V").output() else { return Vec::new() };
    if !output.status.success() {
        return Vec::new();
    }
    // Listing goes to stderr; each line ends with the absolute home path.
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| line.rfind(" /").map(|i| PathBuf::from(line[i + 1..].trim())))
        .filter(|p| p.join("bin").is_dir())
        .collect()
}

#[cfg(target_os = "windows")]
fn windows_registry_homes() -> Vec<(PathBuf, String)> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut out = Vec::new();
    let open = |path: &str| hklm.open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY).ok();

    for base in [r"SOFTWARE\JavaSoft\JDK", r"SOFTWARE\JavaSoft\JRE", r"SOFTWARE\JavaSoft\Java Runtime Environment", r"SOFTWARE\JavaSoft\Java Development Kit"] {
        if let Some(key) = open(base) {
            for version in key.enum_keys().flatten() {
                if let Some(sub) = open(&format!("{base}\\{version}"))
                    && let Ok(home) = sub.get_value::<String, _>("JavaHome")
                {
                    out.push((PathBuf::from(home), format!("registry {base}")));
                }
            }
        }
    }
    for vendor in [r"SOFTWARE\Eclipse Adoptium", r"SOFTWARE\Eclipse Foundation", r"SOFTWARE\AdoptOpenJDK", r"SOFTWARE\Semeru"] {
        for kind in ["JRE", "JDK"] {
            let base = format!("{vendor}\\{kind}");
            if let Some(key) = open(&base) {
                for version in key.enum_keys().flatten() {
                    for vm in ["hotspot", "openj9"] {
                        if let Some(sub) = open(&format!("{base}\\{version}\\{vm}\\MSI"))
                            && let Ok(path) = sub.get_value::<String, _>("Path")
                        {
                            out.push((PathBuf::from(path), format!("registry {vendor}")));
                        }
                    }
                }
            }
        }
    }
    for (base, value) in [(r"SOFTWARE\Azul Systems\Zulu", "InstallationPath"), (r"SOFTWARE\BellSoft\Liberica", "InstallationPath")] {
        if let Some(key) = open(base) {
            for name in key.enum_keys().flatten() {
                if let Some(sub) = open(&format!("{base}\\{name}"))
                    && let Ok(path) = sub.get_value::<String, _>(value)
                {
                    out.push((PathBuf::from(path), format!("registry {base}")));
                }
            }
        }
    }
    out
}
