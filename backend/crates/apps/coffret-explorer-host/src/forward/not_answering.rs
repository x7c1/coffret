use std::net::SocketAddr;

use axum::http::header::CONTENT_TYPE;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::failure_kind;

/// The answer to a request the server never took: a `502`, in a sentence
/// naming the address.
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
    let sentence = if cause.is_connect() {
        format!("The coffret server at {server} is not answering. Start it, or check the address this explorer was started with.\n")
    } else {
        format!("The request could not be forwarded to the coffret server at {server} (failure kind: {kind}).\n")
    };
    (
        StatusCode::BAD_GATEWAY,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        sentence,
    )
        .into_response()
}
