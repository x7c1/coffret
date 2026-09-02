use axum::http::StatusCode;
use coffret_device::{CommitError, Error, FetchError, FreezeError, SyncError};

use super::ApiError;

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        match error {
            Error::Fetch { cause } => from_fetch(cause),
            Error::Sync { cause } => from_sync(cause),
            Error::Freeze { cause } => from_freeze(cause),
            Error::CatchUp { cause } => from_catch_up(cause),
            // Everything else a Library can fail at here is the server's own
            // state rather than an answer about the request: a catalog that will
            // not open, a settings file that changed under the process. There is
            // nothing for the browser to do about any of them.
            other => ApiError::server(other),
        }
    }
}

/// What a catch-up's own failure comes back as.
///
/// The same distinction the other two draw, because it is the one a browser can
/// act on: Storage did not answer, so the catalog stands wherever the replay had
/// got to — a head the Library really committed, since records are applied one
/// at a time — and the refresh is worth pressing again once there is a bucket to
/// reach, carrying on from there rather than from the beginning. A head the
/// listing named and that could not be opened is on that side of the line too —
/// nothing here can tell an object that has just been pruned from one a proxy
/// swallowed, and both are answered by asking again.
///
/// A control object that arrived and is not one is `unverified`, for the reason
/// a fetch's mismatches are: what is at the far end is not what this Library
/// names.
///
/// The rest are `500`, and not for one reason. A catalog that would not take a
/// record is this device's own, exactly as a sync's is. The commit's own
/// verdicts — a slot lost, a Keyring left incomplete, a path claimed twice, a
/// Container no catalog maps — a catch-up never reaches at all, because nothing
/// in it commits. And an epoch this device holds no Master Key for is neither of
/// those: it is a state of the Library, permanent until this device is
/// re-enrolled in the new epoch (spec: CP-5, MR-2), so pressing the control
/// again will never clear it. It arrives as `500` because the kinds a browser
/// branches on hold no name for it and no page can offer the re-enrolment. All
/// of them travel to the log, where whoever is keeping the Library will read
/// them, and none of them says anything further to a screen.
fn from_catch_up(cause: CommitError) -> ApiError {
    match cause {
        CommitError::Storage(_)
        | CommitError::MissingHead { .. }
        | CommitError::KeyringUnreadable { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "storage",
            "the Library's Storage did not answer".to_owned(),
        )
        .caused_by(cause),
        CommitError::Format(_) | CommitError::CorruptControlObject { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "unverified",
            "what Storage answered with is not the control state the Library names".to_owned(),
        )
        .caused_by(cause),
        CommitError::Index(_)
        | CommitError::EpochActivated { .. }
        | CommitError::EntryPathCollision { .. }
        | CommitError::UnmappedContainer { .. }
        | CommitError::IncompleteKeyring { .. }
        | CommitError::ConflictLimitReached { .. } => ApiError::server(cause),
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

/// What a freeze's own failure comes back as.
///
/// The same line the sync draws, and for the same reason: Storage did not
/// answer is the failure somebody can act on, and it is the one the retry is
/// offered from — so it says so rather than arriving as "the server could not
/// answer" beside a button that packs the book again.
///
/// A Pack whose object did not arrive whole is `unverified` for the reason the
/// sync's is: what is at the far end is not the content this device sent, and
/// the batch was never committed.
///
/// Everything else is this device: its catalog, its disk, a filename that spells
/// no Entry Path, two files claiming one (spec: EP-1, EP-4), and a file that
/// stopped being the file the scan measured while its Pack was being written.
/// None of them is anything a browser can do differently about — and none of
/// them costs the retry, which is offered from the stopped state whatever
/// stopped it: a freeze that failed committed nothing (spec: CP-1), so every
/// page is still sitting in the folder and eligible again.
fn from_freeze(cause: FreezeError) -> ApiError {
    match cause {
        FreezeError::Storage(_)
        | FreezeError::Commit(_)
        | FreezeError::ListingLimitReached { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "storage",
            "the Library's Storage did not answer".to_owned(),
        )
        .caused_by(cause),
        FreezeError::TransferCorrupted { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "unverified",
            "what reached Storage is not the content this device sent".to_owned(),
        )
        .caused_by(cause),
        FreezeError::Index(_)
        | FreezeError::Format(_)
        | FreezeError::Io { .. }
        | FreezeError::UnrepresentableName { .. }
        | FreezeError::PathCollision { .. }
        | FreezeError::SourceChanged { .. } => ApiError::server(cause),
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
