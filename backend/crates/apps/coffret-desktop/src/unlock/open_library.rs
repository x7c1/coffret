use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Context;
use coffret_device::{LibraryDir, Passphrase};
use coffret_explorer_host::credentials::{Credentials, SystemClock};
use coffret_explorer_host::Config;
use coffret_server::{Launch, ServerState, Serving, UnlockPrompt};
use tauri::{AppHandle, State};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use super::{ask_for_passphrase, refusal_evidence, refusal_line, Desktop, EnteredPassphrase};
use crate::{explorer, tray};

/// Opens the Library called `name` with `passphrase`, serves the explorer in
/// front of it, and opens the explorer in the default browser.
///
/// What is answered to the page on a refusal is one line — the Passphrase does
/// not open the Library, another server is serving it ([`refusal_line()`]) —
/// and the log is told the failure's log-safe rendering, never the chain the
/// server would have printed on a terminal ([`refusal_evidence()`]).
///
/// The window is left as it is: the page asks for it to be hidden once it has
/// heard this answer, so the answer is never torn down by the hiding
/// ([`super::dismiss_window()`]).
#[tauri::command]
pub async fn open_library(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    name: String,
    passphrase: EnteredPassphrase,
) -> Result<(), String> {
    let passphrase = passphrase.into_passphrase();

    let _turn = desktop.opening.lock().await;
    if let Some(address) = desktop.explorer() {
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
        idle_minutes: desktop
            .idle_minutes
            .get()
            .copied()
            .ok_or("The app did not finish starting.")?,
        unlock_prompt: Some(prompt),
    };
    let started = desktop
        .runtime
        .spawn(serve(app.clone(), launch, passphrase))
        .await;
    let (address, served) = match started {
        Ok(Ok(started)) => started,
        Ok(Err(refused)) => {
            tracing::warn!(
                error = %refusal_evidence(&refused),
                "a Library could not be opened from the Passphrase window"
            );
            return Err(refusal_line(&refused));
        }
        Err(panicked) => {
            tracing::error!(error = %panicked, "opening a Library stopped without an answer");
            return Err("Opening the Library stopped without an answer.".to_owned());
        }
    };
    // Nobody else sets them: the lock above is held by whoever reaches here.
    let _ = desktop.served.set(served);
    let _ = desktop.explorer.set(address);
    desktop.runtime.spawn(answer_prompts(app.clone(), asked));

    if let Err(error) = tray::show(&app) {
        // The explorer is open regardless, and a second launch reaches it
        // again; what is lost is the tray's way to quit, which closing the
        // terminal or logging out still is, and its way to unlock, which the
        // explorer's own *unlock* and a second launch still are.
        tracing::error!(error = %format!("{error:#}"), "the tray icon could not be shown");
    }
    // The explorer is a process of its own and does not touch the window, so it
    // may open before the page hears the answer.
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
/// Run on the app's own runtime, because what it binds is served there.
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

    // Loopback, for the reason the server's own socket is (spec: LA-1), and
    // any free port: nothing but this app ever needs to know the number, and
    // it is the one that opens the browser on it. Bound before the host is
    // built, because the host answers only the authority it was built around,
    // and that has to be the one actually bound (spec: LA-13).
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .context("nothing could listen for the explorer on 127.0.0.1")?;
    let address = listener
        .local_addr()
        .context("the address the explorer is served at could not be read")?;
    // The handle that issues launch tickets stays with this app and is never
    // handed to the host's routes (spec: LA-17). Nothing issues one yet, so
    // the explorer's page loads but is admitted to no Library data.
    let credentials = Credentials::new(Arc::new(SystemClock))
        .context("the explorer's credentials could not be drawn")?;
    let host = coffret_explorer_host::router(
        &listener,
        Config::for_library(server, &library, Arc::clone(&served)),
        credentials.gate(),
    )?;
    eprintln!("The explorer is at http://{address}/.");

    tokio::spawn(async move {
        if let Err(error) = serving.serve().await {
            stop(&app, &error);
        }
    });
    // It answers for as long as the app runs, and tries again after a failure
    // to accept rather than stopping.
    tokio::spawn(coffret_explorer_host::serve(listener, host));
    Ok((address, served))
}

/// Ends the app because the server it serves stopped.
///
/// An app whose server is gone has nothing to show, and the tray would be
/// offering an explorer that is not there.
fn stop(app: &AppHandle, error: &anyhow::Error) {
    eprintln!("the server stopped: {error:#}");
    tracing::error!("the server stopped; coffret-desktop is ending");
    app.exit(1);
}
