use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;

use super::bearer;
use crate::host::Host;
use crate::refusal::Refusal;

/// The one request a locked Library still takes beside `/auth/session`: the
/// page asking the app to put its Passphrase window in front (spec: LA-14).
const UNLOCK: &str = "/api/unlock";

/// Admits a request under `/api` by its browser token, and only while the
/// Library is unlocked or the request is the one that unlocks it
/// (spec: LA-14).
///
/// The session it found is put on the request, for the forward to read which
/// generation's key it goes with.
pub(crate) async fn admit(
    State(host): State<Arc<Host>>,
    mut request: Request,
    next: Next,
) -> Response {
    let session = match bearer(request.headers()).map(|token| host.gate.authenticate(token)) {
        Some(Ok(session)) => session,
        None | Some(Err(_)) => {
            return Refusal::AuthenticationRequired.answer("token", request.headers())
        }
    };
    if *session.instance_id() != host.gate.instance_id() {
        return Refusal::AuthenticationRequired.answer("generation", request.headers());
    }
    let unlocks = request.method() == Method::POST && request.uri().path() == UNLOCK;
    if host.lock.is_locked() && !unlocks {
        return Refusal::LibraryLocked.answer("locked", request.headers());
    }
    request.extensions_mut().insert(session);
    next.run(request).await
}
