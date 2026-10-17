use iced::widget::{column, container, progress_bar, row, text};
use iced::{Alignment, Element, Fill, Length};

use super::components::*;
use crate::app::App;
use crate::message::Message;
use crate::state::{ClientState, JavaState, ReportSource, SelfUpdateState};
use crate::theme::fonts;
use crate::theme::style::NoticeLevel;
use crate::theme::{self, style};

fn dialog<'a>(content: impl Into<Element<'a, Message>>, width: f32) -> Element<'a, Message> {
    iced::widget::center(container(content).style(style::dialog).padding(26).width(Length::Fixed(width))).into()
}

fn dialog_title<'a>(title: impl Into<String>) -> Element<'a, Message> {
    row![mark(30.0), display_gold(title, 17.0)].spacing(12).align_y(Alignment::Center).into()
}

/// The crash / bug-report dialog: three numbered steps, big buttons, no jargon.
pub fn bug_report<'a>(app: &'a App) -> Element<'a, Message> {
    let Some(view) = &app.report else {
        return dialog(column![body("Nothing to report."), ghost_button("Close", Some(Message::DismissReport))].spacing(12), 480.0);
    };
    let report = app.current_report();
    let is_crash = !matches!(view.source, ReportSource::Logs);
    let intro = if is_crash { "Sorry about that. Help us fix it, it only takes three clicks:" } else { "Tell us what went wrong. It only takes three clicks:" };

    let copy_label = if view.copied {
        "1.  Copied!"
    } else if is_crash {
        "1.  Copy crash report"
    } else {
        "1.  Copy report"
    };
    let step = |button: Element<'a, Message>, hint: &str| row![button, muted(hint.to_string())].spacing(12).align_y(Alignment::Center);
    let steps = column![
        step(
            primary_button(copy_label, (!view.copied).then_some(Message::CopyReport)).width(Length::Fixed(300.0)).into(),
            "puts the report below on your clipboard"
        ),
        step(
            secondary_button("2.  Open an issue on GitHub", Some(Message::OpenIssue)).width(Length::Fixed(300.0)).height(44).into(),
            "opens a new issue with the report filled in (sign in if asked)"
        ),
        text("3.  Say what you were doing, then press Create. Paste (Ctrl+V) if the text is missing.").size(15).font(fonts::UI_MEDIUM).color(theme::TEXT_BODY),
    ]
    .spacing(10);

    let mut content = column![dialog_title(report.headline.clone()), body(intro), steps].spacing(12).width(Fill);
    if let Some(hint) = report.hint {
        content = content.push(text(hint).size(14).color(theme::AMBER_500));
    }
    // The window is 600 px tall; a hint costs the preview two lines.
    let preview_height = if report.hint.is_some() { 130.0 } else { 180.0 };
    content = content.push(label("What gets shared (no account name or password)"));
    content = content.push(
        container(thin_scrollable(mono(report.text.clone()).width(Fill)).height(Length::Fixed(preview_height))).style(style::inset).padding(10).width(Fill),
    );

    let file_note = match &report.file {
        Some(path) => format!(
            "Full report: {}. If a developer asks for it, drag that file into the issue.",
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        ),
        None => String::new(),
    };
    let mut buttons = row![].spacing(8).align_y(Alignment::Center);
    if report.file.is_some() {
        buttons = buttons.push(secondary_button("Show file", Some(Message::ShowReportFile)));
    }
    buttons = buttons.push(fill_space());
    buttons = buttons.push(ghost_button("Close", Some(Message::DismissReport)));
    if matches!(view.source, ReportSource::LaunchFailure { .. }) {
        buttons = buttons.push(primary_button("Try again", Some(Message::RetryLaunch)).height(38));
    }
    content = content.push(column![faint(file_note), buttons].spacing(8));
    dialog(content, 720.0)
}

