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

    /// This device holds no file at the path, and the caller asked to be
    /// answered from this device alone: the file route's peek (spec: EP-10).
    ///
    /// The same kind and status as [`no_such_entry`](Self::no_such_entry),
    /// because to whoever peeked it is the same answer — there is nothing here
    /// to show — and a caller that peeks has already decided not to fetch.
    /// The sentence says which "here" it was about.
    pub fn not_on_this_device() -> Self {
        Self::plain(
            StatusCode::NOT_FOUND,
            "no_such_entry",
            "this device holds no file at that path".to_owned(),
        )
    }
}
