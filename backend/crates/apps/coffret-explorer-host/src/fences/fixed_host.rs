use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;

use crate::host::Host;
use crate::refusal::Refusal;

/// How many bytes of headers any request may bring (spec: LA-19).
pub(crate) const MAX_HEADER_BYTES: usize = 8 * 1024;

/// Refuses a request whose headers are past their bound, or whose `Host` is not
/// the authority the listener bound (spec: LA-13, LA-19).
///
/// Over every path, the page's included: a page served under somebody else's
/// name is that name's page as far as the browser is concerned.
pub(crate) async fn fixed_host(
    State(host): State<Arc<Host>>,
    request: Request,
    next: Next,
) -> Response {
    if header_bytes(request.headers()) > MAX_HEADER_BYTES {
        return Refusal::HeadersTooLarge.answer("header_bytes", request.headers());
    }
    if !host.authority.is_named_by(request.headers(), request.uri()) {
        return Refusal::OriginRefused.answer("host", request.headers());
    }
    next.run(request).await
}

/// The headers' size as they were written: each name, `: `, value and line end.
fn header_bytes(headers: &HeaderMap) -> usize {
    headers
        .iter()
        .map(|(name, value)| name.as_str().len() + value.len() + 4)
        .sum()
}
