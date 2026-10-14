use axum::extract::Request;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS,
    X_FRAME_OPTIONS,
};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;

use crate::page::content_security_policy;

/// Puts the headers every answer of the host carries on it, whatever answered.
///
/// On every answer: no referrer, and no sniffing a type other than the one
/// given. On every answer under `/api` and `/auth`, none of it stored, since
/// what they answer is the Library or a credential. On every other — the page
/// and its assets — the content security policy, and no framing by any other
/// page (spec: LA-16).
pub(crate) async fn answer_headers(request: Request, next: Next) -> Response {
    let credentialed = is_credentialed(request.uri().path());
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    if credentialed {
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    } else {
        headers.insert(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(content_security_policy()),
        );
        headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    }
    response
}

/// Whether `path` is under `/api` or `/auth`.
fn is_credentialed(path: &str) -> bool {
    ["/api", "/auth"].iter().any(|prefix| {
        path.strip_prefix(prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
}
