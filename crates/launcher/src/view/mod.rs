//! View composition: hero art, main screen, then whichever panel is open.

mod components;
mod java_help;
mod main_screen;
mod notices;
mod settings_panel;

use iced::widget::{container, image, mouse_area, opaque, stack};
use iced::{ContentFit, Element, Fill};

use crate::app::App;
use crate::message::Message;
use crate::state::Panel;
use crate::theme::{images, style};

pub fn view(app: &App) -> Element<'_, Message> {
    let background = image(images::background()).content_fit(ContentFit::Cover).width(Fill).height(Fill);
    // The site's `--scrim-bottom`: clear at the top, nearly solid at the bottom, so type reads over the art.
    let scrim = container(iced::widget::space().width(Fill).height(Fill)).width(Fill).height(Fill).style(style::hero_scrim);

    let mut layers = stack![background, scrim, main_screen::main_screen(app)];

    let panel: Option<Element<'_, Message>> = match app.panel {
        Panel::None => None,
        Panel::Settings => Some(settings_panel::settings_drawer(app)),
        Panel::JavaHelp => Some(java_help::java_help(app)),
        Panel::BugReport => Some(notices::bug_report(app)),
        Panel::LauncherUpdate => Some(notices::launcher_update(app)),
        Panel::FirstRun => Some(notices::first_run(app)),
    };
    if let Some(panel) = panel {
        // iced's modal pattern: the scrim is the background of the container that holds the
        // panel, so the panel can never end up underneath it; `opaque` keeps clicks from
        // reaching the screen below, and the scrim itself closes dismissable panels.
        let dismissable = !matches!(app.panel, Panel::FirstRun);
        let backdrop = container(opaque(panel)).width(Fill).height(Fill).style(style::scrim);
        let backdrop: Element<'_, Message> = if dismissable { mouse_area(backdrop).on_press(Message::ClosePanel).into() } else { backdrop.into() };
        layers = layers.push(opaque(backdrop));
    }
    layers.width(Fill).height(Fill).into()
}
