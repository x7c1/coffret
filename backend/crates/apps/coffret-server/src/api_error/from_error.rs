use axum::http::StatusCode;
use coffret_device::{Error, FetchError};

use super::ApiError;

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        match error {
            Error::Fetch { cause } => from_fetch(cause),
            // Everything else a Library can fail at here is the server's own
            // state rather than an answer about the request: a catalog that will
            // not open, a settings file that changed under the process. There is
            // nothing for the browser to do about any of them.
            other => ApiError::server(other),
        }
    }
}

/// What a fetch's own failure comes back as.
///
/// The distinctions are the ones a browser can act on. A path the Library holds
/// nothing at is the request's; a path no mapping reaches is this device's, and
/// the explorer says so as "this folder is not on this device" (spec: EP-9). A
/// Storage that did not answer and a Container that did not authenticate are
/// both `502`, and deliberately: the bytes never reached disk either way
/// (spec: EP-11), the failure is upstream of the browser, and there is nothing
/// the browser could do differently about the two.
fn from_fetch(cause: FetchError) -> ApiError {
    match cause {
        FetchError::EntryNotCurrent { .. } => ApiError::no_such_entry(),
        FetchError::UnmappedEntryPath { .. } => ApiError::declined_as(
            "unmapped",
            "no folder on this device holds this part of the Library",
            cause,
        ),
        FetchError::UnmaterializablePath { .. } | FetchError::LocalPathCollision { .. } => {
            ApiError::declined_as(
                "unmaterializable",
                "this device cannot hold a file at that path",
                cause,
            )
        }
        FetchError::Storage(_)
        | FetchError::Commit(_)
        | FetchError::ContainerUnreachable { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "storage",
            "the Library's Storage did not answer".to_owned(),
        )
        .caused_by(cause),
        FetchError::Format(_)
        | FetchError::CiphertextMismatch { .. }
        | FetchError::ContentMismatch { .. }
        | FetchError::EntryMissing { .. }
        | FetchError::UnmappedContainer { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "unverified",
            "what Storage answered with is not the content the Library names".to_owned(),
        )
        .caused_by(cause),
        FetchError::Index(_) | FetchError::Io { .. } => ApiError::server(cause),
    }
}
