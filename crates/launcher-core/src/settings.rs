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

/// The client's gameplay switches. Some are on by default in the client and
/// have a `--no-` flag; the launcher only emits a flag when the choice differs
/// from the client's own default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Extra {
    TileMarkers,
    XpDrops,
    NpcLabels,
    GroundItems,
    ShiftDrop,
    Zoom,
}

impl Extra {
    pub const ALL: [Extra; 6] = [Extra::TileMarkers, Extra::XpDrops, Extra::NpcLabels, Extra::GroundItems, Extra::ShiftDrop, Extra::Zoom];

    /// What the client does when nothing is passed.
    pub fn default_on(self) -> bool {
        matches!(self, Extra::ShiftDrop | Extra::Zoom)
    }

    /// The flag that flips the client's default.
    pub fn flag(self) -> &'static str {
        match self {
            Extra::TileMarkers => "--tile-markers",
            Extra::XpDrops => "--xp-drops",
            Extra::NpcLabels => "--npc-labels",
            Extra::GroundItems => "--ground-items",
            Extra::ShiftDrop => "--no-shift-drop",
            Extra::Zoom => "--no-zoom",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Extra::TileMarkers => "Tile markers",
            Extra::XpDrops => "Experience drops",
            Extra::NpcLabels => "NPC labels",
            Extra::GroundItems => "Ground item labels",
            Extra::ShiftDrop => "Shift-drop",
            Extra::Zoom => "Wider zoom",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Extra::TileMarkers => "Right-click a tile and choose Mark tile to outline it.",
            Extra::XpDrops => "A \"+N Skill\" drop rises near the minimap when you gain experience.",
            Extra::NpcLabels => "NPC names and combat levels above their heads.",
            Extra::GroundItems => "The names and counts of items on each tile.",
            Extra::ShiftDrop => "Drop an item with Shift and a left click.",
            Extra::Zoom => "Let the mouse wheel zoom further out than the original range.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Extras {
    pub tile_markers: bool,
    pub xp_drops: bool,
    pub npc_labels: bool,
    pub ground_items: bool,
    pub shift_drop: bool,
    pub zoom: bool,
}

impl Default for Extras {
    fn default() -> Self {
        Self { tile_markers: false, xp_drops: false, npc_labels: false, ground_items: false, shift_drop: true, zoom: true }
    }
}

impl Extras {
    pub fn get(&self, extra: Extra) -> bool {
        match extra {
            Extra::TileMarkers => self.tile_markers,
            Extra::XpDrops => self.xp_drops,
            Extra::NpcLabels => self.npc_labels,
            Extra::GroundItems => self.ground_items,
            Extra::ShiftDrop => self.shift_drop,
            Extra::Zoom => self.zoom,
        }
    }

    pub fn set(&mut self, extra: Extra, on: bool) {
        match extra {
            Extra::TileMarkers => self.tile_markers = on,
            Extra::XpDrops => self.xp_drops = on,
            Extra::NpcLabels => self.npc_labels = on,
            Extra::GroundItems => self.ground_items = on,
            Extra::ShiftDrop => self.shift_drop = on,
            Extra::Zoom => self.zoom = on,
        }
    }

    /// The switches to pass, in a stable order: only where the choice differs from the client's default.
    pub fn flags(&self) -> Vec<&'static str> {
        Extra::ALL.iter().copied().filter(|e| self.get(*e) != e.default_on()).map(Extra::flag).collect()
    }
}

/// A pick-list choice that is a number on the client's command line.
macro_rules! numeric_choice {
    ($(#[$meta:meta])* $name:ident, $flag:literal, default = $default:ident, { $($variant:ident => $value:literal, $label:literal;)+ }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $( #[serde(rename = $value)] $variant, )+
        }

        impl Default for $name {
            fn default() -> Self {
                $name::$default
            }
        }

        impl $name {
            pub const ALL: [$name; [$(stringify!($variant)),+].len()] = [$($name::$variant),+];
            pub const FLAG: &'static str = $flag;

            /// The value as the client wants it on the command line.
            pub fn value(self) -> &'static str {
                match self { $( $name::$variant => $value, )+ }
            }

            pub fn label(self) -> &'static str {
                match self { $( $name::$variant => $label, )+ }
            }

            /// `--flag value`, or nothing when this is the client's own default.
            pub fn args(self) -> Option<[&'static str; 2]> {
                (self != $name::$default).then_some([$flag, self.value()])
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.label())
            }
        }
    };
}

numeric_choice! {
    /// `--render-distance`: how far the world is drawn, as a multiple of the original.
    RenderDistance, "--render-distance", default = X2, {
        X1 => "1", "Original";
        X1_5 => "1.5", "1.5×";
        X2 => "2", "2×  ·  client default";
        X3 => "3", "3×";
    }
}

