//! Opening the explorer in the default browser.

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

use crate::launch::{browser_target, Launcher};

/// Opens the explorer in the default browser, through a fresh launch file
/// `launcher` writes (spec: LA-21).
///
/// What the browser is handed is the file's `file:` URL, never the ticket in
/// it. Where the browser could not be asked, the file is named instead, so the
/// person can open it themselves within the minute its ticket is good for; and
/// where no file could be written, that is said. Neither sentence goes to the
/// log, which is told only that it happened (spec: EL-1).
pub fn open(app: &AppHandle, launcher: &Launcher) {
    let file = match launcher.open() {
        Ok(file) => file,
        Err(error) => {
            say(app, format!("The explorer could not be opened: {error}."));
            return;
        }
    };
    let opened = browser_target(&file).and_then(|target| {
        app.opener()
            .open_url(target.as_str(), None::<&str>)
            .map_err(anyhow::Error::new)
    });
    if opened.is_err() {
        // The opener's error is not rendered: it may quote what it was asked
        // to open, which is the file's path.
        tracing::warn!("the default browser could not be asked to open the explorer");
        say(
            app,
            format!(
                "The default browser could not be opened. Open {} in a browser within a \
                 minute, or choose Open the explorer from the tray icon again.",
                file.display()
            ),
        );
    }
}

/// Tells the person `message` in a warning dialog.
fn say(app: &AppHandle, message: String) {
    app.dialog()
        .message(message)
        .title("Coffret")
        .kind(MessageDialogKind::Warning)
        .show(|_| {});
}
