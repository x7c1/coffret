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
        // The address this server bound, for the one refusal that names it: it
        // is already extracted here, and the fences themselves carry nothing
        // about the server they are one of.
        Err(refused) => refused.recorded(&admission.authority).into_response(),
    }
}
