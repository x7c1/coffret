use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Context;
use coffret_device::{LibraryDir, Passphrase};
use coffret_server::{Launch, ServerState, Serving, UnlockPrompt};
use tauri::{AppHandle, State};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use super::{ask_for_passphrase, hide_window, Shell};
use crate::{explorer, tray};

/// Opens the Library called `name` with `passphrase`, serves the explorer in
/// front of it, and opens the explorer in the default browser.
///
/// What is answered to the page on a refusal is the whole chain the server
/// would have printed — the Library is not on this device, the Passphrase does
/// not open it, another server is serving it — so the person reads the same
/// sentence here as on a terminal.
#[tauri::command]
pub async fn open_library(
    app: AppHandle,
    shell: State<'_, Shell>,
    name: String,
    passphrase: String,
) -> Result<(), String> {
    // Before anything else, so that what the page sent is wiped whatever
    // happens next: the string's own allocation becomes the Passphrase's
    // (spec: DK-7).
    let passphrase = Passphrase::from_bytes(passphrase.into_bytes());

    let _turn = shell.opening.lock().await;
    if let Some(address) = shell.explorer() {
        // A press that waited behind the one that opened the Library.
        explorer::open(&app, address);
        return Ok(());
    }

    // The server's way of asking for the Passphrase again once it has locked:
    // the explorer's *unlock* reaches this window through it (spec: DK-1).
    let (prompt, asked) = UnlockPrompt::channel();
    let launch = Launch {
        library: name,
        port: 0,
        idle_minutes: shell
            .idle_minutes
            .get()
            .copied()
            .ok_or("the shell did not finish starting")?,
        unlock_prompt: Some(prompt),
    };
    let started = shell
        .runtime
        .spawn(serve(app.clone(), launch, passphrase))
        .await;
    let (address, served) = match started {
        Ok(Ok(started)) => started,
        Ok(Err(refused)) => {
            tracing::info!("a Library could not be opened from the Passphrase window");
            return Err(format!("{refused:#}"));
        }
        Err(panicked) => {
            tracing::error!(error = %panicked, "opening a Library stopped without an answer");
            return Err("opening the Library stopped without an answer".to_owned());
        }
    };
    // Nobody else sets them: the lock above is held by whoever reaches here.
    let _ = shell.served.set(served);
    let _ = shell.explorer.set(address);
    shell.runtime.spawn(answer_prompts(app.clone(), asked));

    if let Err(error) = tray::show(&app) {
        // The explorer is open regardless, and a second launch reaches it
        // again; what is lost is the tray's way to quit, which closing the
        // terminal or logging out still is, and its way to unlock, which the
        // explorer's own *unlock* and a second launch still are.
        tracing::error!(error = %format!("{error:#}"), "the tray icon could not be shown");
    }
    hide_window(&app);
    explorer::open(&app, address);
    Ok(())
}

/// Puts the Passphrase window in front whenever the server asks for it, for as
/// long as the server can ask.
///
/// The server only wakes this; it is the window that takes the Passphrase, so
/// nothing secret ever passes through here (spec: DK-10).
async fn answer_prompts(app: AppHandle, mut asked: mpsc::Receiver<()>) {
    while asked.recv().await.is_some() {
        let on_main = app.clone();
        // On the main thread, where the window's own events are handled, so
        // that reading what the window shows and changing it are one step.
        if let Err(error) = app.run_on_main_thread(move || ask_for_passphrase(&on_main)) {
            tracing::warn!(%error, "the Passphrase window could not be asked for");
        }
    }
}

/// Opens the Library, binds the explorer's host in front of it, and starts
/// both answering. Returns where the explorer is, and what the server serves.
///
/// Run on the shell's own runtime, because what it binds is served there.
async fn serve(
    app: AppHandle,
    launch: Launch,
    passphrase: Passphrase,
) -> anyhow::Result<(SocketAddr, Arc<ServerState>)> {
    let idle_minutes = launch.idle_minutes;
    let serving: Serving = launch.open(move || Ok(passphrase)).await?;
    let server = serving.address();
    let served = Arc::clone(serving.state());
    let library = LibraryDir::resolve(&served.name)?;
    eprintln!(
        "Serving the Library {:?} at http://{server}.",
        served.name.as_str()
    );
    eprintln!(
        "It locks itself after {idle_minutes} minute(s) in which nothing is read from or \
         written to the Library; unlock it again from the explorer or from the tray icon."
    );

    let host = coffret_explorer_host::router(coffret_explorer_host::Config::for_library(
        server, &library,
    ))?;
    // Loopback, for the reason the server's own socket is (spec: LA-1), and
    // any free port: nothing but this shell ever needs to know the number, and
    // it is the one that opens the browser on it.
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .context("nothing could listen for the explorer on 127.0.0.1")?;
    let address = listener
        .local_addr()
        .context("the address the explorer is served at could not be read")?;
    eprintln!("The explorer is at http://{address}/.");

    let stopped = app.clone();
    tokio::spawn(async move {
        if let Err(error) = serving.serve().await {
            stop(&stopped, "the server", &error);
        }
    });
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, host).await {
            stop(&app, "the explorer's host", &anyhow::Error::new(error));
        }
    });
    Ok((address, served))
}

/// Ends the shell because one of the two things it serves stopped.
///
/// A shell whose server is gone has nothing to show, and one whose host is gone
/// has nothing anybody can reach; either way the tray would be offering an
/// explorer that is not there.
fn stop(app: &AppHandle, what: &str, error: &anyhow::Error) {
    eprintln!("{what} stopped: {error:#}");
    tracing::error!("{what} stopped; coffret-desktop is ending");
    app.exit(1);
}
