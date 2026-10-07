use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::api_error::{ApiError, WayBack};
use crate::lock::Asked;
use crate::state::ServerState;

/// What an unlock that was asked for answers with: which state the Library is
/// in, and what to tell the person.
#[derive(Serialize)]
pub struct UnlockingDto {
    /// `unlocked` where it already was, and `locked` where the app has been
    /// asked for the Passphrase and has not been given it yet — the words the
    /// work answer's `library` uses.
    library: &'static str,
    /// One sentence a person could read.
    message: &'static str,
}

/// `POST /api/unlock`
///
/// Asks the process this server runs in to take the Passphrase again (spec:
/// DK-1). The Passphrase itself never crosses this route — nothing is read from
/// the request at all, for the reason [`UnlockPrompt`](crate::UnlockPrompt)
/// gives. What it does is wake the desktop app's own window, which takes the
/// Passphrase where no page can see it.
///
/// Admitted like every route (spec: LA-2), and answered while locked, which is
/// the only time it has anything to do. It takes no key and so is no activity
/// (spec: DK-4): asking for the unlock is not wanting the Library yet. Three
/// answers:
///
/// - the Library is open already: `200`, saying so
/// - it is locked and the app has a window to ask in: `202`, the window woken,
///   saying the app is asking for the Passphrase; the work answer's `library`
///   turns to `unlocked` once it has been given
/// - it is locked and nothing can be asked — a server started from the command
///   line: the locked refusal, whose sentence says to start the server again,
///   which is the one true thing to tell somebody there
pub async fn unlock(
    State(state): State<Arc<ServerState>>,
) -> Result<(StatusCode, Json<UnlockingDto>), ApiError> {
    if state.holds_library() {
        return Ok((
            StatusCode::OK,
            Json(UnlockingDto {
                library: "unlocked",
                message: "the Library is already unlocked",
            }),
        ));
    }
    match state.ask_for_passphrase() {
        Asked::Woken => Ok((
            StatusCode::ACCEPTED,
            Json(UnlockingDto {
                library: "locked",
                message: "the Coffret app is asking for the Passphrase in its own window",
            }),
        )),
        // A prompt whose window end has gone is no window to send anybody to,
        // and what is still true of that process is what is true of every
        // server: starting it again takes the Passphrase.
        Asked::Unheard => {
            if state.way_back() == WayBack::InTheApp {
                tracing::warn!(
                    operation = "unlock",
                    "the app's Passphrase window could not be asked for; nothing is listening",
                );
            }
            Err(ApiError::locked(WayBack::ByStartingAgain))
        }
    }
}
