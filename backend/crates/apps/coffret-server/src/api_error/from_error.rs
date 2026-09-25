use axum::http::StatusCode;
use coffret_device::{
    CommitError, Error, FetchError, FormatError, FreezeError, Redacted, StorageError, SyncError,
};

use super::ApiError;

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        match error {
            // A catalog that could not be used, whichever entry point met it:
            // the device layer reports it under this one name, taking it out of
            // the fetch's vocabulary on the way, so it is classified once rather
            // than once per gesture.
            Error::Index { .. } => catalog_unusable(error.redacted()),
            Error::Fetch { cause } => from_fetch(*cause),
            // The same answers, because the verdicts are the same ones: this is
            // EP-9's translation reported on its own, and what a browser does
            // about an unmapped path does not depend on whether a transfer was
            // going to follow it.
            Error::LocalPathNotSettled { cause } => from_fetch(*cause),
            // And the same again for a file turned away on its way into a
            // mapped folder: what a browser can do about an unmapped path, a
            // path no file here can stand for, or a name coffret keeps for
            // itself is the same whichever side of the Library the request was
            // moving the file towards.
            Error::FileNotTakenIn { cause } => from_fetch(*cause),
            // And once more for a read of what somebody has put in a mapped
            // folder. A name a case-folding volume will not tell apart from
            // this device's own management area is the same verdict about the
            // same disk whether the request was reading that folder or writing
            // into it, and the browser is given the same answer to act on.
            Error::LocalFilesNotRead { cause } => from_fetch(*cause),
            // And a fourth time for the opening of the file this device placed
            // for an Entry, which is the reading a browser does every time
            // somebody opens a picture. The same verdicts about the same path,
            // so the same answers: whether a transfer was going to follow the
            // translation is this server's own business and nothing a page
            // branches on.
            Error::LocalFileNotOpened { cause } => from_fetch(*cause),
            Error::Sync { cause } => from_sync(*cause),
            Error::Freeze { cause } => from_freeze(*cause),
            Error::CatchUp { cause } => from_commit(&cause, cause.redacted()),
            // The verdict a single writer gets when the folder it was to place
            // into is not the folder the mapping was recorded against — a
            // dropped file, most of the time (spec: EP-13). It is the same state
            // a fetch meets, so it is answered the same way rather than falling
            // into the catch-all below and reaching the browser as a `500` that
            // says nothing about a mapping.
            Error::RootRefused(ref refusal) => {
                ApiError::refused_root(refusal.prefix.as_ref(), &error)
            }
            // Everything else a Library can fail at here is the server's own
            // state rather than an answer about the request: a settings file
            // that changed under the process, a mapped
            // root whose own marker this process may not read
            // (`RootUnvouched`, spec: EP-13) — that one deliberately, because
            // the browser is told nothing about a mapping nothing was learned
            // about. There is nothing for the browser to do about any of them.
            // How far one of them reaches inside a request is decided by
            // whoever is taking that request and never by the kind chosen here.
            other => ApiError::server(other.redacted()),
        }
    }
}

