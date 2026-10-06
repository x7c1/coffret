use std::net::SocketAddr;

use axum::http::header::CONTENT_TYPE;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// The answer to a request the server never took: a `502`, in a sentence
/// naming the address.
pub(super) fn not_answering(server: SocketAddr, cause: &reqwest::Error) -> Response {
    let chain = error_chain(cause);
    tracing::warn!(%server, cause = %chain, "a request could not be forwarded to the server");
    let sentence = if cause.is_connect() {
        format!("The coffret server at {server} is not answering. Start it, or check the address this explorer was started with.\n")
    } else {
        format!("The request could not be forwarded to the coffret server at {server}: {chain}.\n")
    };
    (
        StatusCode::BAD_GATEWAY,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        sentence,
    )
        .into_response()
}

/// An error with every cause under it, which is where a refused connection
/// says that it was refused.
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut said = error.to_string();
    let mut next = error.source();
    while let Some(cause) = next {
        said.push_str(": ");
        said.push_str(&cause.to_string());
        next = cause.source();
    }
    said
}
