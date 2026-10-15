use axum::extract::Request;
use axum::http::header::CONTENT_LENGTH;
use axum::middleware::Next;
use axum::response::Response;

use super::MAX_BODY_BYTES;
use crate::refusal::Refusal;

/// Refuses a request to `/auth` that declares more body than it may bring,
/// before a byte of it is read (spec: LA-19).
///
/// A body that declares nothing and runs on is stopped where it passes the
/// bound by the route that reads it; the routes that read none never read it.
pub(crate) async fn bounded_body(request: Request, next: Next) -> Response {
    let declared = request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|length| length.to_str().ok())
        .map(|length| length.trim().parse::<u64>());
    match declared {
        Some(Ok(length)) if length <= MAX_BODY_BYTES as u64 => next.run(request).await,
        None => next.run(request).await,
        Some(_) => Refusal::BodyTooLarge.answer("body_bytes", request.headers()),
    }
}