/// What a commit's own failure comes back as, whichever flow met it.
///
/// One classification for the four flows that go through the commit flow — a
/// catch-up replaying the Journal, and a sync, a freeze and a fetch each
/// wrapping what it met there — because a commit's verdict is the same verdict
/// whoever asked for the commit. A wrapping flow that restated a narrower one
/// would file every commit failure as Storage not answering, which is false of
/// most of them — an epoch above all, which no retry mends.
///
/// `commit` is what is classified and `cause` is what the log is given: the
/// redacted rendering of the whole failure as the flow reported it, so that a
/// sync's commit failure reads as a sync's in the log while being answered the
/// way the catch-up's is.
///
/// The distinction a browser can act on first: Storage did not answer, so the
/// catalog stands wherever the flow had got to — for a catch-up a head the
/// Library really committed, since records are applied one at a time — and the
/// control is worth pressing again once there is a bucket to reach. A head the
/// listing named and that could not be opened is on that side of the line too —
/// nothing here can tell an object that has just been pruned from one a proxy
/// swallowed, and both are answered by asking again.
///
/// A control object that arrived and is not one is `unverified`, for the reason
/// a fetch's mismatches are: what is at the far end is not what this Library
/// names.
///
/// An epoch this device holds no Master Key for is `epoch`, and is neither of
/// those nor any of the rest: it is a state of the Library, permanent until
/// this device is re-enrolled in the new epoch (spec: CP-5, MR-2), so pressing
/// the control again will never clear it. [`ApiError::epoch`] says why it has
/// the status it has.
///
/// The rest are `500`. A catalog that would not take a record is this device's
/// own. The commit's own verdicts — a slot lost too often, a Keyring left
/// incomplete, a committed Keyring it could not repair (spec: KL-16), a path
/// claimed twice, a Container no catalog maps, a control value it assembled
/// that the rules do not admit — are about what this device assembled or the
/// state its commit met, and nothing a browser can do differently about; a
/// catch-up, which writes nothing, never reaches them at all. All of them travel
/// to the log, where whoever is keeping the Library will read them, and none of
/// them says anything further to a screen.
fn from_commit(commit: &CommitError, cause: String) -> ApiError {
    match commit {
        CommitError::Storage(storage) => from_storage(storage, cause),
        CommitError::MissingHead { .. } | CommitError::KeyringUnreadable { .. } => {
            storage_did_not_answer(cause)
        }
        // No control object states a length this build cannot address, so the
        // commit never raises it; named anyway, so that the arm below keeps
        // accusing only the bytes whatever reaches it.
        CommitError::Format(FormatError::UnaddressableOnThisBuild { .. }) => {
            ApiError::server(cause)
        }
        CommitError::Format(_) | CommitError::CorruptControlObject { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "unverified",
            "what Storage answered with is not the control state the Library names".to_owned(),
        )
        .caused_by(cause),
        CommitError::EpochActivated { .. } => ApiError::epoch(cause),
        CommitError::Index(_) => catalog_unusable(cause),
        CommitError::EntryPathCollision { .. }
        | CommitError::UnmappedContainer { .. }
        | CommitError::UnwritableControlValue { .. }
        | CommitError::IncompleteKeyring { .. }
        | CommitError::UnrepairedKeyring { .. }
        | CommitError::ConflictLimitReached { .. } => ApiError::server(cause),
    }
}

/// What Storage's own verdict comes back as, whichever flow carried it.
///
/// Nearly all of them are Storage not coming through, and a browser is told
/// that and offered the retry. The one that is not is a listing that outran
/// the pages this device reads of one: Storage answered every page, so
/// saying it did not answer would be false, and it is answered the way the
/// flows' own listing caps are ([`listing_ran_past_its_cap`]). Every variant is
/// listed rather than left to a wildcard, so that a verdict added to the port
/// has to be placed here on purpose.
fn from_storage(storage: &StorageError, cause: String) -> ApiError {
    match storage {
        StorageError::ListingPastCap { .. } => listing_ran_past_its_cap(cause),
        StorageError::NotFound { .. }
        | StorageError::AlreadyExists { .. }
        | StorageError::PermissionDenied { .. }
        | StorageError::LimitReached { .. }
        | StorageError::Unauthenticated { .. }
        | StorageError::IntegrityMismatch { .. }
        | StorageError::NotPurged { .. }
        | StorageError::Unsupported { .. }
        | StorageError::Rejected { .. }
        | StorageError::MalformedResponse { .. }
        | StorageError::LengthMismatch { .. }
        | StorageError::LengthOverrun { .. }
        | StorageError::ObjectTooLong { .. }
        | StorageError::Io { .. }
        | StorageError::RateLimited { .. }
        | StorageError::ServiceUnavailable { .. }
        | StorageError::Timeout { .. }
        | StorageError::Transport { .. }
        | StorageError::Model(_) => storage_did_not_answer(cause),
    }
}

/// A catalog that could not be used, whichever flow or door met it.
///
/// This device's own state rather than an answer about the request, and nothing
/// a browser can do differently about, so it is a `500` whose cause goes to the
/// log. One function because it is one verdict: a device refusal reports it as
/// [`Error::Index`] from every entry point, and a sync, a freeze, a fetch or a
/// catch-up that met it inside its own flow carries it in that flow's
/// vocabulary — each of which is sent here rather than deciding for itself.
fn catalog_unusable(cause: String) -> ApiError {
    ApiError::server(cause)
}

/// Storage did not come through: the failure the retry is offered from.
///
/// One sentence for every flow, written once, because it is the one a browser
/// shows beside a button that asks again.
fn storage_did_not_answer(cause: String) -> ApiError {
    ApiError::plain(
        StatusCode::BAD_GATEWAY,
        "storage",
        "the Library's Storage did not answer".to_owned(),
    )
    .caused_by(cause)
}

