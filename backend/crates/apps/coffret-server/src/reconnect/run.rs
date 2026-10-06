use std::sync::Arc;

use tokio::sync::oneshot;
use tracing::info;

use super::{ConsentFlow, Reconnect};
use crate::api_error::ApiError;
use crate::lock::KeyHandle;
use crate::refresh::catch_up_after_reconnect;
use crate::state::ServerState;

/// Starts a consent flow for the account the open Library's grant belongs to,
/// or finds the one already waiting, and answers with the page to open.
///
/// The Library is asked for first, so a locked server refuses at once with the
/// refusal every keyed route gives (spec: DK-2); and the handle is held for the
/// whole of the flow, which is work that needs the Library's keys like any
/// other and so counts as somebody being here until it ends (spec: DK-4).
///
/// The flow runs on a task this server owns rather than inside the request:
/// it waits for a person, for up to the five minutes the loopback listener
/// gives them, and the request is over the moment the URL exists. The URL is
/// recorded by the flow itself as it is shown, before the flow waits, so
/// whatever the flow comes to afterwards is recorded after it and never
/// overwritten by the request answering.
pub(crate) async fn start(state: &Arc<ServerState>) -> Result<String, ApiError> {
    let library = state.unlocked()?;
    let _turn = state.reconnects.turn().await;
    if let Some(url) = state.reconnects.waiting_on() {
        return Ok(url);
    }

    let (shown, url) = oneshot::channel();
    let recording = Arc::clone(state);
    let show = Box::new(move |url: &str| {
        recording.reconnects.stands(Reconnect::Waiting {
            url: url.to_owned(),
        });
        // The request may have gone away, which leaves the flow to run on and
        // the page to learn of it from the work answer.
        let _ = shown.send(url.to_owned());
    });
    let Some(flow) = state.consent.ask(&library, show) else {
        return Err(ApiError::nothing_to_reconnect());
    };
    tokio::spawn(follow(Arc::clone(state), library, flow));

    // A flow that ended before it showed anything — a listener that would not
    // bind — has already said so in the log and on record.
    url.await.map_err(|_| ApiError::consent_not_started())
}

/// Waits the flow out, and records what it came to.
async fn follow(state: Arc<ServerState>, library: KeyHandle, flow: ConsentFlow) {
    let mut ending = Ending {
        state: &state,
        ended: false,
    };
    let outcome = flow.await;
    // Let go of the handle before the catch-up, which takes one of its own, so a
    // flow that has ended no longer counts as somebody being here (spec: DK-4).
    drop(library);
    match outcome {
        Ok(()) => {
            info!(
                operation = "reconnect",
                "the grant was renewed; catching the catalog up"
            );
            // How the catalog stands is the catch-up's own to record, and a
            // refusal it meets goes to the log here: the grant was renewed
            // whatever Storage answers next, and the standing says the rest.
            if let Err(refusal) = catch_up_after_reconnect(&state).await {
                refusal.record("reconnect");
            }
            ending.with(Reconnect::Renewed);
        }
        Err(unrenewed) => ending.with(unrenewed.into()),
    }
}

/// A flow being followed, and what the reconnect is left saying however the
/// following ends.
///
/// A guard for the reason the catch-up's `Replaying` is one: a task that
/// panics reaches no line of its own, and a reconnect left saying it is waiting
/// would have the page wait on it, and every later press answered with a page
/// nobody is listening behind.
struct Ending<'a> {
    state: &'a ServerState,
    ended: bool,
}

impl Ending<'_> {
    fn with(&mut self, reconnect: Reconnect) {
        self.state.reconnects.stands(reconnect);
        self.ended = true;
    }
}

impl Drop for Ending<'_> {
    fn drop(&mut self) {
        if !self.ended {
            self.state.reconnects.stands(Reconnect::Failed);
        }
    }
}
