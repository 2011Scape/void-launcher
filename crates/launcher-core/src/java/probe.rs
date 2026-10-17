//! Validating a Java home: version, vendor, architecture, desktop support.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{Arch, JavaInstall, JavaOrigin, JavaProblem, JavaWarning, VmKind};

/// Parsed `release` file from a Java home.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReleaseFile {
    pub java_version: Option<String>,
    pub implementor: Option<String>,
    pub os_arch: Option<String>,
    pub image_type: Option<String>,
    pub modules: Vec<String>,
}

pub fn parse_release_file(text: &str) -> ReleaseFile {
    let mut r = ReleaseFile::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim().trim_matches('"').to_string();
        match key.trim() {
            "JAVA_VERSION" => r.java_version = Some(value),
            "IMPLEMENTOR" => r.implementor = Some(value),
            "OS_ARCH" => r.os_arch = Some(value),
            "IMAGE_TYPE" => r.image_type = Some(value),
            "MODULES" => r.modules = value.split_whitespace().map(str::to_string).collect(),
            _ => {}
        }
    }
    r
}

/// Major version from `1.8.0_292`, `17.0.13`, `21`, `26.0.2.1`, `11.0.23+9`, `21-ea`.
pub fn major_of(version: &str) -> Option<u32> {
    let version = version.trim().trim_matches('"');
    let core: String = version.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let mut parts = core.split('.').filter(|p| !p.is_empty());
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 { parts.next()?.parse().ok() } else { Some(first) }
}

/// Properties from `java -XshowSettings:properties -version` (on stderr).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VersionOutput {
    pub version: String,
    pub vendor: String,
    pub vm_name: String,
    pub os_arch: String,
    pub data_model: Option<u32>,
    pub java_home: Option<String>,
}

pub fn parse_properties_output(text: &str) -> Option<VersionOutput> {
    let mut out = VersionOutput::default();
    let mut found = false;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim().to_string();
        match key.trim() {
            "java.version" => {
                out.version = value;
                found = true;
            }
            "java.vendor" => out.vendor = value,
            "java.vm.name" => out.vm_name = value,
            "os.arch" => out.os_arch = value,
            "sun.arch.data.model" => out.data_model = value.parse().ok(),
            "java.home" => out.java_home = Some(value),
            _ => {}
        }
    }
    if found {
        return Some(out);
    }
    // Fallback: plain `java -version` first line: `openjdk version "21.0.5" 2024-10-15 LTS`.
    let first = text.lines().find(|l| l.contains("version"))?;
    let quoted = first.split('"').nth(1)?;
    out.version = quoted.to_string();
    out.vm_name = text.lines().find(|l| l.contains("VM")).unwrap_or_default().trim().to_string();
    Some(out)
}

pub fn vm_kind(vm_name: &str) -> VmKind {
    let lower = vm_name.to_ascii_lowercase();
    if lower.contains("openj9") {
        VmKind::OpenJ9
    } else if lower.contains("graal") {
        VmKind::Graal
    } else if lower.contains("hotspot") || lower.contains("server vm") || lower.contains("client vm") || lower.contains("zing") {
        VmKind::HotSpot
    } else {
        VmKind::Other
    }
}

/// `bin/java` (or `bin\java.exe`) for a Java home.
pub fn exe_for_home(home: &Path) -> PathBuf {
    if cfg!(windows) { home.join("bin").join("java.exe") } else { home.join("bin").join("java") }
}

