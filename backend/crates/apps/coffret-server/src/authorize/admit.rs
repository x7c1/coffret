use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::Admission;

/// Refuses every request that is not this device's own, then runs the route.
pub(crate) async fn admit(
    State(admission): State<Arc<Admission>>,
    request: Request,
    next: Next,
) -> Response {
    match admission.verdict(request.headers()) {
        Ok(()) => next.run(request).await,
        Err(refused) => refused.recorded().into_response(),
    }
}
