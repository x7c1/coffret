//! The refusal of a filter asked for in a way it cannot be answered: no name
//! and no count of books, an empty name, or a count that is not a positive
//! whole number (spec: LA-42).

use axum::http::StatusCode;

use super::ApiError;

impl ApiError {
    /// The refusal of one such filter; `what` says which way it fails, in a
    /// sentence a person can read.
    pub(crate) fn bad_filter(what: &str) -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_request",
            format!("that is not a filter: {what}"),
        )
    }
}
