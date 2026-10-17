use iced::widget::{checkbox, column, container, pick_list, radio, row, text, text_input};
use iced::{Alignment, Element, Fill, Length, Padding};

use super::components::*;
use crate::app::App;
use crate::links::Link;
use crate::message::Message;
use crate::state::{ClientState, HeapChoice, JavaSourceKind, JavaState, Panel, SelfUpdateState};
use crate::theme::{self, style};
use launcher_core::settings::{AfterLaunch, Extra, HEAP_CHOICES_GB};
use launcher_core::version::Channel;

const DRAWER_WIDTH: f32 = 440.0;

pub fn settings_drawer(app: &App) -> Element<'_, Message> {
    let header = container(
        column![
            row![display_gold("Settings", 16.0), fill_space(), close_button(Message::ClosePanel)].align_y(Alignment::Center).width(Fill),
            vspace(12.0),
            rule_gold(),
        ]
        .width(Fill),
    )
    .style(style::drawer_header)
    .padding(Padding { top: 16.0, right: 22.0, bottom: 0.0, left: 22.0 })
    .width(Fill);

    let sections = column![updates(app), game(app), java(app), performance(app), launcher(app), about(app)].spacing(26).width(Fill);
    let body = thin_scrollable(container(sections).padding(Padding { top: 18.0, right: 16.0, bottom: 22.0, left: 22.0 }).width(Fill)).width(Fill).height(Fill);

    let drawer = container(column![header, body].width(Fill).height(Fill)).style(style::drawer).width(Length::Fixed(DRAWER_WIDTH)).height(Fill);
    row![fill_space(), drawer].width(Fill).height(Fill).into()
}

fn updates(app: &App) -> Element<'_, Message> {
    let channel_button = |channel: Channel| {
        let selected = app.settings.channel == channel;
        let b = if selected {
            primary_button(channel.label(), Some(Message::ChannelPicked(channel)))
        } else {
            outline_button(channel.label(), Some(Message::ChannelPicked(channel)))
        };
        b.height(36).width(Fill)
    };
    let client_text = match &app.client {
        ClientState::Current(c) => format!("Client {} · up to date", c.label()),
        ClientState::UpdateAvailable { installed: Some(c), remote } => format!("Client {} · {} available", c.label(), remote.version),
        ClientState::UpdateAvailable { installed: None, remote } => format!("Client {} · not downloaded", remote.version),
        ClientState::Checking => "Client · checking…".into(),
        ClientState::Downloading { remote, .. } => format!("Client {} · downloading", remote.version),
        ClientState::Offline { installed, .. } | ClientState::Failed { installed, .. } => match installed {
            Some(c) => format!("Client {} · could not check for updates", c.label()),
            None => "Client · not installed".into(),
        },
    };
    let launcher_text = match &app.self_update {
        SelfUpdateState::UpToDate | SelfUpdateState::Skipped => format!("Launcher {} · up to date", app.build.version),
        SelfUpdateState::Checking => format!("Launcher {} · checking…", app.build.version),
        SelfUpdateState::Available(u) | SelfUpdateState::Staging { update: u, .. } | SelfUpdateState::Staged { update: u, .. } => {
            format!("Launcher {} · {} available", app.build.version, u.version)
        }
        SelfUpdateState::Manual { version, .. } => format!("Launcher {} · {version} available (manual update)", app.build.version),
        SelfUpdateState::Disabled(by) => format!("Launcher {} · updated by {by}", app.build.version),
        SelfUpdateState::Offline(_) => format!("Launcher {} · could not check for updates", app.build.version),
    };
    let checking = matches!(app.client, ClientState::Checking) || matches!(app.self_update, SelfUpdateState::Checking);
    column![
        section("Updates"),
        label("Channel"),
        row![channel_button(Channel::Stable), channel_button(Channel::Development)].spacing(8),
        muted("Applies to both the launcher and the client. Development follows every push to main; Stable follows tagged releases."),
        vspace(2.0),
        column![muted(client_text), muted(launcher_text)].spacing(4),
        secondary_button(if checking { "Checking…" } else { "Check for updates" }, (!checking).then_some(Message::CheckForUpdates)).width(Fill),
    ]
    .spacing(8)
    .into()
}