/// Accepts a home directory, a `bin/java` executable, a macOS `.jdk` bundle or
/// `Contents/Home`, and returns the home directory.
pub fn normalize_home(path: &Path) -> PathBuf {
    let file_name = path.file_name().map(|f| f.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if file_name == "java" || file_name == "java.exe" || file_name == "javaw.exe" {
        if let Some(home) = path.parent().and_then(Path::parent) {
            return home.to_path_buf();
        }
    }
    if path.join("Contents").join("Home").join("bin").is_dir() {
        return path.join("Contents").join("Home");
    }
    if file_name == "bin" && path.parent().is_some() {
        return path.parent().unwrap().to_path_buf();
    }
    path.to_path_buf()
}

/// Whether the runtime ships the desktop (AWT) natives the client needs.
pub fn awt_present(home: &Path) -> bool {
    if cfg!(target_os = "windows") {
        home.join("bin").join("awt.dll").exists()
    } else if cfg!(target_os = "macos") {
        home.join("lib").join("libawt_lwawt.dylib").exists()
    } else {
        home.join("lib").join("libawt_xawt.so").exists()
    }
}

/// Runs the executable and classifies it.
pub async fn probe(home: &Path, origin: JavaOrigin, timeout: Duration) -> Result<JavaInstall, JavaProblem> {
    let home = normalize_home(home);
    let exe = exe_for_home(&home);
    if !exe.is_file() {
        return Err(JavaProblem::Broken { path: exe, stderr: "no java executable at this location".into() });
    }
    let release = std::fs::read_to_string(home.join("release")).map(|t| parse_release_file(&t)).unwrap_or_default();

    let mut command = tokio::process::Command::new(&exe);
    command.args(["-XshowSettings:properties", "-version"]).stdin(std::process::Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return Err(JavaProblem::Broken { path: exe, stderr: e.to_string() }),
        Err(_) => return Err(JavaProblem::Broken { path: exe, stderr: "java -version did not finish in time".into() }),
    };
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() {
        let tail: String = stderr.lines().chain(stdout.lines()).take(3).collect::<Vec<_>>().join(" ");
        return Err(JavaProblem::Broken { path: exe, stderr: tail });
    }
    let Some(props) = parse_properties_output(&format!("{stderr}\n{stdout}")) else {
        return Err(JavaProblem::Broken { path: exe, stderr: "could not parse java -version output".into() });
    };
    let full_version = if props.version.is_empty() { release.java_version.clone().unwrap_or_default() } else { props.version.clone() };
    let major = major_of(&full_version).or_else(|| release.java_version.as_deref().and_then(major_of)).unwrap_or(0);
    let arch = if props.data_model == Some(32) {
        Arch::X86
    } else if !props.os_arch.is_empty() {
        Arch::parse(&props.os_arch)
    } else {
        release.os_arch.as_deref().map(Arch::parse).unwrap_or_else(Arch::host)
    };
    let vm = vm_kind(&props.vm_name);
    let vendor = if props.vendor.is_empty() { release.implementor.clone().unwrap_or_else(|| "unknown".into()) } else { props.vendor.clone() };
    let headless = !awt_present(&home) || (!release.modules.is_empty() && !release.modules.iter().any(|m| m == "java.desktop"));

    let mut warnings = Vec::new();
    if major >= 24 {
        warnings.push(JavaWarning::UnsafeDeprecated);
    }
    if major > super::MAX_TESTED_JAVA {
        warnings.push(JavaWarning::TooNew);
    }
    if cfg!(target_os = "macos") && Arch::host() == Arch::Aarch64 && arch == Arch::X86_64 {
        warnings.push(JavaWarning::Rosetta);
    }
    if vm != VmKind::HotSpot {
        warnings.push(JavaWarning::NonHotSpot);
    }
    Ok(JavaInstall { home, exe, major, full_version, vendor, arch, vm, headless, origin, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn major_versions() {
        assert_eq!(major_of("1.8.0_292"), Some(8));
        assert_eq!(major_of("17.0.13"), Some(17));
        assert_eq!(major_of("21"), Some(21));
        assert_eq!(major_of("26.0.2.1"), Some(26));
        assert_eq!(major_of("11.0.23+9"), Some(11));
        assert_eq!(major_of("21-ea"), Some(21));
        assert_eq!(major_of("garbage"), None);
    }

    #[test]
    fn parses_properties_and_plain_output() {
        let text = "Property settings:\n    java.home = /usr/lib/jvm/temurin-21\n    java.vendor = Eclipse Adoptium\n    java.version = 21.0.5\n    java.vm.name = OpenJDK 64-Bit Server VM\n    os.arch = amd64\n    sun.arch.data.model = 64\nopenjdk version \"21.0.5\" 2024-10-15 LTS\n";
        let p = parse_properties_output(text).unwrap();
        assert_eq!(p.version, "21.0.5");
        assert_eq!(p.vendor, "Eclipse Adoptium");
        assert_eq!(vm_kind(&p.vm_name), VmKind::HotSpot);
        assert_eq!(Arch::parse(&p.os_arch), Arch::X86_64);
        assert_eq!(p.data_model, Some(64));

        let plain = "openjdk version \"17.0.9\" 2023-10-17\nOpenJDK Runtime Environment (build 17.0.9+9)\nEclipse OpenJ9 VM (build openj9-0.41.0)\n";
        let p = parse_properties_output(plain).unwrap();
        assert_eq!(p.version, "17.0.9");
        assert_eq!(vm_kind(&p.vm_name), VmKind::OpenJ9);
        assert_eq!(vm_kind("GraalVM CE 21.0.2+13.1"), VmKind::Graal);
        assert_eq!(vm_kind("Java HotSpot(TM) 64-Bit Server VM"), VmKind::HotSpot);
    }

    #[test]
    fn parses_release_file() {
        let r = parse_release_file(
            "IMPLEMENTOR=\"Eclipse Adoptium\"\nJAVA_VERSION=\"21.0.5\"\nOS_ARCH=\"x86_64\"\nIMAGE_TYPE=\"JRE\"\nMODULES=\"java.base java.desktop\"\n",
        );
        assert_eq!(r.java_version.as_deref(), Some("21.0.5"));
        assert_eq!(r.implementor.as_deref(), Some("Eclipse Adoptium"));
        assert!(r.modules.contains(&"java.desktop".to_string()));
    }

    #[test]
    fn normalises_homes() {
        assert_eq!(normalize_home(Path::new("/jdk/bin/java")), PathBuf::from("/jdk"));
        assert_eq!(normalize_home(Path::new("/jdk")), PathBuf::from("/jdk"));
        assert_eq!(normalize_home(Path::new("/jdk/bin")), PathBuf::from("/jdk"));
    }
}
