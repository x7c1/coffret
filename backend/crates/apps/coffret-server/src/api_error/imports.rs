//! The refusals about a drop by path as a request: a handle that holds no drop,
//! and a file named that the drop does not hold or that changed since it was
//! dropped.

use axum::http::StatusCode;
use coffret_device::DropRefusal;

use super::{redact, ApiError, CONFLICT};

impl ApiError {
    /// The handle an import named holds no drop: it was never drawn, it was
    /// used already, or it was held past its lifetime (spec: LA-26).
    ///
    /// One sentence for all three, which a page answers the same way — by
    /// asking for the files to be dropped again — and the log is told which.
    pub(crate) fn drop_not_held(refusal: DropRefusal) -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "that drop is no longer held — drop the files again".to_owned(),
        )
        .caused_by(redact::NotHeld(refusal))
    }

    /// An import named a file by a position the drop holds none at
    /// (spec: LA-27).
    pub(crate) fn not_in_the_drop() -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "the request names a file the drop does not hold".to_owned(),
        )
    }

    /// A file an import was reading grew past the length it had when it was
    /// dropped (spec: LA-28).
    ///
    /// About the file and not the request: the rest of the drop goes on, and
    /// the person drops this one again once it has stopped changing.
    pub(crate) fn changed_since_dropped() -> Self {
        Self::plain(
            StatusCode::CONFLICT,
            CONFLICT,
            "the file changed after it was dropped — drop it again".to_owned(),
        )
    }
}
