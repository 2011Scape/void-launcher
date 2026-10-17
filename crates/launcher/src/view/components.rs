//! Small reusable widgets styled with the Void tokens.
#![allow(dead_code)]

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{Space, button, column, container, row, scrollable, svg, text, tooltip};
use iced::{Alignment, Element, Fill, Length};

use crate::message::Message;
use crate::theme::style::NoticeLevel;
use crate::theme::{self, fonts, images, style};

/// `--type-title` / `--type-section`: Cinzel, strong parchment.
pub fn display<'a>(label: impl Into<String>, size: f32) -> iced::widget::Text<'a> {
    text(label.into()).font(fonts::DISPLAY_BOLD).size(size).color(theme::TEXT_STRONG)
}

/// Cinzel in gold, for the wordmark and dialog titles.
pub fn display_gold<'a>(label: impl Into<String>, size: f32) -> iced::widget::Text<'a> {
    text(label.into()).font(fonts::DISPLAY_BOLD).size(size).color(theme::TEXT_ACCENT)
}

/// `--type-panel-head`: small Cinzel heading inside panels and drawers.
pub fn panel_head<'a>(label: impl Into<String>) -> iced::widget::Text<'a> {
    text(label.into().to_uppercase()).font(fonts::DISPLAY_BOLD).size(13).color(theme::TEXT_STRONG)
}

/// `--type-label`: small uppercase UI label above a control.
pub fn label<'a>(label: impl Into<String>) -> iced::widget::Text<'a> {
    text(label.into().to_uppercase()).font(fonts::UI_SEMIBOLD).size(11).color(theme::TEXT_MUTED)
}

pub fn body<'a>(label: impl Into<String>) -> iced::widget::Text<'a> {
    text(label.into()).size(15).color(theme::TEXT_BODY)
}

pub fn body_strong<'a>(label: impl Into<String>) -> iced::widget::Text<'a> {
    text(label.into()).font(fonts::UI_MEDIUM).size(15).color(theme::TEXT_STRONG)
}

pub fn muted<'a>(label: impl Into<String>) -> iced::widget::Text<'a> {
    text(label.into()).size(13).color(theme::TEXT_MUTED)
}

pub fn faint<'a>(label: impl Into<String>) -> iced::widget::Text<'a> {
    text(label.into()).size(13).color(theme::TEXT_FAINT)
}

pub fn mono<'a>(label: impl Into<String>) -> iced::widget::Text<'a> {
    text(label.into()).font(fonts::MONO).size(12).color(theme::TEXT_BODY)
}

/// The Void mark.
pub fn mark<'a>(size: f32) -> iced::widget::Svg<'a> {
    svg(images::mark()).width(size).height(size)
}

/// Button caption. Filling the button keeps it centred however wide the button
/// is; inside a shrink-to-fit button the fill compresses to the text itself.
fn button_label<'a>(label: &str, size: f32) -> iced::widget::Text<'a> {
    text(label.to_string()).font(fonts::UI_SEMIBOLD).size(size).width(Fill).height(Fill).center()
}

/// `button()` adopts its content's fluid width, so a filling label would make every
/// button fill; start from shrink-to-fit and let callers widen explicitly.
fn labelled<'a>(label: &str, size: f32) -> iced::widget::Button<'a, Message> {
    button(button_label(label, size)).width(Length::Shrink)
}

/// The gold call to action; the site's hero button is 46 px tall.
pub fn primary_button<'a>(label: &str, on_press: Option<Message>) -> iced::widget::Button<'a, Message> {
    labelled(label, 15.0).style(style::primary_button).height(44).padding([0, 24]).on_press_maybe(on_press)
}

pub fn secondary_button<'a>(label: &str, on_press: Option<Message>) -> iced::widget::Button<'a, Message> {
    labelled(label, 14.0).style(style::secondary_button).height(38).padding([0, 18]).on_press_maybe(on_press)
}

pub fn outline_button<'a>(label: &str, on_press: Option<Message>) -> iced::widget::Button<'a, Message> {
    labelled(label, 14.0).style(style::outline_button).height(38).padding([0, 18]).on_press_maybe(on_press)
}

