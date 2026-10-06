//! Opening the explorer in the default browser.

use std::net::SocketAddr;

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

/// Opens the explorer served at `address` in the default browser.
///
/// Where the browser could not be asked, the address is shown instead, so the
/// person can open it themselves: the explorer is up either way, and the
/// address is the one thing they are missing.
pub fn open(app: &AppHandle, address: SocketAddr) {
    let url = format!("http://{address}/");
    if let Err(error) = app.opener().open_url(&url, None::<&str>) {
        tracing::warn!(%error, "the default browser could not be asked to open the explorer");
        app.dialog()
            .message(format!(
                "The default browser could not be opened. The explorer is at {url}"
            ))
            .title("Coffret")
            .kind(MessageDialogKind::Warning)
            .show(|_| {});
    }
}
