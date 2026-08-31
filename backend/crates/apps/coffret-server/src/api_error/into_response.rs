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

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let Some(cause) = &self.cause {
            // The whole chain, and only here: what each layer reported, down to
            // the format crate's or the provider's own words. None of it carries
            // an Entry Path into the event, because what is logged is the
            // rendering of the error and not the request.
            error!(
                operation = "answer",
                status = self.status.as_u16(),
                kind = self.kind,
                error = %chain(cause.as_ref()),
                "a request was refused",
            );
        }
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
