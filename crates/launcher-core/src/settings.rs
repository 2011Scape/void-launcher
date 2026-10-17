//! `~/.void/launcher.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::fsutil::atomic_write_json;
use crate::paths::VoidPaths;
use crate::version::Channel;

/// The 2011Scape world the client connects to by default.
pub const OFFICIAL_HOST: &str = "play.2011.rs";
pub const LOCAL_HOST: &str = "localhost";
/// The port a Void server listens on.
pub const DEFAULT_PORT: u16 = 43594;
pub const DEFAULT_WORLD: u16 = 1;
pub const DEFAULT_HEAP_GB: u8 = 2;
pub const HEAP_CHOICES_GB: [u8; 6] = [1, 2, 3, 4, 6, 8];

/// Which server the client connects to.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Server {
    /// 2011Scape, `play.2011.rs`.
    #[default]
    Official,
    /// A Void server on this computer.
    Local,
    Custom {
        host: String,
        port: u16,
    },
}

impl Server {
    pub fn host(&self) -> &str {
        match self {
            Server::Official => OFFICIAL_HOST,
            Server::Local => LOCAL_HOST,
            Server::Custom { host, .. } => host,
        }
    }

    pub fn port(&self) -> u16 {
        match self {
            Server::Custom { port, .. } => *port,
            _ => DEFAULT_PORT,
        }
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, Server::Custom { .. })
    }
}

/// The client's graphics mode (`--toolkit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Renderer {
    /// Whatever the client last used (its Graphics Options choice).
    #[default]
    Auto,
    /// 634HD, the OpenGL 3.3 renderer.
    Hd,
    /// The original OpenGL mode.
    OpenGl,
    /// Safe Mode: the software renderer.
    Safe,
}

impl Renderer {
    pub const ALL: [Renderer; 4] = [Renderer::Auto, Renderer::Hd, Renderer::OpenGl, Renderer::Safe];

    pub fn label(self) -> &'static str {
        match self {
            Renderer::Auto => "Client default",
            Renderer::Hd => "634HD  ·  OpenGL 3.3",
            Renderer::OpenGl => "OpenGL",
            Renderer::Safe => "Safe mode  ·  software",
        }
    }

    /// The `--toolkit` value, or none for the client's own choice.
    pub fn toolkit_arg(self) -> Option<&'static str> {
        match self {
            Renderer::Auto => None,
            Renderer::Hd => Some("hd"),
            Renderer::OpenGl => Some("opengl"),
            Renderer::Safe => Some("java"),
        }
    }
}

impl std::fmt::Display for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// The client's optional extras, each a command-line switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Extra {
    TileMarkers,
    XpDrops,
    NpcLabels,
    GroundItems,
}

impl Extra {
    pub const ALL: [Extra; 4] = [Extra::TileMarkers, Extra::XpDrops, Extra::NpcLabels, Extra::GroundItems];

    pub fn flag(self) -> &'static str {
        match self {
            Extra::TileMarkers => "--tile-markers",
            Extra::XpDrops => "--xp-drops",
            Extra::NpcLabels => "--npc-labels",
            Extra::GroundItems => "--ground-items",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Extra::TileMarkers => "Tile markers",
            Extra::XpDrops => "Experience drops",
            Extra::NpcLabels => "NPC labels",
            Extra::GroundItems => "Ground item labels",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Extra::TileMarkers => "Right-click a tile and choose Mark tile to outline it.",
            Extra::XpDrops => "A \"+N Skill\" drop rises near the minimap when you gain experience.",
            Extra::NpcLabels => "NPC names and combat levels above their heads.",
            Extra::GroundItems => "The names and counts of items on each tile.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Extras {
    pub tile_markers: bool,
    pub xp_drops: bool,
    pub npc_labels: bool,
    pub ground_items: bool,
}

impl Extras {
    pub fn get(&self, extra: Extra) -> bool {
        match extra {
            Extra::TileMarkers => self.tile_markers,
            Extra::XpDrops => self.xp_drops,
            Extra::NpcLabels => self.npc_labels,
            Extra::GroundItems => self.ground_items,
        }
    }

    pub fn set(&mut self, extra: Extra, on: bool) {
        match extra {
            Extra::TileMarkers => self.tile_markers = on,
            Extra::XpDrops => self.xp_drops = on,
            Extra::NpcLabels => self.npc_labels = on,
            Extra::GroundItems => self.ground_items = on,
        }
    }

