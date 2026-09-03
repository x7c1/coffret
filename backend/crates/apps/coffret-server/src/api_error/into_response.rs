use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use tracing::error;

use super::ApiError;

/// What a refusal looks like on the wire.
#[derive(Serialize)]
struct Refusal<'a> {
    error: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    surfaced: Option<&'a str>,
}

impl ApiError {
    /// Puts what the layer below reported into the log.
    ///
    /// The whole chain, redacted: what each layer reported, down to the format
    /// crate's or the provider's own answer, as identities and log-safe facts
    /// rather than as messages. Nothing here is left to render unsafely:
    /// `cause` is already that rendering and never the failure itself (see
    /// [`redact`](super::redact)).
    ///
    /// A method rather than a line inside the response, because there are two
    /// things that become of a refusal now — a request answered with it, and a
    /// fill that declined one Entry or was stopped by it — and the cause has to
    /// reach the log from both. It reaches nothing else from either: a body must
    /// not carry it, and an activity keeps only the sentence a person reads.
    pub(crate) fn record(&self, operation: &'static str) {
        let Some(cause) = &self.cause else {
            return;
        };
        error!(
            operation,
            status = self.status.as_u16(),
            kind = self.kind,
            error = cause.as_str(),
            "something was refused",
        );
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        self.record("answer");
        (
            self.status,
            Json(Refusal {
                error: self.kind,
                message: &self.message,
                reason: self.reason,
                surfaced: self.surfaced,
            }),
        )
            .into_response()
    }
}
