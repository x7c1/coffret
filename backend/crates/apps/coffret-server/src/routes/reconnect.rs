use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::api_error::ApiError;
use crate::reconnect;
use crate::state::ServerState;

/// What a reconnect that started answers with: the page to open, and what to
/// tell the person.
#[derive(Serialize)]
pub struct ReconnectingDto {
    /// The consent page, for the explorer to open in a tab of its own.
    ///
    /// The one thing the flow hands a page. It names this server's OAuth
    /// client, the permission asked for (spec: SA-3), the loopback port the
    /// answer comes back to and the flow's `state` (spec: SA-2) — all of which
    /// the browser is sent anyway — and no token, no verifier and no key.
    url: String,
    /// One sentence a person could read.
    message: &'static str,
}

/// `POST /api/reconnect`
///
/// Starts asking for Google Drive's permission again, inside the server, with
/// the keys the open Library already holds — so no Passphrase is asked for,
/// and none crosses this boundary. Keyed like every route that needs the
/// Library (spec: LA-2), and refused as locked while it is (spec: DK-2).
///
/// It answers `202` with the consent page's URL once the flow has one, and the
/// flow goes on waiting for the browser on a task of the server's own: what it
/// comes to is the work answer's `reconnect`. A second press while one waits
/// answers with the same page.
pub async fn reconnect(
    State(state): State<Arc<ServerState>>,
) -> Result<(StatusCode, Json<ReconnectingDto>), ApiError> {
    let url = reconnect::start(&state).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(ReconnectingDto {
            url,
            message: "answer Google's consent page in the tab that opened, and this device goes \
                      on with the renewed permission",
        }),
    ))
}