numeric_choice! {
    /// `--fps`: the frame cap. The game keeps its speed; movement is interpolated between cycles.
    FpsCap, "--fps", default = Fps144, {
        Fps30 => "30", "30";
        Fps50 => "50", "50  ·  original";
        Fps60 => "60", "60";
        Fps100 => "100", "100";
        Fps144 => "144", "144  ·  client default";
        Fps240 => "240", "240";
    }
}

numeric_choice! {
    /// `--msaa`: OpenGL multisampling of the 3D view.
    Msaa, "--msaa", default = Off, {
        Off => "0", "Off";
        X2 => "2", "MSAA 2×";
        X4 => "4", "MSAA 4×";
        X8 => "8", "MSAA 8×";
    }
}

numeric_choice! {
    /// `--render-scale`: OpenGL supersampling of the 3D view.
    RenderScale, "--render-scale", default = X1, {
        X1 => "1", "Off";
        X1_5 => "1.5", "1.5×";
        X2 => "2", "2×";
    }
}

/// `--scale`: how large the game is drawn. The client picks from the screen height by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scale {
    #[default]
    Auto,
    #[serde(rename = "1")]
    X1,
    #[serde(rename = "1.5")]
    X1_5,
    #[serde(rename = "2")]
    X2,
}

impl Scale {
    pub const ALL: [Scale; 4] = [Scale::Auto, Scale::X1, Scale::X1_5, Scale::X2];

    pub fn label(self) -> &'static str {
        match self {
            Scale::Auto => "Fit the screen  ·  client default",
            Scale::X1 => "1×  ·  original size",
            Scale::X1_5 => "1.5×",
            Scale::X2 => "2×",
        }
    }

    pub fn args(self) -> Option<[&'static str; 2]> {
        match self {
            Scale::Auto => None,
            Scale::X1 => Some(["--scale", "1"]),
            Scale::X1_5 => Some(["--scale", "1.5"]),
            Scale::X2 => Some(["--scale", "2"]),
        }
    }
}

impl std::fmt::Display for Scale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// The client's graphics switches. Every default is the client's own, so an
/// untouched profile passes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Graphics {
    pub render_distance: RenderDistance,
    pub fps: FpsCap,
    pub scale: Scale,
    pub msaa: Msaa,
    /// `--smaa`: SMAA 1x on the 3D view's edges (OpenGL).
    pub smaa: bool,
    pub render_scale: RenderScale,
}

impl Graphics {
    /// The switches to pass, in a stable order.
    pub fn args(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        for pair in [self.render_distance.args(), self.fps.args(), self.scale.args(), self.msaa.args(), self.render_scale.args()].into_iter().flatten() {
            out.extend(pair);
        }
        if self.smaa {
            out.push("--smaa");
        }
        out
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
    pub graphics: Graphics,
    pub world: u16,
    /// `--debug`: ids and tile coordinates on right-click menus.
    pub debug: bool,
    /// Anything else for the client's command line, appended last (`--lang german`, `--dev-commands`, ...).
    pub extra_client_args: String,
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
            graphics: Graphics::default(),
            world: DEFAULT_WORLD,
            debug: false,
            extra_client_args: String::new(),
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
        assert!(s.extras.flags().is_empty(), "client defaults pass no gameplay flags");
        assert!(s.graphics.args().is_empty(), "client defaults pass no graphics flags");
        assert!(s.extras.shift_drop && s.extras.zoom);
        assert_eq!(s.world, 1);
    }

    #[test]
    fn round_trips_and_ignores_unknown_fields() {
        let json = r#"{"channel":"development","server":{"kind":"custom","host":"10.0.0.2","port":1234},"custom_keys":"/srv/game.properties",
            "renderer":"hd","extras":{"xp_drops":true,"zoom":false},"graphics":{"fps":"60","msaa":"4","smaa":true,"scale":"1.5"},
            "world":2,"debug":true,"java":{"source":"custom","path":"/opt/jdk"},"future_field":1}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.channel, Channel::Development);
        assert_eq!(s.server, Server::Custom { host: "10.0.0.2".into(), port: 1234 });
        assert_eq!(s.keys_file(), Some(&PathBuf::from("/srv/game.properties")));
        assert_eq!(s.renderer, Renderer::Hd);
        assert_eq!(s.extras.flags(), vec!["--xp-drops", "--no-zoom"]);
        assert_eq!(s.graphics.args(), vec!["--fps", "60", "--scale", "1.5", "--msaa", "4", "--smaa"]);
        assert!(s.debug);
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
