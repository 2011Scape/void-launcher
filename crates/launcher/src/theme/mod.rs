//! Void design tokens for iced, taken from the site's `style/tokens/*.css`.
#![allow(dead_code)]

pub mod fonts;
pub mod images;
pub mod style;

use std::sync::LazyLock;

use iced::theme::Palette;
use iced::{Color, Theme};

const fn rgb(hex: u32) -> Color {
    Color { r: ((hex >> 16) & 0xff) as f32 / 255.0, g: ((hex >> 8) & 0xff) as f32 / 255.0, b: (hex & 0xff) as f32 / 255.0, a: 1.0 }
}

pub const fn alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

// Umber: the primary brown ramp. Warm, low-chroma, never black.
pub const UMBER_950: Color = rgb(0x17120d);
pub const UMBER_900: Color = rgb(0x211a13);
pub const UMBER_850: Color = rgb(0x282017);
pub const UMBER_800: Color = rgb(0x2f261b);
pub const UMBER_700: Color = rgb(0x3b3024);
pub const UMBER_600: Color = rgb(0x4a3c2c);
pub const UMBER_500: Color = rgb(0x5d4b36);
pub const UMBER_400: Color = rgb(0x786243);
pub const UMBER_300: Color = rgb(0x9b8465);
pub const UMBER_200: Color = rgb(0xbda98c);
pub const UMBER_100: Color = rgb(0xdbcdb4);

// Gold: the single accent. Highlights, actions, active state, rules.
pub const GOLD_700: Color = rgb(0x8a6318);
pub const GOLD_600: Color = rgb(0xa97c22);
pub const GOLD_500: Color = rgb(0xc9962e);
pub const GOLD_400: Color = rgb(0xe0ae3c);
pub const GOLD_300: Color = rgb(0xf0c667);
pub const GOLD_200: Color = rgb(0xf7dc9c);
pub const GOLD_100: Color = rgb(0xfbeecb);
/// The gold of the Void mark itself.
pub const MARK_GOLD: Color = rgb(0xeabd60);

// Parchment: type and hairlines on dark ground.
pub const PARCH_50: Color = rgb(0xf6efe2);
pub const PARCH_100: Color = rgb(0xefe4d2);
pub const PARCH_200: Color = rgb(0xded0b9);
pub const PARCH_300: Color = rgb(0xbfae95);
pub const PARCH_400: Color = rgb(0x9c8b72);

// Semantic hues, all pulled toward the warm ground.
pub const MOSS_500: Color = rgb(0x7fae4f);
pub const MOSS_600: Color = rgb(0x5f8a38);
pub const MOSS_900: Color = rgb(0x26301a);
pub const EMBER_500: Color = rgb(0xc2493a);
pub const EMBER_600: Color = rgb(0x9d3628);
pub const EMBER_900: Color = rgb(0x331815);
pub const AMBER_500: Color = rgb(0xdd9a2b);
pub const AMBER_900: Color = rgb(0x33260f);
pub const STEEL_500: Color = rgb(0x7d9db0);
pub const STEEL_600: Color = rgb(0x5d7c8f);
pub const STEEL_900: Color = rgb(0x1c2830);

// Rune: the cold grey of the mark's stone.
pub const RUNE_300: Color = rgb(0xc8ced2);
pub const RUNE_500: Color = rgb(0x8f9aa1);
pub const RUNE_700: Color = rgb(0x5a646b);

// Surfaces: dark, warm, three steps only.
pub const SURFACE_APP: Color = UMBER_850;
pub const SURFACE_PANEL: Color = UMBER_800;
pub const SURFACE_PANEL_RAISED: Color = UMBER_700;
pub const SURFACE_INSET: Color = UMBER_900;
pub const SURFACE_HEADER: Color = UMBER_900;
pub const SURFACE_HOVER: Color = UMBER_700;
pub const SURFACE_ACTIVE: Color = UMBER_600;
pub const SURFACE_OVERLAY: Color = alpha(UMBER_950, 0.78);

// Ink.
pub const TEXT_BODY: Color = PARCH_100;
pub const TEXT_STRONG: Color = PARCH_50;
pub const TEXT_MUTED: Color = PARCH_300;
pub const TEXT_FAINT: Color = PARCH_400;
pub const TEXT_ACCENT: Color = GOLD_300;
pub const TEXT_ON_GOLD: Color = UMBER_950;
pub const TEXT_LINK: Color = GOLD_300;
pub const TEXT_LINK_HOVER: Color = GOLD_200;
pub const TEXT_DISABLED: Color = UMBER_300;

// Lines.
pub const BORDER_SUBTLE: Color = UMBER_600;
pub const BORDER_STRONG: Color = UMBER_500;
pub const BORDER_GOLD: Color = GOLD_600;
pub const BORDER_PANEL: Color = UMBER_600;

// Actions.
pub const ACTION_PRIMARY: Color = GOLD_400;
pub const ACTION_PRIMARY_HOVER: Color = GOLD_300;
pub const ACTION_PRIMARY_PRESS: Color = GOLD_500;
pub const ACTION_SECONDARY: Color = UMBER_600;
pub const ACTION_SECONDARY_HOVER: Color = UMBER_500;
pub const ACTION_SECONDARY_PRESS: Color = UMBER_700;
pub const ACTION_DISABLED: Color = UMBER_700;

// Bevel highlights (the site lights panels from above).
pub const BEVEL_LIGHT: Color = alpha(GOLD_200, 0.13);
pub const BEVEL_DARK: Color = alpha(Color::BLACK, 0.4);
pub const SHADOW: Color = rgb(0x0a0704);

// Corners stay shallow: chiselled stone, not soft plastic.
pub const RADIUS_XS: f32 = 2.0;
pub const RADIUS_SM: f32 = 3.0;
pub const RADIUS_MD: f32 = 5.0;
pub const RADIUS_LG: f32 = 8.0;

static THEME: LazyLock<Theme> = LazyLock::new(|| {
    Theme::custom(
        "Void".to_string(),
        Palette { background: SURFACE_APP, text: TEXT_BODY, primary: ACTION_PRIMARY, success: MOSS_500, warning: AMBER_500, danger: EMBER_500 },
    )
});

/// The launcher theme; built once, cloning only bumps an `Arc`.
pub fn theme() -> Theme {
    THEME.clone()
}
