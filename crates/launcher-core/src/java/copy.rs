//! User-facing explanations and per-OS instructions for Java problems.

use super::{JavaProblem, MANAGED_FEATURE, MAX_TESTED_JAVA, MIN_JAVA};

pub const ADOPTIUM_URL: &str = "https://adoptium.net/temurin/releases/?version=21&package=jre";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    pub title: String,
    pub body: String,
    /// Shell commands that install a suitable Java on this OS (label, command).
    pub commands: Vec<(String, String)>,
    /// Whether the managed install is the recommended fix.
    pub offer_managed: bool,
}

pub fn explain(problem: &JavaProblem) -> Explanation {
    let (title, body, offer_managed) = match problem {
        JavaProblem::NotFound | JavaProblem::ManagedNotInstalled => (
            "Java is required to play".to_string(),
            format!(
                "The client runs on Java {MIN_JAVA} to {MAX_TESTED_JAVA} and none was found on this computer. The quickest fix is to let the launcher install a private copy of Java {MANAGED_FEATURE}; nothing else on your computer changes."
            ),
            true,
        ),
        JavaProblem::TooOld { best } => (
            format!("Java {} is too old", best.full_version),
            format!("We found Java {} at {}, but the client needs Java {MIN_JAVA} or newer.", best.full_version, best.home.display()),
            true,
        ),
        JavaProblem::Headless { install } => (
            "Your Java has no desktop support".to_string(),
            format!(
                "Java {} at {} is a headless build without the AWT libraries the game window needs. Install a full JRE (not the -headless package).",
                install.full_version,
                install.home.display()
            ),
            true,
        ),
        JavaProblem::ArchMismatch { install, host } => (
            "Wrong Java architecture".to_string(),
            format!(
                "Java {} at {} is {} but this machine is {host}. Install a 64-bit Java for your processor.",
                install.full_version,
                install.home.display(),
                install.arch
            ),
            true,
        ),
        JavaProblem::Broken { path, stderr } => (
            "Java did not start".to_string(),
            format!("{} could not run java -version: {}", path.display(), if stderr.is_empty() { "no output" } else { stderr }),
            true,
        ),
        JavaProblem::ManagedCorrupt { reason } => {
            ("The bundled Java runtime is damaged".to_string(), format!("{reason}. Reinstalling it will download a fresh copy."), true)
        }
        JavaProblem::CustomPathInvalid { path, reason } => (
            "That Java path does not work".to_string(),
            format!("{}: {reason}. Choose the folder that contains bin/java, or switch back to the managed runtime.", path.display()),
            true,
        ),
    };
    Explanation { title, body, commands: install_commands(), offer_managed }
}

/// Manual install commands for the current OS.
pub fn install_commands() -> Vec<(String, String)> {
    if cfg!(target_os = "windows") {
        vec![("Windows (winget)".into(), "winget install EclipseAdoptium.Temurin.21.JRE".into())]
    } else if cfg!(target_os = "macos") {
        vec![("macOS (Homebrew)".into(), "brew install --cask temurin@21".into())]
    } else {
        let id = linux_distro_id();
        let all = [
            ("Debian / Ubuntu", "sudo apt install openjdk-21-jre", &["debian", "ubuntu", "linuxmint", "pop"][..]),
            ("Fedora", "sudo dnf install java-21-openjdk", &["fedora", "rhel", "centos", "rocky", "almalinux"][..]),
            ("Arch", "sudo pacman -S jre21-openjdk", &["arch", "manjaro", "endeavouros", "cachyos"][..]),
            ("openSUSE", "sudo zypper install java-21-openjdk", &["opensuse", "suse", "opensuse-tumbleweed", "opensuse-leap"][..]),
        ];
        let matching: Vec<(String, String)> =
            all.iter().filter(|(_, _, ids)| id.iter().any(|i| ids.contains(&i.as_str()))).map(|(l, c, _)| (l.to_string(), c.to_string())).collect();
        if matching.is_empty() { all.iter().map(|(l, c, _)| (l.to_string(), c.to_string())).collect() } else { matching }
    }
}

/// `ID` and `ID_LIKE` from `/etc/os-release`.
fn linux_distro_id() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string("/etc/os-release") else { return Vec::new() };
    let mut ids = Vec::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("ID=") {
            ids.push(v.trim_matches('"').to_string());
        } else if let Some(v) = line.strip_prefix("ID_LIKE=") {
            ids.extend(v.trim_matches('"').split_whitespace().map(str::to_string));
        }
    }
    ids
}
