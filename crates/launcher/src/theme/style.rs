//! Style functions reproducing the site's buttons, panels and inputs.
#![allow(dead_code)]

use std::f32::consts::PI;

use iced::gradient::Linear;
use iced::widget::overlay::menu;
use iced::widget::{button, checkbox, container, pick_list, progress_bar, radio, scrollable, text_input};
use iced::{Background, Border, Color, Gradient, Radians, Shadow, Theme, Vector};

use super::*;

/// Top-to-bottom gradient.
fn vertical(top: Color, bottom: Color) -> Background {
    Background::Gradient(Gradient::Linear(Linear::new(Radians(PI)).add_stop(0.0, top).add_stop(1.0, bottom)))
}

fn rounded(radius: f32, color: Color, width: f32) -> Border {
    Border { color, width, radius: radius.into() }
}

fn drop(color: Color, y: f32, blur: f32) -> Shadow {
    Shadow { color, offset: Vector::new(0.0, y), blur_radius: blur }
}

/// `--shadow-xs`, `--shadow-md`, `--shadow-lg`.
pub fn shadow_xs() -> Shadow {
    drop(alpha(SHADOW, 0.5), 1.0, 2.0)
}
pub fn shadow_md() -> Shadow {
    drop(alpha(SHADOW, 0.45), 6.0, 18.0)
}
pub fn shadow_lg() -> Shadow {
    drop(alpha(SHADOW, 0.55), 18.0, 44.0)
}

/// Mixes `a` towards `b` by `t`.
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color { r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t, a: 1.0 }
}

/// A lit-from-above face: slightly lighter at the top edge (the site's inset bevels).
fn bevel(face: Color) -> Background {
    vertical(mix(face, Color::WHITE, 0.07), mix(face, Color::BLACK, 0.06))
}

/// `.void-btn-primary`: gold face, gold-700 border, bevel.
pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(bevel(ACTION_PRIMARY)),
        text_color: TEXT_ON_GOLD,
        border: rounded(RADIUS_MD, GOLD_600, 1.0),
        shadow: shadow_xs(),
        snap: true,
    };
    match status {
        button::Status::Hovered => button::Style { background: Some(bevel(ACTION_PRIMARY_HOVER)), shadow: drop(alpha(GOLD_400, 0.22), 0.0, 18.0), ..base },
        button::Status::Pressed => button::Style { background: Some(Background::Color(ACTION_PRIMARY_PRESS)), shadow: Shadow::default(), ..base },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(ACTION_DISABLED)),
            text_color: TEXT_DISABLED,
            border: rounded(RADIUS_MD, UMBER_600, 1.0),
            shadow: Shadow::default(),
            ..base
        },
        _ => base,
    }
}

/// `.void-btn-secondary`: umber face, strong border, bevel.
pub fn secondary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(bevel(ACTION_SECONDARY)),
        text_color: TEXT_STRONG,
        border: rounded(RADIUS_MD, BORDER_STRONG, 1.0),
        shadow: shadow_xs(),
        snap: true,
    };
    match status {
        button::Status::Hovered => button::Style { background: Some(bevel(ACTION_SECONDARY_HOVER)), ..base },
        button::Status::Pressed => button::Style { background: Some(Background::Color(ACTION_SECONDARY_PRESS)), shadow: Shadow::default(), ..base },
        button::Status::Disabled => {
            button::Style { background: Some(Background::Color(ACTION_DISABLED)), text_color: TEXT_DISABLED, shadow: Shadow::default(), ..base }
        }
        _ => base,
    }
}

/// `.void-btn-ghost`: no chrome until hovered, then the hover surface and gold text.
pub fn ghost_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base =
        button::Style { background: None, text_color: TEXT_BODY, border: rounded(RADIUS_MD, Color::TRANSPARENT, 1.0), shadow: Shadow::default(), snap: true };
    match status {
        button::Status::Hovered => button::Style { background: Some(Background::Color(SURFACE_HOVER)), text_color: TEXT_ACCENT, ..base },
        button::Status::Pressed => button::Style { background: Some(Background::Color(SURFACE_ACTIVE)), text_color: TEXT_ACCENT, ..base },
        button::Status::Disabled => button::Style { text_color: TEXT_DISABLED, ..base },
        _ => base,
    }
}

