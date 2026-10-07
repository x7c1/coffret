//! The tray icon: what is left of the shell once a Library is open.
//!
//! Three items. *Unlock…* brings the Passphrase window forward once the Library
//! has locked itself, to unlock it in place without waiting for the explorer to
//! ask (spec: DK-1). *Open the explorer* opens it in the browser again, for
//! somebody who closed the tab. *Quit* ends the process, and the server with it
//! — which is the lock (spec: DK-1): what the process held is gone with it.

use anyhow::Context;
use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::explorer;
use crate::unlock::{self, Shell};

/// The tray icon's id.
const TRAY_ID: &str = "coffret";
/// The menu item that brings the Passphrase window forward to unlock.
const UNLOCK_ID: &str = "unlock";
/// The menu item that opens the explorer again.
const OPEN_ID: &str = "open";
/// The menu item that ends the shell.
const QUIT_ID: &str = "quit";

/// Puts the icon in the tray.
pub fn show(app: &AppHandle) -> anyhow::Result<()> {
    let unlock = MenuItem::with_id(app, UNLOCK_ID, "Unlock…", true, None::<&str>)
        .context("the tray's unlock item could not be made")?;
    let open = MenuItem::with_id(app, OPEN_ID, "Open the explorer", true, None::<&str>)
        .context("the tray's open item could not be made")?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)
        .context("the tray's quit item could not be made")?;
    let menu = Menu::with_items(app, &[&unlock, &open, &quit])
        .context("the tray menu could not be made")?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Coffret")
        .menu(&menu)
        .on_menu_event(chosen);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .build(app)
        .context("the tray icon could not be made")?;
    Ok(())
}

/// What a menu item does.
fn chosen(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        UNLOCK_ID => unlock_chosen(app),
        OPEN_ID => match app.state::<Shell>().explorer() {
            Some(address) => explorer::open(app, address),
            // The icon is shown only once the explorer is served.
            None => tracing::warn!("the tray asked for an explorer that is not served"),
        },
        QUIT_ID => app.exit(0),
        other => tracing::warn!(
            item = other,
            "the tray menu reported an item it does not have"
        ),
    }
}

/// What *Unlock…* does: the Passphrase window, where the Library has locked,
/// and a word saying it has not where it is still open.
///
/// Always offered rather than enabled and disabled with the lock, because the
/// lock happens on the server's own clock and a menu kept in step with it would
/// be a second thing watching that clock.
fn unlock_chosen(app: &AppHandle) {
    let Some(served) = app.state::<Shell>().served().cloned() else {
        // The icon is shown only once a Library is served.
        tracing::warn!("the tray asked to unlock a Library that is not served");
        return;
    };
    if served.holds_library() {
        app.dialog()
            .message("The Library is unlocked.")
            .title("Coffret")
            .kind(MessageDialogKind::Info)
            .show(|_| {});
        return;
    }
    unlock::ask_for_passphrase(app);
}
