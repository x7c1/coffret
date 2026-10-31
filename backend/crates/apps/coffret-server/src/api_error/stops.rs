//! The refusals about naming an add: a name that is not one, and a name
//! another add under way already goes by (spec: LA-35) — and the one refusal
//! of a stop itself, of an add that cannot be undone (spec: LA-36).

use axum::http::StatusCode;
use coffret_device::EntryPath;

use crate::folder::Folder;

use super::{ApiError, CONFLICT};

impl ApiError {
    /// The name a request gave its add is not one a page may give: one to
    /// sixty-four ASCII letters, digits, `-` or `_`.
    pub(crate) fn bad_add() -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "the add is named by one to sixty-four letters, digits, - or _".to_owned(),
        )
    }

    /// Another add under way already goes by the name this request gave its
    /// own, so a stop asked by that name would not say which it means.
    pub(crate) fn add_under_way() -> Self {
        Self::plain(
            StatusCode::CONFLICT,
            CONFLICT,
            "an add by this name is already under way; name this one differently".to_owned(),
        )
    }

    /// The add onto `folder` wrote `path` over a file this device could not
    /// keep aside — its folder's filesystem makes no hard link — so undoing it
    /// could not put that file back, and the stop is refused rather than leave
    /// the add's file in its place (spec: LA-36).
    ///
    /// Says what the person asked and why it could not be done, naming the
    /// folder as the add's line does and the file, as a refusal about one of
    /// the person's own files does, so they know which one it is; nothing was
    /// marked, and the add goes on to its run.
    pub(crate) fn replaced_for_good(folder: &Folder, path: &EntryPath) -> Self {
        let folder = match folder.as_str() {
            "" => "the Library root",
            named => named,
        };
        Self::plain(
            StatusCode::CONFLICT,
            CONFLICT,
            format!(
                "could not stop adding to {folder} — {path} replaced a file that cannot be put back"
            ),
        )
    }
}