/// Ghost with a visible outline, for a secondary action next to a primary one.
pub fn outline_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style { background: None, text_color: TEXT_BODY, border: rounded(RADIUS_MD, BORDER_STRONG, 1.0), shadow: Shadow::default(), snap: true };
    match status {
        button::Status::Hovered => {
            button::Style { background: Some(Background::Color(SURFACE_HOVER)), text_color: TEXT_ACCENT, border: rounded(RADIUS_MD, GOLD_600, 1.0), ..base }
        }
        button::Status::Pressed => button::Style { background: Some(Background::Color(SURFACE_ACTIVE)), ..base },
        button::Status::Disabled => button::Style { text_color: TEXT_DISABLED, border: rounded(RADIUS_MD, UMBER_600, 1.0), ..base },
        _ => base,
    }
}

/// `.void-btn-danger`.
pub fn danger_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base =
        button::Style { background: Some(bevel(EMBER_600)), text_color: PARCH_50, border: rounded(RADIUS_MD, EMBER_600, 1.0), shadow: shadow_xs(), snap: true };
    match status {
        button::Status::Hovered => button::Style { background: Some(bevel(EMBER_500)), ..base },
        button::Status::Pressed => button::Style { background: Some(Background::Color(EMBER_600)), shadow: Shadow::default(), ..base },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(ACTION_DISABLED)),
            text_color: TEXT_DISABLED,
            border: rounded(RADIUS_MD, UMBER_600, 1.0),
            ..base
        },
        _ => base,
    }
}

/// Square text-glyph button (the × in headers): no chrome until hovered.
pub fn icon_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base =
        button::Style { background: None, text_color: TEXT_MUTED, border: rounded(RADIUS_SM, Color::TRANSPARENT, 0.0), shadow: Shadow::default(), snap: true };
    match status {
        button::Status::Hovered => button::Style { background: Some(Background::Color(SURFACE_HOVER)), text_color: TEXT_ACCENT, ..base },
        button::Status::Pressed => button::Style { background: Some(Background::Color(SURFACE_ACTIVE)), text_color: TEXT_ACCENT, ..base },
        _ => base,
    }
}

/// Text-only button used for links.
pub fn link_button(_theme: &Theme, status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => TEXT_LINK_HOVER,
        button::Status::Disabled => TEXT_DISABLED,
        _ => TEXT_LINK,
    };
    button::Style { background: None, text_color: color, border: Border::default(), shadow: Shadow::default(), snap: true }
}

/// A panel on the page: `--surface-panel`, panel border, shallow corners, `--shadow-md`.
pub fn panel(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_BODY),
        background: Some(bevel(SURFACE_PANEL)),
        border: rounded(RADIUS_MD, BORDER_PANEL, 1.0),
        shadow: shadow_md(),
        snap: true,
    }
}

/// A dialog floating over the scrim: raised one step, stronger border, `--shadow-lg`.
pub fn dialog(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_BODY),
        background: Some(bevel(SURFACE_PANEL)),
        border: rounded(RADIUS_LG, BORDER_STRONG, 1.0),
        shadow: shadow_lg(),
        snap: true,
    }
}

/// Recessed box inside a panel (previews, code).
pub fn inset(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_BODY),
        background: Some(Background::Color(SURFACE_INSET)),
        border: rounded(RADIUS_SM, BORDER_SUBTLE, 1.0),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// The 56 px header strip: `--surface-header`; the gold hairline below it is a separate rule.
pub fn header(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_BODY),
        background: Some(Background::Color(SURFACE_HEADER)),
        border: Border::default(),
        shadow: shadow_xs(),
        snap: true,
    }
}

pub fn status_bar(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_MUTED),
        background: Some(Background::Color(alpha(SURFACE_HEADER, 0.94))),
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn drawer(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_BODY),
        background: Some(Background::Color(SURFACE_PANEL)),
        border: Border { color: BORDER_PANEL, width: 1.0, radius: 0.0.into() },
        shadow: Shadow { color: alpha(SHADOW, 0.6), offset: Vector::new(-8.0, 0.0), blur_radius: 32.0 },
        snap: true,
    }
}

