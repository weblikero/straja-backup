#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod ui;
fn main() -> iced::Result {
    if let Some(path) = std::env::args_os()
        .skip_while(|a| a != "--apply-update")
        .nth(1)
    {
        if let Err(error) = straja::updater::apply(std::path::Path::new(&path)) {
            straja::updater::notify(&format!(
                "{}\n\n{error:#}",
                straja::i18n::tr("Actualizarea Straja nu a putut porni.")
            ));
        }
        return Ok(());
    }
    #[cfg(any(debug_assertions, feature = "ui-preview"))]
    if std::env::args().any(|arg| arg == "--preview" || arg == "--preview-updates") {
        return ui::preview();
    }
    straja::i18n::reload();
    if straja::updater::running() {
        straja::updater::notify(&straja::i18n::tr(
            "Actualizarea Straja este în curs. Aplicația se va redeschide la final.",
        ));
        return Ok(());
    }
    let _ = straja::tray::launch();
    if straja::tray::show_existing_window() {
        return Ok(());
    }
    ui::run()
}