/// A listing of Storage that did not end within the pages this device reads.
///
/// Still `storage`, because what is wrong is on that side and nothing a browser
/// branches on differs. But not the sentence above, which would be false:
/// Storage answered every page it was asked for, and what happened is that the
/// listing went on past the cap this device puts on one. One sentence for the
/// sync's and the freeze's own caps and for the Storage port's, whichever flow
/// met it, for the reason the one above is one.
fn listing_ran_past_its_cap(cause: String) -> ApiError {
    ApiError::plain(
        StatusCode::BAD_GATEWAY,
        "storage",
        "the Library's Storage answered, but its listing ran past the cap on how many pages \
         this device reads of one"
            .to_owned(),
    )
    .caused_by(cause)
}

/// What a sync's own failure comes back as.
///
/// One distinction is worth drawing, and it is the one the fetch draws: Storage
/// did not answer. That is the failure somebody can act on — the connection is
/// gone, the grant has run out — and it is the one the retry is offered from, so
/// it says so rather than arriving as "the server could not answer" beside a
/// button. A listing that ran past its cap is on the same side and says what it
/// is, and a commit's failure is classified as every commit's is
/// ([`from_commit`]).
///
/// Everything else is this device: its catalog, its disk, a filename that spells
/// no Entry Path, two files claiming one (spec: EP-1, EP-4), a folder whose name
/// folds to `.coffret` without being it (spec: EP-14). None of them is anything
/// a browser can do differently about. The last two are settled by renaming
/// something, and that is not a page's gesture either: what would be renamed is
/// a local path, which does not cross this boundary at all (spec: EL-1), so the
/// sentence naming the folder is the one a terminal shows and the page is told
/// the run stopped. An object that did not arrive at Storage whole is
/// `unverified` for the reason the fetch's mismatches are: what is at the far
/// end is not the content this device named.
fn from_sync(cause: SyncError) -> ApiError {
    match cause {
        SyncError::Storage(ref storage) => from_storage(storage, cause.redacted()),
        SyncError::Commit(ref commit) => from_commit(commit, cause.redacted()),
        SyncError::ListingLimitReached { .. } => listing_ran_past_its_cap(cause.redacted()),
        SyncError::TransferCorrupted { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "unverified",
            "what reached Storage is not the content this device sent".to_owned(),
        )
        .caused_by(cause.redacted()),
        SyncError::Index(_) => catalog_unusable(cause.redacted()),
        SyncError::Format(_)
        | SyncError::Io { .. }
        | SyncError::UnrepresentableName { .. }
        | SyncError::FoldedReservedName { .. }
        | SyncError::PathCollision { .. } => ApiError::server(cause.redacted()),
    }
}

