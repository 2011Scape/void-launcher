#![windows_subsystem = "windows"]

mod app;
mod links;
mod message;
mod state;
mod tasks;
mod theme;
mod view;

use iced::window;

fn app_theme(_app: &app::App) -> iced::Theme {
    theme::theme()
}

fn main() -> iced::Result {
    // Before anything else so even a crash during boot leaves a report behind.
    if let Ok(paths) = launcher_core::VoidPaths::resolve() {
        launcher_core::report::install_panic_hook(paths, app::build_identity());
    }
    let icon = window::icon::from_file_data(theme::images::ICON_PNG, None).ok();
    iced::application(app::App::boot, app::App::update, app::App::view)
        .title(app::App::title)
        .subscription(app::App::subscription)
        .theme(app_theme)
        .font(theme::fonts::CINZEL_REGULAR_BYTES)
        .font(theme::fonts::CINZEL_BOLD_BYTES)
        .font(theme::fonts::UI_REGULAR_BYTES)
        .font(theme::fonts::UI_MEDIUM_BYTES)
        .font(theme::fonts::UI_SEMIBOLD_BYTES)
        .font(theme::fonts::MONO_BYTES)
        .default_font(theme::fonts::UI)
        .antialiasing(true)
        .window(window::Settings {
            size: iced::Size::new(app::WINDOW_WIDTH, app::WINDOW_HEIGHT),
            min_size: Some(iced::Size::new(app::WINDOW_WIDTH, app::WINDOW_HEIGHT)),
            resizable: false,
            icon,
            ..window::Settings::default()
        })
        .centered()
        .run()
}
