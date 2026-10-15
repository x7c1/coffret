use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::host::Host;
use crate::refusal::Refusal;

/// Refuses a request a browser says another site's page made (spec: LA-14,
/// LA-16).
pub(crate) async fn same_origin(
    State(host): State<Arc<Host>>,
    request: Request,
    next: Next,
) -> Response {
    if let Err(fence) = host.authority.admits_origin(request.headers()) {
        return Refusal::OriginRefused.answer(fence, request.headers());
    }
    next.run(request).await
}
