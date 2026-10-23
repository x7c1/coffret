use std::borrow::Cow;

use axum::http::header::CONNECTION;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use super::RefusalReason;
use crate::attempt::AttemptRefused;

/// Every answer the door gives in place of the one a request asked for.
///
/// Each is a fixed status and a fixed `error` word, with the sentence the
/// window shows. None names the token that was shown, or which of the ways a
/// token can fail it failed in (spec: LA-23).
#[derive(Debug)]
pub(crate) enum Refusal {
    /// `403`: the request names somewhere the door is not, or does not say the
    /// door's own page made it (spec: LA-22).
    OriginRefused,
    /// `431`: more headers than any request may bring.
    HeadersTooLarge,
    /// `401`: no token the door holds, however it failed to be one.
    AuthenticationRequired,
    /// `415`: a submission whose `Content-Type` is not JSON.
    NotJsonContent,
    /// `413`: more body than a submission may bring.
    BodyTooLarge,
    /// `408`: a body that took too long to arrive.
    TooSlow,
    /// `400`: a body that is not the JSON a submission is.
    NotJson,
    /// `422`: the submission was read, and opening or unlocking the Library
    /// refused it; the sentence is the refusal's own line
    /// ([`AttemptRefused::line()`]).
    Attempt(AttemptRefused),
    /// `500`: the Libraries on this device could not be listed.
    LibrariesUnread,
    /// `404`: a path the door does not answer.
    NotFound,
}

/// What a refusal's body holds.
#[derive(Serialize)]
struct Body<'a> {
    error: &'static str,
    message: &'a str,
    /// Whether the window's token has ended with this answer, so that the
    /// window has to be opened again before anything else can be submitted.
    ended: bool,
}

impl Refusal {
    /// The answer, put into the log on the way as a refusal of this kind for
    /// `reason`: a fixed word naming the fence or the limit and, for a body
    /// that could not be read, which way it failed (spec: EL-1, EL-3, EL-4).
    ///
    /// `ended` says whether the window's token is gone. The connection is
    /// closed after the answer, so that a body a refused request was bringing
    /// is never read.
    pub(crate) fn answer(self, reason: impl Into<RefusalReason>, ended: bool) -> Response {
        let reason = reason.into();
        tracing::info!(
            kind = "door_refusal",
            refusal = self.word(),
            reason = reason.word,
            failure = reason.failure(),
            io_kind = reason.io_kind.map(tracing::field::debug),
            ended,
            "a request to the door was refused",
        );
        let mut response = (
            self.status(),
            Json(Body {
                error: self.word(),
                message: &self.sentence(),
                ended,
            }),
        )
            .into_response();
        response
            .headers_mut()
            .insert(CONNECTION, HeaderValue::from_static("close"));
        response
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::OriginRefused => StatusCode::FORBIDDEN,
            Self::HeadersTooLarge => StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            Self::AuthenticationRequired => StatusCode::UNAUTHORIZED,
            Self::NotJsonContent => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::BodyTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::TooSlow => StatusCode::REQUEST_TIMEOUT,
            Self::NotJson => StatusCode::BAD_REQUEST,
            Self::Attempt(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::LibrariesUnread => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }

    pub(crate) fn word(&self) -> &'static str {
        match self {
            Self::OriginRefused => "origin_refused",
            Self::HeadersTooLarge => "headers_too_large",
            Self::AuthenticationRequired => "authentication_required",
            Self::NotJsonContent => "unsupported_media_type",
            Self::BodyTooLarge => "body_too_large",
            Self::TooSlow => "request_timeout",
            Self::NotJson => "bad_request",
            Self::Attempt(_) => "refused",
            Self::LibrariesUnread => "libraries_unread",
            Self::NotFound => "not_found",
        }
    }

    fn sentence(&self) -> Cow<'static, str> {
        let fixed = match self {
            Self::OriginRefused => "This request did not come from the Passphrase window.",
            Self::HeadersTooLarge => "The request's headers are too large.",
            Self::AuthenticationRequired => {
                "This window can no longer take the Passphrase. Open it again from the app."
            }
            Self::NotJsonContent | Self::NotJson => "The window sent something it should not.",
            Self::BodyTooLarge => "The Passphrase is too long.",
            Self::TooSlow => "The window took too long to send the Passphrase.",
            Self::Attempt(refused) => return Cow::Owned(refused.line()),
            Self::LibrariesUnread => "The Libraries on this device could not be listed.",
            Self::NotFound => "There is nothing here.",
        };
        Cow::Borrowed(fixed)
    }
}
