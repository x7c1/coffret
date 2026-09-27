//! The refusals about the path a caller named: not an Entry Path, or one the
//! Library holds nothing at.

use std::fmt;

use axum::http::StatusCode;

use super::ApiError;

impl ApiError {
    /// The text a caller sent is not an Entry Path (spec: EP-2).
    ///
    /// `defect` says how it failed the shape, in the words the model refuses it
    /// in ([`PathDefect`](coffret_device::PathDefect)) — a caller told only that
    /// their path was refused has no way to find the one component that made it
    /// so. It is taken as anything that can say itself rather than as a string,
    /// so that the route hands the refusal along instead of restating it.
    pub fn bad_path(defect: impl fmt::Display) -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_path",
            format!("that is not an Entry Path: {defect}"),
        )
    }

    /// The Library holds no current Entry at the path (spec: EP-5).
    pub fn no_such_entry() -> Self {
        Self::plain(
            StatusCode::NOT_FOUND,
            "no_such_entry",
            "the Library holds nothing at that path".to_owned(),
        )
    }
}
