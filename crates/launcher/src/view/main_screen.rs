use iced::widget::{column, container, pick_list, progress_bar, row, text, text_input};
use iced::{Alignment, Element, Fill, Length};

use super::components::*;
use crate::app::App;
use crate::links::Link;
use crate::message::Message;
use crate::state::{ClientState, JavaState, LaunchState, Panel, SelfUpdateState, ServerKind};
use crate::theme::style::NoticeLevel;
use crate::theme::{self, fonts, style};
use launcher_core::settings::Renderer;

pub const HEADER_HEIGHT: f32 = 56.0;
const PANEL_WIDTH: f32 = 400.0;

pub fn main_screen(app: &App) -> Element<'_, Message> {
    let header = container(
        row![
            row![mark(26.0), text("VOID").font(fonts::DISPLAY_BOLD).size(17).color(theme::PARCH_50)].spacing(10).align_y(Alignment::Center),
            fill_space(),
            ghost_button("Settings", Some(Message::ShowPanel(Panel::Settings))),
        ]
        .align_y(Alignment::Center)
        .width(Fill),
    )
    .style(style::header)
    .padding([0, 20])
    .height(HEADER_HEIGHT)
    .width(Fill)
    .center_y(HEADER_HEIGHT);

    let hero = column![
        iced::widget::space().height(Fill),
        display("RuneScape Revived", 34.0),
        text("Rediscover 2011 RuneScape with modern server emulation at your fingertips.").size(17).color(theme::PARCH_200),
        vspace(4.0),
        text("Windows · macOS · Linux · Java 21").font(fonts::MONO).size(12).color(theme::PARCH_300),
    ]
    .spacing(10)
    .width(Fill)
    .height(Fill);

    let content = row![hero, play_panel(app)].spacing(28).align_y(Alignment::End).padding([24, 28]).width(Fill).height(Fill);

    let mut bottom = column![].spacing(0).width(Fill);
    if let Some(notice) = &app.notice {
        bottom = bottom.push(container(notice_banner(notice.level, &notice.text, notice.action.clone())).padding([0, 28]).width(Fill));
        bottom = bottom.push(vspace(10.0));
    }
    bottom = bottom.push(status_bar(app));

    column![header, rule_gold(), content, bottom].width(Fill).height(Fill).into()
}

fn play_panel(app: &App) -> Element<'_, Message> {
    let server_kind = ServerKind::of(&app.settings.server);
    let mut server = column![
        label("World"),
        pick_list(ServerKind::ALL, Some(server_kind), Message::ServerPicked)
            .width(Fill)
            .padding([8, 12])
            .text_size(14)
            .style(style::pick_list_style)
            .menu_style(style::menu_style),
    ]
    .spacing(6);
    if server_kind == ServerKind::Custom {
        server = server.push(
            row![
                text_input("Host or IP address", &app.form.custom_host)
                    .on_input(Message::CustomHostChanged)
                    .on_submit(Message::PlayPressed)
                    .padding([8, 12])
                    .size(14)
                    .style(style::text_input_style)
                    .width(Fill),
                text_input("43594", &app.form.custom_port)
                    .on_input(Message::CustomPortChanged)
                    .on_submit(Message::PlayPressed)
                    .padding([8, 12])
                    .size(14)
                    .style(style::text_input_style)
                    .width(90),
            ]
            .spacing(8),
        );
        let keys_label: String = match &app.settings.custom_keys {
            Some(path) => path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string_lossy().into_owned()),
            None => "Standard Void keys".into(),
        };
        let mut keys = row![secondary_button("Server keys…", Some(Message::BrowseKeysFile)).height(34), muted(keys_label).width(Fill)]
            .spacing(10)
            .align_y(Alignment::Center);
        if app.settings.custom_keys.is_some() {
            keys = keys.push(link_button("Clear", Message::ClearKeysFile));
        }
        server = server.push(keys);
        match &app.form.server_error {
            Some(error) => server = server.push(text(error.clone()).size(13).color(theme::EMBER_500)),
            None => server = server.push(faint("A public server should have its own keys; ask its owner for game.properties.")),
        }
    }

    let renderer = field(
        "Renderer",
        pick_list(Renderer::ALL, Some(app.settings.renderer), Message::RendererPicked)
            .width(Fill)
            .padding([8, 12])
            .text_size(14)
            .style(style::pick_list_style)
            .menu_style(style::menu_style),
    );

    let java_line: Element<'_, Message> = match &app.java {
        JavaState::Installing(progress) => column![
            progress_bar(0.0..=1.0, progress.fraction()).length(Fill).girth(8).style(style::progress_style),
            mono(format!("{}  {}", progress.phase, progress.detail())),
        ]
        .spacing(6)
        .into(),
        _ => iced::widget::space().height(0).into(),
    };

    let action: Element<'_, Message> = match &app.client {
        ClientState::Downloading { progress, .. } => column![
            progress_bar(0.0..=1.0, progress.fraction()).length(Fill).girth(10).style(style::progress_style),
            row![mono(format!("{}  {}", progress.phase, progress.detail())).width(Fill), ghost_button("Cancel", Some(Message::CancelDownload))]
                .align_y(Alignment::Center)
                .spacing(8),
        ]
        .spacing(8)
        .into(),
        _ => {
            let (label, enabled) = play_label(app);
            primary_button(label, enabled.then_some(Message::PlayPressed)).height(46).width(Fill).into()
        }
    };

    let inner = column![server, renderer, java_line, action].spacing(16).width(Fill);
    panel(inner, Length::Fixed(PANEL_WIDTH)).into()
}

