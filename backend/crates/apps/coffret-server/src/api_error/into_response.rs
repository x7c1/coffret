use std::error;

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
    /// The whole chain: what each layer reported, down to the format crate's or
    /// the provider's own words. None of it carries an Entry Path into the
    /// event, because what is logged is the rendering of the error and not the
    /// request.
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
            error = %chain(cause.as_ref()),
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

/// One error and everything under it, on one line.
fn chain(error: &(dyn error::Error + 'static)) -> String {
    let mut said = error.to_string();
    let mut under = error.source();
    while let Some(cause) = under {
        said.push_str(": ");
        said.push_str(&cause.to_string());
        under = cause.source();
    }
    said
}
