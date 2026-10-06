//! The tray icon: what is left of the shell once a Library is open.
//!
//! Two items. *Open the explorer* opens it in the browser again, for somebody
//! who closed the tab. *Quit* ends the process, and the server with it — which
//! is the lock (spec: DK-1): what the process held is gone with it.

use anyhow::Context;
use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::explorer;
use crate::unlock::Shell;

/// The tray icon's id.
const TRAY_ID: &str = "coffret";
/// The menu item that opens the explorer again.
const OPEN_ID: &str = "open";
/// The menu item that ends the shell.
const QUIT_ID: &str = "quit";

/// Puts the icon in the tray.
pub fn show(app: &AppHandle) -> anyhow::Result<()> {
    let open = MenuItem::with_id(app, OPEN_ID, "Open the explorer", true, None::<&str>)
        .context("the tray's open item could not be made")?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)
        .context("the tray's quit item could not be made")?;
    let menu = Menu::with_items(app, &[&open, &quit]).context("the tray menu could not be made")?;
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