fn game(app: &App) -> Element<'_, Message> {
    let mut world = row![
        text_input("1", &app.form.world).on_input(Message::WorldChanged).padding([6, 12]).size(14).style(style::text_input_style).width(Length::Fixed(80.0)),
        muted("The world to log in to. 2011Scape is world 1.").width(Fill),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if let Some(error) = &app.form.world_error {
        world = row![text(error.clone()).size(13).color(theme::EMBER_500)];
    }
    let mut extras = column![].spacing(10);
    for extra in Extra::ALL {
        extras = extras.push(
            column![
                checkbox(app.settings.extras.get(extra))
                    .label(extra.label())
                    .on_toggle(move |on| Message::ExtraToggled(extra, on))
                    .style(style::checkbox_style)
                    .text_size(14),
                container(faint(extra.description())).padding(Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 28.0 }),
            ]
            .spacing(2),
        );
    }
    column![
        section("Game"),
        label("World"),
        world,
        vspace(4.0),
        label("Extras"),
        muted("Off by default, as in the client. Most can also be toggled in game with a :: command."),
        extras,
    ]
    .spacing(8)
    .into()
}

fn java(app: &App) -> Element<'_, Message> {
    let kind = JavaSourceKind::of(&app.settings.java);
    let java_status: Element<'_, Message> = match &app.java {
        JavaState::Ready(j) => muted(format!("Using {}{}", j.summary(), if j.warnings.is_empty() { "" } else { " · see warnings below" })).into(),
        JavaState::Checking => muted("Checking…").into(),
        JavaState::Installing(p) => muted(format!("{} {}", p.phase, p.detail())).into(),
        JavaState::Problem(p) => {
            row![text(format!("{p}")).size(14).color(theme::EMBER_500).width(Fill), outline_button("Fix", Some(Message::ShowPanel(Panel::JavaHelp))).height(32)]
                .align_y(Alignment::Center)
                .spacing(8)
                .into()
        }
        JavaState::InstallFailed(e) => text(format!("Install failed: {e}")).size(14).color(theme::EMBER_500).into(),
    };
    let managed = launcher_core::java::runtime::installed(&app.paths);
    let managed_note = match &managed {
        Some(info) => format!("Managed runtime · Temurin {} in ~/.void/runtime", info.semver),
        None => "Managed runtime · Temurin 21, installed on demand into ~/.void/runtime (recommended)".into(),
    };
    let mut java = column![
        section("Java"),
        radio(managed_note, JavaSourceKind::Managed, Some(kind), Message::JavaSourcePicked).style(style::radio_style).text_size(14),
        radio("System Java (JAVA_HOME, PATH, common install folders)", JavaSourceKind::System, Some(kind), Message::JavaSourcePicked)
            .style(style::radio_style)
            .text_size(14),
        radio("Custom path", JavaSourceKind::Custom, Some(kind), Message::JavaSourcePicked).style(style::radio_style).text_size(14),
    ]
    .spacing(8);
    if kind == JavaSourceKind::Custom {
        java = java.push(
            column![
                text_input("/path/to/java-home", &app.form.custom_java_path)
                    .on_input(Message::CustomJavaPathChanged)
                    .on_submit(Message::JavaSourcePicked(JavaSourceKind::Custom))
                    .padding([8, 12])
                    .size(14)
                    .style(style::text_input_style)
                    .width(Fill),
                row![
                    secondary_button("Browse…", Some(Message::BrowseJavaPath)).width(Fill),
                    primary_button("Apply", Some(Message::JavaSourcePicked(JavaSourceKind::Custom))).height(38).width(Fill),
                ]
                .spacing(8),
            ]
            .spacing(8),
        );
    }
    java = java.push(java_status);
    if let JavaState::Ready(j) = &app.java {
        for w in &j.warnings {
            let msg = match w {
                launcher_core::java::JavaWarning::UnsafeDeprecated => {
                    "Java 24+ prints deprecation warnings for the client's memory access; Java 21 is recommended."
                }
                launcher_core::java::JavaWarning::TooNew => "The client is tested on Java 21 to 25; this Java is newer. Java 21 is recommended.",
                launcher_core::java::JavaWarning::Rosetta => "This Java runs under Rosetta; an arm64 build would be faster.",
                launcher_core::java::JavaWarning::NonHotSpot => "Not a HotSpot JVM; ZGC is unavailable and the client is untested on it.",
            };
            java = java.push(text(msg).size(13).color(theme::AMBER_500));
        }
    }
    let busy = matches!(app.java, JavaState::Checking | JavaState::Installing(_));
    let install_label = if managed.is_some() { "Reinstall Java 21" } else { "Install Java 21" };
    java.push(
        row![
            secondary_button(install_label, (!busy).then_some(Message::InstallJava)).width(Fill),
            secondary_button("Re-check", (!busy).then_some(Message::RecheckJava)).width(Fill)
        ]
        .spacing(8),
    )
    .into()
}