    /// The switches to pass, in a stable order.
    pub fn flags(&self) -> Vec<&'static str> {
        Extra::ALL.iter().copied().filter(|e| self.get(*e)).map(Extra::flag).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum JavaSource {
    /// The launcher-managed Temurin runtime in `~/.void/runtime`.
    #[default]
    Managed,
    /// Whatever suitable Java is installed on the system.
    System,
    /// A specific Java home or `bin/java` chosen by the user.
    Custom { path: PathBuf },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AfterLaunch {
    #[default]
    Close,
    StayOpen,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema: u32,
    pub channel: Channel,
    pub server: Server,
    /// Remembered custom server even while another server is selected.
    pub custom_host: String,
    pub custom_port: u16,
    /// A `game.properties` with the custom server's keys (`--keys`).
    pub custom_keys: Option<PathBuf>,
    pub renderer: Renderer,
    pub extras: Extras,
    pub world: u16,
    pub java: JavaSource,
    pub max_heap_gb: u8,
    pub extra_jvm_args: String,
    pub after_launch: AfterLaunch,
    pub check_updates: bool,
    pub first_run_completed: bool,
    /// RFC 3339 timestamp of the last launcher self-update check.
    pub last_update_check: Option<String>,
    /// RFC 3339 timestamp of the last time the user was prompted about a launcher update.
    pub last_update_prompt: Option<String>,
    /// RFC 3339 modification time of the newest crash file the user has been shown.
    pub last_crash_seen: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: 2,
            channel: Channel::Stable,
            server: Server::Official,
            custom_host: String::new(),
            custom_port: DEFAULT_PORT,
            custom_keys: None,
            renderer: Renderer::Auto,
            extras: Extras::default(),
            world: DEFAULT_WORLD,
            java: JavaSource::Managed,
            max_heap_gb: DEFAULT_HEAP_GB,
            extra_jvm_args: String::new(),
            after_launch: AfterLaunch::Close,
            check_updates: true,
            first_run_completed: false,
            last_update_check: None,
            last_update_prompt: None,
            last_crash_seen: None,
        }
    }
}

impl Settings {
    /// Loads settings; a missing or corrupt file yields defaults (the corrupt
    /// file is kept aside as `launcher.json.bad`).
    pub fn load(paths: &VoidPaths) -> Settings {
        let path = paths.launcher_json();
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
                Ok(settings) => settings,
                Err(e) => {
                    tracing::warn!("launcher.json is corrupt ({e}); using defaults");
                    let _ = std::fs::rename(&path, path.with_extension("json.bad"));
                    Settings::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(e) => {
                tracing::warn!("could not read launcher.json ({e}); using defaults");
                Settings::default()
            }
        }
    }

    pub fn save(&self, paths: &VoidPaths) -> Result<(), CoreError> {
        atomic_write_json(&paths.launcher_json(), self)
    }

    pub fn max_heap_mb(&self) -> u64 {
        u64::from(self.max_heap_gb.max(1)) * 1024
    }

    /// The keys file to pass, only for a custom server.
    pub fn keys_file(&self) -> Option<&PathBuf> {
        if self.server.is_custom() { self.custom_keys.as_ref() } else { None }
    }
}

/// Validates a user-entered server host: a bare hostname or IP, no scheme, no port.
pub fn validate_host(host: &str) -> Result<String, String> {
    let host = host.trim();
    if host.is_empty() {
        return Err("Enter a hostname or IP address.".into());
    }
    if host.contains("://") || host.contains('/') || host.contains(' ') {
        return Err("Enter just the hostname or IP, without http:// or a path.".into());
    }
    if host.contains(':') && host.parse::<std::net::Ipv6Addr>().is_err() {
        return Err("Put the port in the port field, not after a colon.".into());
    }
    let valid_chars = host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'));
    if !valid_chars {
        return Err("Hostnames may only contain letters, digits, dots and dashes.".into());
    }
    Ok(host.to_string())
}

pub fn validate_port(port: &str) -> Result<u16, String> {
    match port.trim().parse::<u16>() {
        Ok(p) if p > 0 => Ok(p),
        _ => Err("Port must be a number between 1 and 65535.".into()),
    }
}

pub fn validate_world(world: &str) -> Result<u16, String> {
    match world.trim().parse::<u16>() {
        Ok(w) if w > 0 => Ok(w),
        _ => Err("World must be a number from 1 upwards.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_from_empty_object() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Settings::default());
        assert_eq!(s.server.host(), OFFICIAL_HOST);
        assert_eq!(s.server.port(), DEFAULT_PORT);
        assert_eq!(s.renderer, Renderer::Auto);
        assert!(s.extras.flags().is_empty());
        assert_eq!(s.world, 1);
    }

    #[test]
    fn round_trips_and_ignores_unknown_fields() {
        let json = r#"{"channel":"development","server":{"kind":"custom","host":"10.0.0.2","port":1234},"custom_keys":"/srv/game.properties",
            "renderer":"hd","extras":{"xp_drops":true},"world":2,"java":{"source":"custom","path":"/opt/jdk"},"future_field":1}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.channel, Channel::Development);
        assert_eq!(s.server, Server::Custom { host: "10.0.0.2".into(), port: 1234 });
        assert_eq!(s.keys_file(), Some(&PathBuf::from("/srv/game.properties")));
        assert_eq!(s.renderer, Renderer::Hd);
        assert_eq!(s.extras.flags(), vec!["--xp-drops"]);
        assert_eq!(s.java, JavaSource::Custom { path: PathBuf::from("/opt/jdk") });
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn keys_only_apply_to_custom_servers() {
        let s = Settings { custom_keys: Some("/k".into()), ..Settings::default() };
        assert_eq!(s.keys_file(), None);
        assert_eq!(Server::Local.host(), "localhost");
        assert_eq!(Server::Local.port(), DEFAULT_PORT);
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let paths = VoidPaths::with_home(dir.path());
        paths.ensure_dirs().unwrap();
        std::fs::write(paths.launcher_json(), b"{not json").unwrap();
        let s = Settings::load(&paths);
        assert_eq!(s, Settings::default());
        assert!(paths.launcher_json().with_extension("json.bad").exists());
        s.save(&paths).unwrap();
        assert_eq!(Settings::load(&paths), s);
    }

    #[test]
    fn host_port_and_world_validation() {
        assert!(validate_host("play.2011.rs").is_ok());
        assert!(validate_host("192.168.1.10").is_ok());
        assert!(validate_host("http://x.org").is_err());
        assert!(validate_host("x.org:1234").is_err());
        assert!(validate_host("").is_err());
        assert_eq!(validate_port("43594"), Ok(43594));
        assert!(validate_port("0").is_err());
        assert!(validate_port("abc").is_err());
        assert_eq!(validate_world("2"), Ok(2));
        assert!(validate_world("0").is_err());
    }
}