pub fn launcher_update(app: &App) -> Element<'_, Message> {
    let content: Element<'_, Message> = match &app.self_update {
        SelfUpdateState::Available(u) => column![
            dialog_title(format!("Launcher update {}", u.version)),
            body(if u.required {
                "This update is required to keep working with the current client and servers."
            } else {
                "A newer launcher is available. It installs in a few seconds and restarts."
            }),
            muted(format!("You have {}.", app.build.version)),
            row![
                link_button("What's new", Message::OpenUrl(u.notes_url.clone())),
                fill_space(),
                ghost_button("Later", Some(Message::DismissLauncherUpdate)),
                primary_button("Update and restart", Some(Message::StageLauncherUpdate))
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(14)
        .into(),
        SelfUpdateState::Staging { update, progress } => column![
            dialog_title(format!("Downloading launcher {}", update.version)),
            progress_bar(0.0..=1.0, progress.fraction()).length(Fill).girth(10).style(style::progress_style),
            mono(format!("{}  {}", progress.phase, progress.detail())),
        ]
        .spacing(14)
        .into(),
        SelfUpdateState::Staged { update, .. } => column![
            dialog_title(format!("Launcher {} is ready", update.version)),
            body("The launcher will close, replace itself and start again."),
            row![fill_space(), ghost_button("Later", Some(Message::DismissLauncherUpdate)), primary_button("Restart now", Some(Message::ApplyLauncherUpdate))]
                .spacing(8),
        ]
        .spacing(14)
        .into(),
        SelfUpdateState::Manual { version, url } => column![
            dialog_title(format!("Launcher {version} available")),
            body("This version of the launcher is too old to update itself. Download the new one from the releases page."),
            row![
                fill_space(),
                ghost_button("Later", Some(Message::DismissLauncherUpdate)),
                primary_button("Open releases", Some(Message::OpenUrl(url.clone())))
            ]
            .spacing(8),
        ]
        .spacing(14)
        .into(),
        _ => column![body("The launcher is up to date."), row![fill_space(), ghost_button("Close", Some(Message::ClosePanel))]].spacing(14).into(),
    };
    dialog(content, 520.0)
}

pub fn first_run(app: &App) -> Element<'_, Message> {
    let java_row = match &app.java {
        JavaState::Checking => status_item(NoticeLevel::Info, app.tick % 2 == 0, "Java · checking…".into()),
        JavaState::Ready(j) => status_item(NoticeLevel::Success, false, format!("Java · {} ready", j.full_version)),
        JavaState::Problem(_) | JavaState::InstallFailed(_) => status_item(NoticeLevel::Warning, false, "Java · will be installed (about 50 MB)".into()),
        JavaState::Installing(p) => status_item(NoticeLevel::Info, app.tick % 2 == 0, format!("Java · {} {}", p.phase, p.detail())),
    };
    let client_row = match &app.client {
        ClientState::Checking => status_item(NoticeLevel::Info, app.tick % 2 == 0, "Client · checking…".into()),
        ClientState::Current(c) => status_item(NoticeLevel::Success, false, format!("Client · {} ready", c.label())),
        ClientState::UpdateAvailable { remote, .. } => status_item(
            NoticeLevel::Warning,
            false,
            if remote.size > 0 {
                format!("Client · {} will be downloaded ({})", remote.version, launcher_core::progress::format_bytes(remote.size))
            } else {
                format!("Client · {} will be downloaded", remote.version)
            },
        ),
        ClientState::Downloading { progress, .. } => {
            status_item(NoticeLevel::Info, app.tick % 2 == 0, format!("Client · {} {}", progress.phase, progress.detail()))
        }
        ClientState::Offline { installed: Some(c), .. } => status_item(NoticeLevel::Warning, false, format!("Client · {} (offline)", c.label())),
        ClientState::Offline { installed: None, .. } | ClientState::Failed { .. } => {
            status_item(NoticeLevel::Error, false, "Client · could not be downloaded, check your connection".into())
        }
    };
    let busy = app.onboarding || matches!(app.java, JavaState::Installing(_)) || matches!(app.client, ClientState::Downloading { .. });
    let content = column![
        dialog_title("Welcome to Void"),
        body("The launcher keeps the game client and a private Java runtime up to date inside ~/.void. The game's own cache in ~/.jagex_cache_32 stays where it is."),
        column![java_row, client_row].spacing(6),
        row![fill_space(), ghost_button("Skip", Some(Message::ClosePanel)), primary_button(if busy { "Setting up…" } else { "Get started" }, (!busy).then_some(Message::GetStarted))]
            .spacing(8),
    ]
    .spacing(16);
    dialog(content, 560.0)
}