fn performance(app: &App) -> Element<'_, Message> {
    let heap_choices: Vec<HeapChoice> = HEAP_CHOICES_GB.iter().map(|g| HeapChoice(*g)).collect();
    column![
        section("Performance"),
        row![
            label("Max heap").width(Fill),
            pick_list(heap_choices, Some(HeapChoice(app.settings.max_heap_gb)), Message::HeapPicked)
                .padding([6, 12])
                .text_size(14)
                .width(Length::Fixed(110.0))
                .style(style::pick_list_style)
                .menu_style(style::menu_style)
        ]
        .align_y(Alignment::Center),
        muted("2 GB suits the client. Raise it only if you see out-of-memory errors; it is capped to fit your computer's memory."),
        vspace(2.0),
        label("Extra JVM arguments"),
        text_input("-Dclient.fps=60", &app.settings.extra_jvm_args)
            .on_input(Message::ExtraArgsChanged)
            .padding([8, 12])
            .size(13)
            .font(crate::theme::fonts::MONO)
            .style(style::text_input_style)
            .width(Fill),
        faint("Passed to java before -jar. Client options (--fps, --scale, --msaa…) belong in the client's own settings, not here."),
    ]
    .spacing(8)
    .into()
}

fn launcher(app: &App) -> Element<'_, Message> {
    column![
        section("Launcher"),
        checkbox(app.settings.after_launch == AfterLaunch::StayOpen)
            .label("Keep the launcher open after Play (start several clients)")
            .on_toggle(Message::KeepOpenToggled)
            .style(style::checkbox_style)
            .text_size(14),
        vspace(2.0),
        label("Folders"),
        row![
            secondary_button("Logs", Some(Message::OpenLogsFolder)).width(Fill),
            secondary_button("Report a problem", Some(Message::ReportProblem)).width(Fill)
        ]
        .spacing(8),
        faint("Everything the launcher owns is in ~/.void. The game cache in ~/.jagex_cache_32 belongs to the client and is never touched."),
    ]
    .spacing(8)
    .into()
}

fn about(app: &App) -> Element<'_, Message> {
    column![
        section("About"),
        muted(format!("Void Launcher {} ({}) · {}", app.build.version, app.build.short_commit(), app.build.target)),
        muted(format!("Built {}", app.build.built_at)),
        row![
            link_button("Website", Message::OpenLink(Link::Website)),
            link_button("Client source", Message::OpenLink(Link::ClientSource)),
            link_button("Launcher source", Message::OpenLink(Link::LauncherSource)),
            link_button("Issues", Message::OpenLink(Link::Issues)),
            link_button("Releases", Message::OpenLink(Link::LauncherReleases))
        ]
        .spacing(10),
        faint("AGPL-3.0. Fonts: Cinzel, Inter Tight, JetBrains Mono (SIL Open Font License). Artwork from the Void site."),
    ]
    .spacing(8)
    .into()
}