pub fn danger_button<'a>(label: &str, on_press: Option<Message>) -> iced::widget::Button<'a, Message> {
    labelled(label, 14.0).style(style::danger_button).height(38).padding([0, 18]).on_press_maybe(on_press)
}

pub fn ghost_button<'a>(label: &str, on_press: Option<Message>) -> iced::widget::Button<'a, Message> {
    labelled(label, 14.0).style(style::ghost_button).height(36).padding([0, 14]).on_press_maybe(on_press)
}

pub fn link_button<'a>(label: &str, on_press: Message) -> iced::widget::Button<'a, Message> {
    button(text(label.to_string()).size(13)).style(style::link_button).padding([2, 4]).on_press(on_press)
}

/// Small text-only close control for dialog and drawer headers.
pub fn close_button<'a>(on_press: Message) -> Element<'a, Message> {
    with_tooltip(button(text("×").size(22).width(28).height(28).center()).style(style::icon_button).padding(0).on_press(on_press), "Close (Esc)")
}

/// A panel: umber surface, hairline border, shallow corners.
pub fn panel<'a>(content: impl Into<Element<'a, Message>>, width: impl Into<Length>) -> iced::widget::Container<'a, Message> {
    container(content).style(style::panel).padding(22).width(width)
}

/// A label above a control.
pub fn field<'a>(name: &str, control: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![label(name), control.into()].spacing(6).width(Fill).into()
}

/// A plain 1 px rule in the subtle border colour.
pub fn rule<'a>() -> Element<'a, Message> {
    hairline(theme::BORDER_SUBTLE)
}

/// The site's gold hairline under the header.
pub fn rule_gold<'a>() -> Element<'a, Message> {
    hairline(theme::BORDER_GOLD)
}

fn hairline<'a>(color: iced::Color) -> Element<'a, Message> {
    container(iced::widget::space().width(Fill).height(1))
        .style(move |_| container::Style { background: Some(color.into()), ..container::Style::default() })
        .width(Fill)
        .into()
}

pub fn status_dot<'a>(level: NoticeLevel, pulse: bool) -> Element<'a, Message> {
    let (color, _) = level.colors();
    let color = if pulse { theme::alpha(color, 0.45) } else { color };
    text("●").size(10).color(color).into()
}

pub fn status_item<'a>(level: NoticeLevel, pulse: bool, label: String) -> Element<'a, Message> {
    row![status_dot(level, pulse), text(label).size(13).color(theme::TEXT_MUTED)].spacing(6).align_y(Alignment::Center).into()
}

pub fn notice_banner<'a>(level: NoticeLevel, message: &str, action: Option<(String, Message)>) -> Element<'a, Message> {
    let (fg, _) = level.colors();
    let mut content = row![text(message.to_string()).size(14).color(fg).width(Fill)].spacing(12).align_y(Alignment::Center);
    if let Some((label, msg)) = action {
        content = content.push(outline_button(&label, Some(msg)).height(32));
    }
    content = content.push(button(text("×").size(18).color(fg)).style(style::link_button).padding([0, 6]).on_press(Message::DismissNotice));
    container(content).style(style::notice(level)).padding([8, 14]).width(Fill).into()
}

pub fn with_tooltip<'a>(content: impl Into<Element<'a, Message>>, tip: &str) -> Element<'a, Message> {
    tooltip(content, container(text(tip.to_string()).size(13)).style(style::inset).padding([6, 10]), tooltip::Position::Top).gap(6).into()
}

/// Vertical scrollable with the launcher's slim gold scroller.
pub fn thin_scrollable<'a>(content: impl Into<Element<'a, Message>>) -> iced::widget::Scrollable<'a, Message> {
    scrollable(content).direction(Direction::Vertical(Scrollbar::new().width(6).scroller_width(6).margin(0))).style(style::scrollable_style)
}

pub fn vspace(height: f32) -> Space {
    iced::widget::space().height(height)
}

pub fn hspace(width: f32) -> Space {
    iced::widget::space().width(width)
}

pub fn fill_space() -> Space {
    iced::widget::space().width(Fill)
}

/// Section heading in the settings drawer: small Cinzel head over a hairline.
pub fn section<'a>(title: &str) -> Element<'a, Message> {
    column![panel_head(title), rule()].spacing(6).width(Fill).into()
}