fn play_label(app: &App) -> (&'static str, bool) {
    if matches!(app.launch, LaunchState::Starting) {
        return ("Starting…", false);
    }
    match &app.java {
        JavaState::Checking => return ("Checking…", false),
        JavaState::Installing(_) => return ("Installing Java…", false),
        JavaState::Problem(_) | JavaState::InstallFailed(_) => return ("Install Java", true),
        JavaState::Ready(_) => {}
    }
    match &app.client {
        ClientState::Checking => ("Checking…", false),
        ClientState::Downloading { .. } => ("Downloading…", false),
        ClientState::UpdateAvailable { installed: None, .. } => ("Download & Play", true),
        ClientState::UpdateAvailable { installed: Some(_), .. } => ("Update & Play", true),
        ClientState::Current(_) => ("Play", true),
        ClientState::Offline { installed: Some(_), .. } | ClientState::Failed { installed: Some(_), .. } => ("Play", true),
        ClientState::Offline { .. } | ClientState::Failed { .. } => ("Retry", true),
    }
}

fn status_bar(app: &App) -> Element<'_, Message> {
    let pulse = app.tick % 2 == 0;
    let client_item = match &app.client {
        ClientState::Checking => status_item(NoticeLevel::Info, pulse, "Client · checking…".into()),
        ClientState::Current(c) => status_item(NoticeLevel::Success, false, format!("Client {} · {} · up to date", c.label(), app.settings.channel)),
        ClientState::UpdateAvailable { installed: Some(c), remote } => {
            status_item(NoticeLevel::Warning, false, format!("Client {} · update to {} available", c.label(), remote.version))
        }
        ClientState::UpdateAvailable { installed: None, remote } => {
            status_item(NoticeLevel::Warning, false, format!("Client {} · not downloaded yet", remote.version))
        }
        ClientState::Downloading { remote, progress, .. } => status_item(
            NoticeLevel::Info,
            pulse,
            format!("Client {} · downloading {}", remote.version, progress.percent().map(|p| format!("{p}%")).unwrap_or_default()),
        ),
        ClientState::Offline { installed: Some(c), .. } => status_item(NoticeLevel::Warning, false, format!("Client {} · offline", c.label())),
        ClientState::Offline { installed: None, .. } => status_item(NoticeLevel::Error, false, "Client · offline, nothing installed".into()),
        ClientState::Failed { installed, .. } => status_item(
            NoticeLevel::Error,
            false,
            match installed {
                Some(c) => format!("Client {} · update failed", c.label()),
                None => "Client · download failed".into(),
            },
        ),
    };
    let java_item = match &app.java {
        JavaState::Checking => status_item(NoticeLevel::Info, pulse, "Java · checking…".into()),
        JavaState::Ready(j) => status_item(
            if j.warnings.is_empty() { NoticeLevel::Success } else { NoticeLevel::Warning },
            false,
            format!("Java {} · {}", j.full_version, j.origin.label()),
        ),
        JavaState::Problem(p) => status_item(NoticeLevel::Error, false, format!("Java · {p}")),
        JavaState::Installing(_) => status_item(NoticeLevel::Info, pulse, "Java · installing…".into()),
        JavaState::InstallFailed(_) => status_item(NoticeLevel::Error, false, "Java · install failed".into()),
    };
    let launcher_text = match &app.self_update {
        SelfUpdateState::Checking => format!("Launcher {} · checking…", app.build.version),
        SelfUpdateState::Available(u) => format!("Launcher {} · update {} available", app.build.version, u.version),
        SelfUpdateState::Staged { update, .. } => format!("Launcher {} · {} ready to install", app.build.version, update.version),
        SelfUpdateState::Manual { version, .. } => format!("Launcher {} · {version} needs a manual update", app.build.version),
        _ => format!("Launcher {}", app.build.version),
    };
    let launcher_item: Element<'_, Message> = match &app.self_update {
        SelfUpdateState::Available(_) => {
            row![text(launcher_text).size(13).color(theme::AMBER_500), link_button("Update", Message::ShowPanel(Panel::LauncherUpdate))]
                .spacing(4)
                .align_y(Alignment::Center)
                .into()
        }
        SelfUpdateState::Staged { .. } => row![text(launcher_text).size(13).color(theme::AMBER_500), link_button("Restart now", Message::ApplyLauncherUpdate)]
            .spacing(4)
            .align_y(Alignment::Center)
            .into(),
        _ => text(launcher_text).size(13).color(theme::TEXT_FAINT).into(),
    };

    let dot = || text("·").size(13).color(theme::TEXT_FAINT);
    let links = row![
        link_button("Report a problem", Message::ReportProblem),
        dot(),
        link_button("GitHub", Message::OpenLink(Link::ClientSource)),
        dot(),
        link_button("Website", Message::OpenLink(Link::Website)),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    column![
        rule(),
        container(
            row![column![client_item, java_item].spacing(4).width(Fill), column![launcher_item, links].spacing(4).align_x(Alignment::End)]
                .align_y(Alignment::Center)
                .width(Fill),
        )
        .style(style::status_bar)
        .padding([10, 28])
        .width(Fill)
    ]
    .into()
}
