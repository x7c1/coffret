use axum::http::StatusCode;
use coffret_device::{Error, FetchError, SyncError};

use super::ApiError;

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        match error {
            Error::Fetch { cause } => from_fetch(cause),
            Error::Sync { cause } => from_sync(cause),
            // Everything else a Library can fail at here is the server's own
            // state rather than an answer about the request: a catalog that will
            // not open, a settings file that changed under the process. There is
            // nothing for the browser to do about any of them.
            other => ApiError::server(other),
        }
    }
}

/// What a sync's own failure comes back as.
///
/// One distinction is worth drawing, and it is the one the fetch draws: Storage
/// did not answer. That is the failure somebody can act on — the connection is
/// gone, the grant has run out — and it is the one the retry is offered from, so
/// it says so rather than arriving as "the server could not answer" beside a
/// button.
///
/// Everything else is this device: its catalog, its disk, a filename that spells
/// no Entry Path, two files claiming one (spec: EP-1, EP-4). None of them is
/// anything a browser can do differently about. An object that did not arrive at
/// Storage whole is `unverified` for the reason the fetch's mismatches are: what
/// is at the far end is not the content this device named.
fn from_sync(cause: SyncError) -> ApiError {
    match cause {
        SyncError::Storage(_) | SyncError::Commit(_) | SyncError::ListingLimitReached { .. } => {
            ApiError::plain(
                StatusCode::BAD_GATEWAY,
                "storage",
                "the Library's Storage did not answer".to_owned(),
            )
            .caused_by(cause)
        }
        SyncError::TransferCorrupted { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "unverified",
            "what reached Storage is not the content this device sent".to_owned(),
        )
        .caused_by(cause),
        SyncError::Index(_)
        | SyncError::Format(_)
        | SyncError::Io { .. }
        | SyncError::UnrepresentableName { .. }
        | SyncError::PathCollision { .. } => ApiError::server(cause),
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
