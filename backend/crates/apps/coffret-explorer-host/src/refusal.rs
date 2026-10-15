use std::time::Duration;

use axum::http::header::{CONNECTION, CONTENT_LENGTH, RETRY_AFTER, TRANSFER_ENCODING};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// Every answer the host gives in place of the one a request asked for.
///
/// Each is one fixed status, one fixed `error` word and one fixed sentence:
/// none names the credential that was shown, the one that was expected, or
/// which of several ways a credential can fail it failed in (spec: LA-14,
/// LA-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// `403`: the request names somewhere the host is not, or a browser says
    /// another site's page made it (spec: LA-13, LA-14).
    OriginRefused,
    /// `401`: no credential that admits, however it failed to (spec: LA-14,
    /// LA-17).
    AuthenticationRequired,
    /// `423`: the Library is locked, and this is not a request that unlocks it
    /// (spec: LA-14).
    LibraryLocked,
    /// `429`: past the exchange limit; the next is let through after this long
    /// (spec: LA-17).
    RateLimited(Duration),
    /// `404`: a path under `/auth` that is not one of the host's own
    /// (spec: LA-16).
    NotFound,
    /// `405`: a method the route does not take, `OPTIONS` included
    /// (spec: LA-16).
    MethodNotAllowed,
    /// `400`: an exchange whose body is not JSON (spec: LA-16).
    NotJson,
    /// `415`: an exchange whose `Content-Type` is not JSON (spec: LA-16).
    NotJsonContent,
    /// `413`: more body than a request to `/auth` may bring (spec: LA-19).
    BodyTooLarge,
    /// `431`: more headers than any request may bring (spec: LA-19).
    HeadersTooLarge,
    /// `408`: a request to `/auth` that took too long to arrive (spec: LA-19).
    TooSlow,
    /// `502`: the server behind the host is gone, or is not the one the
    /// host's sessions were issued under; every session was revoked
    /// (spec: LA-15).
    ServerUnavailable,
    /// `500`: no credential could be drawn (spec: LA-20).
    NotDrawn,
}

#[derive(Serialize)]
struct Body {
    error: &'static str,
    message: &'static str,
}

impl Refusal {
    /// The answer, put into the log on the way as a refusal of this kind for
    /// `reason`, a fixed word naming the fence or the limit (spec: EL-1).
    ///
    /// Where `request` says a body is coming, the connection is closed after
    /// the answer rather than drained: the body is never read (spec: LA-19).
    pub(crate) fn answer(self, reason: &'static str, request: &HeaderMap) -> Response {
        tracing::info!(
            kind = "refusal",
            refusal = self.word(),
            reason,
            "a request was refused",
        );
        let mut response = (
            self.status(),
            Json(Body {
                error: self.word(),
                message: self.sentence(),
            }),
        )
            .into_response();
        if let Self::RateLimited(retry_after) = self {
            // Whole seconds, rounded up, so that a page that waits as long as it
            // was told is let through.
            let seconds = retry_after.as_millis().div_ceil(1000).max(1);
            response
                .headers_mut()
                .insert(RETRY_AFTER, HeaderValue::from(seconds as u64));
        }
        if carries_body(request) {
            response
                .headers_mut()
                .insert(CONNECTION, HeaderValue::from_static("close"));
        }
        response
    }

    fn status(self) -> StatusCode {
        match self {
            Self::OriginRefused => StatusCode::FORBIDDEN,
            Self::AuthenticationRequired => StatusCode::UNAUTHORIZED,
            Self::LibraryLocked => StatusCode::LOCKED,
            Self::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::NotJson => StatusCode::BAD_REQUEST,
            Self::NotJsonContent => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::BodyTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::HeadersTooLarge => StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            Self::TooSlow => StatusCode::REQUEST_TIMEOUT,
            Self::ServerUnavailable => StatusCode::BAD_GATEWAY,
            Self::NotDrawn => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The `error` word the page reads, and an event records the refusal by.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::OriginRefused => "origin_refused",
            Self::AuthenticationRequired => "authentication_required",
            Self::LibraryLocked => "library_locked",
            Self::RateLimited(_) => "rate_limited",
            Self::NotFound => "not_found",
            Self::MethodNotAllowed => "method_not_allowed",
            Self::NotJson => "not_json",
            Self::NotJsonContent => "unsupported_media_type",
            Self::BodyTooLarge => "body_too_large",
            Self::HeadersTooLarge => "headers_too_large",
            Self::TooSlow => "request_timeout",
            Self::ServerUnavailable => "server_unavailable",
            Self::NotDrawn => "credential_not_drawn",
        }
    }

    fn sentence(self) -> &'static str {
        match self {
            Self::OriginRefused => {
                "This explorer answers only its own page, at the address the app opened."
            }
            Self::AuthenticationRequired => {
                "This explorer holds no session. Open the explorer from the coffret app."
            }
            Self::LibraryLocked => {
                "The Library is locked. Unlock it from the coffret app to read it again."
            }
            Self::RateLimited(_) => {
                "The explorer was opened too many times at once. Try again in a moment."
            }
            Self::NotFound => "There is nothing here.",
            Self::MethodNotAllowed => "This address does not take that method.",
            Self::NotJson => "The request's body is not JSON.",
            Self::NotJsonContent => "The request's body has to be JSON.",
            Self::BodyTooLarge => "The request's body is larger than this address takes.",
            Self::HeadersTooLarge => "The request's headers are larger than this explorer takes.",
            Self::TooSlow => "The request took too long to arrive.",
            Self::ServerUnavailable => {
                "The coffret server behind this explorer is not the one it was opened for, or \
                 has stopped. Open the explorer from the coffret app again."
            }
            Self::NotDrawn => "No credential could be drawn. Try again.",
        }
    }
}

/// Whether a request says a body follows its head.
fn carries_body(headers: &HeaderMap) -> bool {
    let declared = headers
        .get(CONTENT_LENGTH)
        .and_then(|length| length.to_str().ok())
        .is_some_and(|length| length.trim() != "0");
    declared || headers.contains_key(TRANSFER_ENCODING)
}

#[cfg(test)]
mod contract;