/// What a freeze's own failure comes back as.
///
/// The same line the sync draws, and for the same reason: Storage did not
/// answer is the failure somebody can act on, and it is the one the retry is
/// offered from — so it says so rather than arriving as "the server could not
/// answer" beside a button that packs the book again. The listing's cap and the
/// commit's failures are answered as the sync answers them.
///
/// A Pack whose object did not arrive whole is `unverified` for the reason the
/// sync's is: what is at the far end is not the content this device sent, and
/// the batch was never committed.
///
/// Everything else is this device: its catalog, its disk, a filename that spells
/// no Entry Path, two files claiming one (spec: EP-1, EP-4), a folder whose name
/// folds to `.coffret` without being it (spec: EP-14), and a file that stopped
/// being the file the scan measured while its Pack was being written. None of
/// them is anything a browser can do differently about, and the two a rename
/// settles are settled at a terminal for the reason the sync's are — and none of
/// them costs the retry, which is offered from
/// the stopped state whatever stopped it: a freeze that failed committed nothing
/// (spec: CP-1), so every page is still sitting in the folder and eligible
/// again.
fn from_freeze(cause: FreezeError) -> ApiError {
    match cause {
        FreezeError::Storage(ref storage) => from_storage(storage, cause.redacted()),
        FreezeError::Commit(ref commit) => from_commit(commit, cause.redacted()),
        FreezeError::ListingLimitReached { .. } => listing_ran_past_its_cap(cause.redacted()),
        FreezeError::TransferCorrupted { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "unverified",
            "what reached Storage is not the content this device sent".to_owned(),
        )
        .caused_by(cause.redacted()),
        FreezeError::Index(_) => catalog_unusable(cause.redacted()),
        FreezeError::Format(_)
        | FreezeError::Io { .. }
        | FreezeError::UnrepresentableName { .. }
        | FreezeError::FoldedReservedName { .. }
        | FreezeError::PathCollision { .. }
        | FreezeError::SourceChanged { .. } => ApiError::server(cause.redacted()),
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
/// the browser could do differently about the two. A failure the fetch met in
/// the commit flow is classified as every commit's is ([`from_commit`]).
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
        // Its own reason rather than `unmaterializable`, because the two send a
        // person to different places: that one says no local name can stand for
        // the path at all, and this says exactly one name in it is coffret's own
        // (spec: EP-11's scratch, EP-14's management area). The component stays
        // out of the body the way an Entry Path does — it is a piece of one —
        // and the sentence names the reserved names instead, which nobody chose.
        // Both of them rather than the one this path carries, since the component
        // is what stayed out: a person reads this beside the name they dropped,
        // and a sentence naming no name at all would leave them working out for
        // themselves which of their components it meant.
        //
        // Both of them and no spelling of either, which is what the arm below is
        // for: `.COFFRET` is nothing coffret keeps for itself, and a sentence
        // saying it did would have somebody reading their own folder off the
        // screen beside a claim about it that is not true.
        FetchError::ReservedComponent { .. } => ApiError::declined_as(
            "reserved",
            "that path carries a name coffret keeps for itself inside a mapped folder: \
             `.coffret`, or a name beginning `.coffret-fetch-`",
            cause,
        ),
        // The same reason, because it is the same thing for a browser to show
        // and the same thing to do about: one name is not available, and the
        // rest of the path is fine. What differs is the sentence, and all of it
        // differs. The name is the person's rather than coffret's; the folder
        // may be standing *in* the one they asked for rather than in the path,
        // since a listing is refused whole where a name in it folds (spec:
        // EP-14), so "that path carries" would be false of it; and the gesture
        // is a rename where the one above is a different path.
        //
        // The spelling still cannot be said here, the component being a piece of
        // an Entry Path (spec: EL-1). What the sentence offers instead is the
        // one name it may say and the relation the person's own name has to it,
        // which is enough to find on a screen.
        FetchError::FoldedReservedComponent { .. } => ApiError::declined_as(
            "reserved",
            "a name in that path, or in a folder standing in it, differs from `.coffret` only \
             in case — and `.coffret` is the name coffret keeps for its own folder inside a \
             mapped folder, which it settles by name. Rename that folder, or ask for a path \
             that does not carry the spelling",
            cause,
        ),
        FetchError::Storage(ref storage) => from_storage(storage, cause.redacted()),
        FetchError::ContainerUnreachable { .. } => storage_did_not_answer(cause.redacted()),
        FetchError::Commit(ref commit) => from_commit(commit, cause.redacted()),
        // This build cannot address a length the Container states, which says
        // nothing about what Storage answered with: a 64-bit build opens what
        // this one refuses. So it is not `unverified`, and it is nothing a
        // browser can do differently about either — it is this server's own
        // limit, and it goes to the log as that.
        FetchError::Format(FormatError::UnaddressableOnThisBuild { .. }) => {
            ApiError::server(cause.redacted())
        }
        FetchError::Format(_)
        | FetchError::CiphertextMismatch { .. }
        | FetchError::ContentMismatch { .. }
        | FetchError::EntryMissing { .. }
        | FetchError::UnmappedContainer { .. } => ApiError::plain(
            StatusCode::BAD_GATEWAY,
            "unverified",
            "what Storage answered with is not the content the Library names".to_owned(),
        )
        .caused_by(cause.redacted()),
        // A mapped root that is not the folder its mapping was recorded against
        // is this device's configuration rather than the server failing
        // (spec: EP-13), so it is declined with a reason of its own: the gesture
        // that settles it is at a terminal, and a person told only that the
        // server could not answer would never learn there is one.
        FetchError::RefusedRoot(ref refusal) => {
            ApiError::refused_root(refusal.prefix.as_ref(), &cause)
        }
        // Named because the vocabulary still has the variant, and never met
        // from a device refusal: the device layer lifts it out into its own
        // `Index` before wrapping the rest. Sent to the same classification as
        // that one all the same, so the two cannot come to answer differently.
        FetchError::Index(_) => catalog_unusable(cause.redacted()),
        // A time this device's clock cannot reach is this device's limit, as
        // the disk's own refusal is, and nothing a browser can do differently
        // about either.
        FetchError::Io { .. } | FetchError::UnstampableMtime { .. } => {
            ApiError::server(cause.redacted())
        }
    }
}
