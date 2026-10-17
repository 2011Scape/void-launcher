//! Embedded artwork. The handles are created once: `image::Handle::from_bytes`
//! mints a fresh id every call, and a new id per frame makes the renderer
//! re-decode and re-upload the art on every redraw (visible as a flicker).

use std::sync::LazyLock;

use iced::widget::{image, svg};

/// The site's hero art.
pub const BG_PNG: &[u8] = include_bytes!("../../assets/images/bg.png");
pub const ICON_PNG: &[u8] = include_bytes!("../../assets/images/icon.png");
/// The Void mark, gold on transparent.
pub const MARK_SVG: &[u8] = include_bytes!("../../assets/images/mark.svg");

static BACKGROUND: LazyLock<image::Handle> = LazyLock::new(|| image::Handle::from_bytes(BG_PNG));
static MARK: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(MARK_SVG));

pub fn background() -> image::Handle {
    BACKGROUND.clone()
}

pub fn mark() -> svg::Handle {
    MARK.clone()
}
