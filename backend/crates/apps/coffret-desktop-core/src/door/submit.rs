use std::sync::Arc;

use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use super::fences::presented_token;
use super::windows::AfterRefusal;
use super::{Door, Refusal, Submission};
use crate::attempt::{attempt, AttemptRefused};

/// `POST /submit`: what a window sends when its form is submitted — the
/// Passphrase, and in the open mode the Library to open (spec: LA-22).
///
/// What it passes, in this order: the fixed `Host` (over every path, before
/// this); the door's own `Origin` and `Sec-Fetch-Site: same-origin`; a token
/// the door holds; a JSON body within its bound. Every refusal up to the token
/// is answered without the body being read, and counts against no window. From
/// the token on, a refusal counts against that window's token, and the fifth
/// ends it.
///
/// Then the attempt itself, under the core's opening turn, so that of two
/// submissions at once the second sees what the first did. The token is looked
/// at again once the turn is taken: a submission that waited behind its own
/// window's last try finds the token gone. Success ends the token. A refusal
/// is answered with one sentence that carries no path, and the log is told its
/// log-safe rendering (spec: EL-1, LA-24).
pub(super) async fn submit(State(door): State<Arc<Door>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if let Err(fence) = door.admits_submission(&parts.headers) {
        return Refusal::OriginRefused.answer(fence, false);
    }
    let Some(token) = presented_token(&parts.headers) else {
        return Refusal::AuthenticationRequired.answer("token", true);
    };
    if door.shared.windows.admit(token).is_none() {
        return Refusal::AuthenticationRequired.answer("token", true);
    }
    let submission = match Submission::read(&parts.headers, body).await {
        Ok(submission) => submission,
        Err((refusal, reason)) => {
            let ended = door.shared.windows.refused(token) != AfterRefusal::Stands;
            return refusal.answer(reason, ended);
        }
    };

    let _turn = door.shared.opening.lock().await;
    let Some(mode) = door.shared.windows.admit(token) else {
        return Refusal::AuthenticationRequired.answer("token", true);
    };
    let Submission {
        library,
        passphrase,
    } = submission;
    // Spawned and awaited, so that an attempt that panics is answered as one
    // rather than taking the connection down unanswered.
    let attempted = tokio::spawn(attempt(
        Arc::clone(&door.shared),
        mode,
        library,
        passphrase.into_passphrase(),
    ))
    .await
    .unwrap_or_else(|stopped| {
        // Whether it panicked and nothing more: a panic's message is whatever
        // the code that panicked wrote, a path among it (spec: EL-1).
        tracing::error!(
            panicked = stopped.is_panic(),
            "a Passphrase window's attempt stopped without an answer",
        );
        Err(AttemptRefused::Panicked)
    });
    match attempted {
        Ok(_) => {
            door.shared.windows.succeeded(token);
            Json(json!({ "done": true })).into_response()
        }
        Err(refused) => {
            tracing::warn!(
                error = %refused.evidence(),
                "a Library could not be opened or unlocked from a Passphrase window",
            );
            let ended = door.shared.windows.refused(token) != AfterRefusal::Stands;
            Refusal::Attempt(refused).answer("attempt", ended)
        }
    }
}
