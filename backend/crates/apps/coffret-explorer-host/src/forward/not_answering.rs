use std::net::SocketAddr;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use super::failure_kind;
use crate::refusal::Refusal;

/// The `error` word for a request that was sent and broke off, which revoked
/// nothing: the server may still be the one the sessions were issued under.
const BROKE_OFF: &str = "bad_gateway";

/// The shape every refusal of the host's has, here with a sentence that names
/// the address.
#[derive(Serialize)]
struct Body {
    error: &'static str,
    message: String,
}

/// The answer to a request the server never took: a `502`, in a sentence
/// naming the address.
///
/// A server that refused the connection is one that stopped, and every session
/// went with it (spec: LA-15), so the answer's `error` is the word that says
/// so, for the page to let its token go. Any other failure revoked nothing and
/// has a word of its own.
///
/// The event and the sentence are built apart (spec: EL-1). Neither is a
/// rendering of `cause`, whose text names the request's URL and with it any
/// Entry Path in its query: both say what kind of failure it was, and the
/// sentence adds what the person can do about it.
pub(super) fn not_answering(server: SocketAddr, cause: &reqwest::Error) -> Response {
    let kind = failure_kind(cause);
    tracing::warn!(
        %server,
        operation = "send_request",
        kind,
        "a request could not be forwarded to the server",
    );
    answer(server, cause.is_connect(), kind)
}

/// The `502` for a request to `server` that met a failure of `kind`, which was
/// the connection being `refused` or something after it.
pub(crate) fn answer(server: SocketAddr, refused: bool, kind: &str) -> Response {
    let body = if refused {
        Body {
            error: Refusal::ServerUnavailable.word(),
            message: format!(
                "The coffret server at {server} is not answering. Start it, then open the \
                 explorer again."
            ),
        }
    } else {
        Body {
            error: BROKE_OFF,
            message: format!(
                "The request could not be forwarded to the coffret server at {server} \
                 (failure kind: {kind})."
            ),
        }
    };
    (StatusCode::BAD_GATEWAY, Json(body)).into_response()
}
