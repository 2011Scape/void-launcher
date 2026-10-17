use iced::widget::{column, container, progress_bar, row, text};
use iced::{Alignment, Element, Fill, Length};
use launcher_core::java::JavaProblem;
use launcher_core::java::copy::{explain, install_commands};

use super::components::*;
use crate::app::App;
use crate::links::Link;
use crate::message::Message;
use crate::state::JavaState;
use crate::theme::{self, style};

pub fn java_help(app: &App) -> Element<'_, Message> {
    let (title, body_text, offer_managed, installing, failed): (String, String, bool, Option<&crate::state::ProgressView>, Option<String>) = match &app.java {
        JavaState::Problem(problem) => {
            let e = explain(problem);
            (e.title, e.body, e.offer_managed, None, None)
        }
        JavaState::Installing(progress) => {
            ("Installing Java 21".into(), "Downloading Eclipse Temurin 21 into ~/.void/runtime.".into(), false, Some(progress), None)
        }
        JavaState::InstallFailed(reason) => {
            let e = explain(&JavaProblem::NotFound);
            (
                e.title,
                format!("Couldn't download Java: {reason}. Check your connection and try again, or install Java manually."),
                true,
                None,
                Some(reason.clone()),
            )
        }
        JavaState::Checking => ("Checking Java".into(), "Looking for a Java 21 installation…".into(), false, None, None),
        JavaState::Ready(j) => ("Java is ready".into(), format!("Using {}.", j.summary()), false, None, None),
    };
    let _ = failed;

    let mut content = column![row![mark(30.0), display_gold(title, 17.0)].spacing(12).align_y(Alignment::Center), body(body_text),].spacing(14).width(Fill);

    if let Some(progress) = installing {
        content = content.push(
            column![
                progress_bar(0.0..=1.0, progress.fraction()).length(Fill).girth(10).style(style::progress_style),
                mono(format!("{}  {}", progress.phase, progress.detail()))
            ]
            .spacing(6),
        );
    } else if offer_managed {
        content = content.push(
            column![
                primary_button("Install Java 21 for me", Some(Message::InstallJava)).width(Fill),
                faint(
                    "Downloads Eclipse Temurin 21 (about 50 MB) into ~/.void/runtime. Nothing else on your computer changes; delete that folder to remove it."
                ),
            ]
            .spacing(6),
        );
        content = content.push(rule());
        content = content.push(label("Or install it yourself"));
        for (label, command) in install_commands() {
            content = content.push(
                row![
                    text(label).size(14).color(theme::TEXT_MUTED).width(Length::Fixed(140.0)),
                    container(mono(command.clone())).style(style::inset).padding([6, 10]).width(Fill),
                    outline_button("Copy", Some(Message::CopyText(command))).height(32),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        }
        content = content.push(
            row![
                link_button("Download from adoptium.net", Message::OpenLink(Link::Adoptium)),
                fill_space(),
                outline_button("Choose a custom path", Some(Message::BrowseJavaPath)),
                secondary_button("Re-check", Some(Message::RecheckJava)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    content = content.push(row![fill_space(), ghost_button("Close", Some(Message::ClosePanel))]);

    iced::widget::center(container(content).style(style::dialog).padding(26).width(Length::Fixed(620.0))).into()
}
