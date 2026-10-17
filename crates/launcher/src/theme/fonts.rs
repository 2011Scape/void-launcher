//! Cinzel for display, Inter Tight for UI text, JetBrains Mono for code: the site's faces.
#![allow(dead_code)]
use iced::Font;
use iced::font::{Family, Stretch, Style, Weight};

pub const CINZEL_REGULAR_BYTES: &[u8] = include_bytes!("../../assets/fonts/Cinzel-Regular.ttf");
pub const CINZEL_BOLD_BYTES: &[u8] = include_bytes!("../../assets/fonts/Cinzel-Bold.ttf");
pub const UI_REGULAR_BYTES: &[u8] = include_bytes!("../../assets/fonts/InterTight-Regular.ttf");
pub const UI_MEDIUM_BYTES: &[u8] = include_bytes!("../../assets/fonts/InterTight-Medium.ttf");
pub const UI_SEMIBOLD_BYTES: &[u8] = include_bytes!("../../assets/fonts/InterTight-SemiBold.ttf");
pub const MONO_BYTES: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");

const fn font(family: &'static str, weight: Weight) -> Font {
    Font { family: Family::Name(family), weight, stretch: Stretch::Normal, style: Style::Normal }
}

pub const DISPLAY: Font = font("Cinzel", Weight::Normal);
pub const DISPLAY_BOLD: Font = font("Cinzel", Weight::Bold);
pub const UI: Font = font("Inter Tight", Weight::Normal);
pub const UI_MEDIUM: Font = font("Inter Tight", Weight::Medium);
pub const UI_SEMIBOLD: Font = font("Inter Tight", Weight::Semibold);
pub const MONO: Font = font("JetBrains Mono", Weight::Normal);
