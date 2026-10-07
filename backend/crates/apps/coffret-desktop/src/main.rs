//! Coffret's desktop shell.
//!
//! A launcher, not a window onto the explorer. It starts the server inside its
//! own process, serves the built explorer in front of it on a loopback port of
//! its own (`coffret-explorer-host`), and opens that address in the system's
//! default browser. The explorer's reader and its drag-and-drop want a
//! Chromium-class engine, which the webview a Linux desktop offers is not, so
//! the explorer is never shown in a webview here.
//!
//! The one window the shell has is its own: a small page asking which Library
//! to open and its Passphrase ([`unlock`]). That is where the Passphrase is
//! typed, so it never passes through the explorer's page (spec: DK-10), and the
//! key the server admits its callers by stays between the server and the host
//! as it does everywhere else (spec: LA-3, LA-6).
//!
//! Once a Library is open, the window is hidden and a tray icon ([`tray`]) is
//! what is left of the shell. The window comes back once the Library has
//! locked itself after the idle interval: the explorer's *unlock* asks the
//! server, the server wakes the shell, and the window takes the Passphrase
//! again and unlocks the Library in place (spec: DK-1) — as does the tray's
//! *Unlock…*. The Passphrase still never passes through the explorer's page.
//!
//! Where Libraries are is the binaries' own answer and not this shell's: the
//! default state directory for the installed app, or whatever
//! `COFFRET_STATE_DIR` names for a process started with it set.

mod explorer;
mod settings;
mod startup;
mod tray;
mod unlock;

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tokio::runtime::Runtime;

use crate::unlock::Shell;

fn main() {
    let context = tauri::generate_context!();
    #[cfg(target_os = "linux")]
    set_window_app_id(&context.config().identifier);

    // The runtime the server and the explorer's host run on, for the whole of
    // the process: the server holds the Library open for as long as it is
    // served, and nothing here is meant to outlive or underlive it.
    let runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("coffret-desktop could not start its async runtime: {error}");
            std::process::exit(1);
        }
    };

    let built = tauri::Builder::default()
        // Registered first, so a second launch is caught before anything else
        // runs: the plugin hands its arguments to the running copy and exits,
        // and its `setup` below — which would open a second window onto the
        // same Libraries — never runs.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            launched_again(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Shell::new(runtime))
        .invoke_handler(tauri::generate_handler![
            unlock::open_library,
            unlock::unlock_library
        ])
        // Tauri panics on an error returned from here, so every failure is
        // reported (and exits 1) inside the hook instead.
        .setup(|app| {
            if let Err(error) = startup::start(app) {
                report_startup_failure(app.handle(), &error);
            }
            Ok(())
        })
        .build(context);

    match built {
        // Closing the Passphrase window while nothing is served is the last
        // window closing, which ends the event loop, and the process with it.
        // Once a Library is open the window is hidden rather than closed, and
        // the tray's *Quit* is what ends it.
        Ok(app) => app.run(|_, _| {}),
        Err(error) => {
            eprintln!("coffret-desktop could not start: {error}");
            std::process::exit(1);
        }
    }
}

/// Make the identifier the window's app ID, so the desktop tells this build's
/// window apart from another build's: the development shell and the installed
/// one are both an executable named `coffret-desktop`.
///
/// GTK 3 takes a Wayland window's app ID (and the X11 `WM_CLASS`) from the
/// program name, not from the GTK application ID that `enableGTKAppId` sets, so
/// the program name has to be set too, before GTK starts. GNOME matches this ID
/// to the desktop entry's `StartupWMClass`.
#[cfg(target_os = "linux")]
fn set_window_app_id(identifier: &str) {
    glib::set_prgname(Some(identifier));
}

/// What a second launch does in the copy that is already running.
///
/// The explorer again, where a Library is already open: that is what somebody
/// launching the app is asking for — and the Passphrase window in front of it
/// where that Library has locked, since the explorer has nothing to show until
/// it is unlocked. Otherwise the Passphrase window, brought forward, since
/// that is where they are in the middle of opening one.
fn launched_again(app: &AppHandle) {
    let shell = app.state::<Shell>();
    if let Some(address) = shell.explorer() {
        explorer::open(app, address);
        unlock::ask_for_passphrase(app);
        return;
    }
    unlock::bring_forward(app);
}

/// Show a startup failure in a message dialog and exit 1 when it is
/// dismissed.
///
/// Every failure that reaches here is one a person has to act on — the log
/// could not be started, the Libraries could not be listed, a setting is not a
/// number — and an installed app has no terminal to say it on. It is printed as
/// well, for a shell started from one.
fn report_startup_failure(handle: &AppHandle, error: &anyhow::Error) {
    let message = format!("{error:#}");
    eprintln!("{message}");
    tracing::error!("coffret-desktop could not start");
    let exit_handle = handle.clone();
    handle
        .dialog()
        .message(message)
        .title("Coffret could not start")
        .kind(MessageDialogKind::Error)
        .show(move |_| exit_handle.exit(1));
}