/// Header strip pinned above the settings drawer's scrolling body.
pub fn drawer_header(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_BODY),
        background: Some(Background::Color(SURFACE_HEADER)),
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// `--surface-overlay` behind dialogs.
pub fn scrim(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: None,
        background: Some(Background::Color(SURFACE_OVERLAY)),
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// `--scrim-bottom`: protects type over the hero art, clear at the top, nearly solid at the bottom.
pub fn hero_scrim(_theme: &Theme) -> container::Style {
    let gradient = Gradient::Linear(
        Linear::new(Radians(PI)).add_stop(0.0, alpha(UMBER_950, 0.0)).add_stop(0.58, alpha(UMBER_950, 0.78)).add_stop(1.0, alpha(UMBER_950, 0.94)),
    );
    container::Style { text_color: None, background: Some(Background::Gradient(gradient)), border: Border::default(), shadow: Shadow::default(), snap: true }
}

pub fn notice(level: NoticeLevel) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let (fg, bg) = level.colors();
        container::Style {
            text_color: Some(fg),
            background: Some(Background::Color(bg)),
            border: rounded(RADIUS_MD, alpha(fg, 0.45), 1.0),
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

pub fn code_box(theme: &Theme) -> container::Style {
    inset(theme)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl NoticeLevel {
    /// (foreground, background) from the site's feedback tokens.
    pub fn colors(self) -> (Color, Color) {
        match self {
            NoticeLevel::Info => (STEEL_500, STEEL_900),
            NoticeLevel::Success => (MOSS_500, MOSS_900),
            NoticeLevel::Warning => (AMBER_500, AMBER_900),
            NoticeLevel::Error => (EMBER_500, EMBER_900),
        }
    }
}

pub fn checkbox_style(_theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let (is_checked, hovered, disabled) = match status {
        checkbox::Status::Active { is_checked } => (is_checked, false, false),
        checkbox::Status::Hovered { is_checked } => (is_checked, true, false),
        checkbox::Status::Disabled { is_checked } => (is_checked, false, true),
    };
    checkbox::Style {
        background: if is_checked { bevel(ACTION_PRIMARY) } else { Background::Color(SURFACE_INSET) },
        icon_color: TEXT_ON_GOLD,
        border: rounded(
            RADIUS_XS,
            if hovered {
                GOLD_400
            } else if is_checked {
                GOLD_600
            } else {
                BORDER_STRONG
            },
            1.0,
        ),
        text_color: Some(if disabled { TEXT_DISABLED } else { TEXT_BODY }),
    }
}

pub fn radio_style(_theme: &Theme, status: radio::Status) -> radio::Style {
    let hovered = matches!(status, radio::Status::Hovered { .. });
    radio::Style {
        background: Background::Color(SURFACE_INSET),
        dot_color: GOLD_400,
        border_width: 1.0,
        border_color: if hovered { GOLD_400 } else { BORDER_STRONG },
        text_color: Some(TEXT_BODY),
    }
}

pub fn text_input_style(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { .. } => GOLD_400,
        text_input::Status::Hovered => BORDER_STRONG,
        text_input::Status::Disabled => UMBER_700,
        _ => BORDER_SUBTLE,
    };
    text_input::Style {
        background: Background::Color(SURFACE_INSET),
        border: rounded(RADIUS_SM, border_color, 1.0),
        icon: TEXT_ACCENT,
        placeholder: TEXT_FAINT,
        value: TEXT_BODY,
        selection: alpha(GOLD_400, 0.35),
    }
}

pub fn pick_list_style(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let border_color = match status {
        pick_list::Status::Opened { .. } => GOLD_400,
        pick_list::Status::Hovered => BORDER_STRONG,
        _ => BORDER_SUBTLE,
    };
    pick_list::Style {
        text_color: TEXT_BODY,
        placeholder_color: TEXT_FAINT,
        handle_color: TEXT_ACCENT,
        background: Background::Color(SURFACE_INSET),
        border: rounded(RADIUS_SM, border_color, 1.0),
    }
}

pub fn menu_style(_theme: &Theme) -> menu::Style {
    menu::Style {
        background: Background::Color(SURFACE_PANEL_RAISED),
        border: rounded(RADIUS_SM, BORDER_STRONG, 1.0),
        text_color: TEXT_BODY,
        selected_text_color: TEXT_ON_GOLD,
        selected_background: Background::Color(ACTION_PRIMARY),
        shadow: shadow_md(),
    }
}

pub fn progress_style(_theme: &Theme) -> progress_bar::Style {
    progress_bar::Style { background: Background::Color(SURFACE_INSET), bar: bevel(ACTION_PRIMARY), border: rounded(RADIUS_SM, BORDER_SUBTLE, 1.0) }
}

pub fn scrollable_style(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = scrollable::default(theme, status);
    let active = matches!(
        status,
        scrollable::Status::Hovered { is_vertical_scrollbar_hovered: true, .. } | scrollable::Status::Dragged { is_vertical_scrollbar_dragged: true, .. }
    );
    let rail = scrollable::Rail {
        background: None,
        border: rounded(RADIUS_SM, Color::TRANSPARENT, 0.0),
        scroller: scrollable::Scroller {
            background: Background::Color(if active { GOLD_400 } else { alpha(GOLD_400, 0.35) }),
            border: rounded(RADIUS_SM, Color::TRANSPARENT, 0.0),
        },
    };
    style.vertical_rail = rail;
    style.horizontal_rail = rail;
    style
}
