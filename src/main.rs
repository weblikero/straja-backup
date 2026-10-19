#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod ui;
fn main() -> iced::Result {
    #[cfg(debug_assertions)]
    if std::env::args().any(|arg| arg == "--preview" || arg == "--preview-updates") {
        return ui::preview();
    }
    straja::i18n::reload();
    let _ = straja::tray::launch();
    if straja::tray::show_existing_window() {
        return Ok(());
    }
    ui::run()
}
