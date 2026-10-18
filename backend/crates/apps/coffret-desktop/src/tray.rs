//! The tray icon: what is left of the app once a Library is open.
//!
//! Four items. *Unlock…* brings the Passphrase window forward once the Library
//! has locked itself, to unlock it in place without waiting for the explorer to
//! ask (spec: DK-1). *Open the explorer* opens it in the browser again through a
//! fresh launch ticket, for somebody who closed the tab or whose tab holds no
//! session — one whose session expired, or one opened from a bookmark
//! (spec: LA-18, LA-21). *Disconnect all browser tabs* ends every tab's session
//! at once (spec: LA-18). *Quit* ends the process, and the server with it —
//! which is the lock (spec: DK-1): what the process held is gone with it, every
//! browser session included.
//!
//! The icon is a one-colour picture of its own (`icons-tray/`, written by
//! `scripts/gen-icons.sh`) rather than the window icon, since a tray draws it
//! small: on macOS the black one as a template image, which the menu bar draws
//! black or white for its own appearance; on Linux the white one, since GNOME's
//! top bar is dark in both its light and its dark style and the appindicator
//! tray shows the picture as it is. Both are given the 64 px picture, which
//! either tray scales down to its own size. The development app's icon carries a "D"
//! badge, so that it is told apart from the installed app running beside it,
//! and on macOS its tooltip says so too (Tauri shows no tray tooltip on Linux).

use anyhow::Context;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{include_image, AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::app_build::AppBuild;
use crate::explorer;
use crate::unlock::{self, Desktop};

/// The tray icon's id.
const TRAY_ID: &str = "coffret";
/// The menu item that brings the Passphrase window forward to unlock.
const UNLOCK_ID: &str = "unlock";
/// The menu item that opens the explorer again.
const OPEN_ID: &str = "open";
/// The menu item that ends every browser tab's session.
const DISCONNECT_ID: &str = "disconnect";
/// The menu item that ends the app.
const QUIT_ID: &str = "quit";

/// The tray's tooltip for `build`.
fn tooltip(build: AppBuild) -> &'static str {
    match build {
        AppBuild::Installed => "Coffret",
        AppBuild::Development => "Coffret (dev)",
    }
}

/// The tray's icon for `build`: the black one, which macOS draws as a
/// template image.
#[cfg(target_os = "macos")]
fn icon(build: AppBuild) -> Image<'static> {
    match build {
        AppBuild::Installed => include_image!("icons-tray/tray@2x.png"),
        AppBuild::Development => include_image!("icons-tray/tray-dev@2x.png"),
    }
}

/// The tray's icon for `build`: the white one, for a dark top bar.
#[cfg(not(target_os = "macos"))]
fn icon(build: AppBuild) -> Image<'static> {
    match build {
        AppBuild::Installed => include_image!("icons-tray/tray-light@2x.png"),
        AppBuild::Development => include_image!("icons-tray/tray-dev-light@2x.png"),
    }
}

/// Puts the icon in the tray.
pub fn show(app: &AppHandle) -> anyhow::Result<()> {
    let unlock = MenuItem::with_id(app, UNLOCK_ID, "Unlock…", true, None::<&str>)
        .context("the tray's unlock item could not be made")?;
    let open = MenuItem::with_id(app, OPEN_ID, "Open the explorer", true, None::<&str>)
        .context("the tray's open item could not be made")?;
    let disconnect = MenuItem::with_id(
        app,
        DISCONNECT_ID,
        "Disconnect all browser tabs",
        true,
        None::<&str>,
    )
    .context("the tray's disconnect item could not be made")?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)
        .context("the tray's quit item could not be made")?;
    let menu = Menu::with_items(app, &[&unlock, &open, &disconnect, &quit])
        .context("the tray menu could not be made")?;
    let build = AppBuild::of(&app.config().identifier);
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon(build))
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip(tooltip(build))
        .menu(&menu)
        .on_menu_event(chosen)
        .build(app)
        .context("the tray icon could not be made")?;
    Ok(())
}

/// What a menu item does.
fn chosen(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        UNLOCK_ID => unlock_chosen(app),
        OPEN_ID => match app.state::<Desktop>().explorer() {
            Some(launcher) => explorer::open(app, launcher),
            // The icon is shown only once the explorer is served.
            None => tracing::warn!("the tray asked for an explorer that is not served"),
        },
        DISCONNECT_ID => match app.state::<Desktop>().explorer() {
            Some(launcher) => launcher.disconnect_all(),
            None => tracing::warn!("the tray asked to disconnect an explorer that is not served"),
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
    let Some(served) = app.state::<Desktop>().served().cloned() else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_development_app_gets_the_development_tooltip() {
        assert_eq!(tooltip(AppBuild::Development), "Coffret (dev)");
        assert_eq!(tooltip(AppBuild::Installed), "Coffret");
    }
}
