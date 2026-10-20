use std::net::Ipv4Addr;
use std::sync::Arc;

use anyhow::Context;
use coffret_device::{LibraryDir, Passphrase};
use coffret_explorer_host::credentials::{Credentials, SystemClock};
use coffret_explorer_host::Config;
use coffret_server::{Launch, Serving, UnlockPrompt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::opened::Opened;
use crate::shared::Shared;
use crate::{refusal_evidence, Event};

/// Opens the Library called `name` with `passphrase`, serves the explorer in
/// front of it, and records both in `shared`: what the Tauri app's
/// `open_library` does, minus the launch files, since the shell is handed
/// tickets directly ([`Desktop::issue_ticket`](crate::Desktop::issue_ticket)).
///
/// The caller holds `shared.opening` and has seen nothing open yet. Run on the
/// core's runtime, because what it binds is served there, and so are the
/// server's tasks.
pub(crate) async fn open_library(
    shared: &Arc<Shared>,
    name: String,
    passphrase: Passphrase,
) -> anyhow::Result<()> {
    // The server's way of asking for the Passphrase again once it has locked:
    // the explorer's *unlock* reaches the shell through it (spec: DK-1).
    let (prompt, asked) = UnlockPrompt::channel();
    let launch = Launch {
        library: name,
        port: 0,
        idle_minutes: shared.idle_minutes,
        unlock_prompt: Some(prompt),
    };
    let serving: Serving = launch.open(move || Ok(passphrase)).await?;
    let server = serving.address();
    let state = Arc::clone(serving.state());
    let library = LibraryDir::resolve(&state.name)?;

    // Loopback, for the reason the server's own socket is (spec: LA-1), and
    // any free port. Bound before the host is built, because the host answers
    // only the authority it was built around, and that has to be the one
    // actually bound (spec: LA-13).
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .context("nothing could listen for the explorer on 127.0.0.1")?;
    let explorer = listener
        .local_addr()
        .context("the address the explorer is served at could not be read")?;
    // The handle that issues launch tickets stays with the core, and is never
    // handed to the host's routes (spec: LA-17).
    let credentials = Credentials::new(Arc::new(SystemClock))
        .context("the explorer's credentials could not be drawn")?;
    let mut config = Config::for_library(server, &library, Arc::clone(&state), shared.kind);
    config.page = shared.page.clone();
    let host = coffret_explorer_host::router(&listener, config, credentials.gate())?;

    // Nobody else sets it: the caller holds the opening turn and saw nothing
    // open.
    let _ = shared.opened.set(Opened {
        state,
        credentials,
        explorer,
    });

    let watched = Arc::clone(shared);
    tokio::spawn(async move {
        let stopped = serving.serve().await;
        let evidence = match &stopped {
            Err(error) => refusal_evidence(error),
            Ok(()) => "Finished".to_owned(),
        };
        tracing::error!(
            error = %evidence,
            "the server stopped; every session, ticket and door token is discarded",
        );
        watched.stop();
    });
    // It answers for as long as the process runs, and tries again after a
    // failure to accept rather than stopping.
    tokio::spawn(coffret_explorer_host::serve(listener, host));
    tokio::spawn(forward_prompts(Arc::clone(shared), asked));
    Ok(())
}

/// Tells the shell whenever the server asks for the Passphrase, for as long as
/// the server can ask.
///
/// The server only wakes this; it is the door's window that takes the
/// Passphrase, so nothing secret ever passes through here (spec: DK-10).
async fn forward_prompts(shared: Arc<Shared>, mut asked: mpsc::Receiver<()>) {
    while asked.recv().await.is_some() {
        shared.events.send(Event::UnlockRequested);
    }
}
