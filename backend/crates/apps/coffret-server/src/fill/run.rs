use std::time::Instant;

use coffret_device::{EntryFetch, EntryPath, EntryState, Error, FetchError};
use tracing::info;

use crate::api_error::ApiError;
use crate::folder::Folder;
use crate::reported::Reported;
use crate::state::ServerState;

use super::{Activity, Declined, FillStatus};

/// Brings the rest of one folder over.
///
/// The listing says which of the folder's files this device does not have
/// (spec: EP-10), and each of them goes through the very same per-Entry
/// single-flight fetch the routes use. That is the point of sharing that gate: a
/// reader's prefetch, a second click and this can all ask for one Entry, and it
/// is placed once rather than once per caller: one temporary file inside the
/// mapped folder, and one rename into place (spec: EP-11).
///
/// One Entry at a time, by range read, exactly as a click on it would be. A
/// folder is often one Pack's worth of Entries, so bringing them over one at a
/// time reads the front of one object once per Entry and catches the catalog up
/// once per Entry too; coalescing the adjacent Entries of one Pack into a single
/// read is the obvious next thing, and PK-16 makes it legitimate whenever it is
/// measured to be worth doing — a range read is a step inside fetching the
/// containing Container rather than a fetch unit of its own. It is deliberately
/// not done here: this reuses the placement discipline unchanged, and the first
/// shape of a fill should not be one that can place a file differently from the
/// way a click on it would.
///
/// The activity is this function's own value, published after every change. One
/// worker runs at a time and nothing else writes an activity while one does, so
/// what the browser polls is always this fill's account of itself rather than
/// two halves of two.
pub(super) async fn fill(state: &ServerState, folder: &Folder) {
    let started = Instant::now();
    let mut activity = Activity::starting(folder.clone());

    let listing = match state.library.list(folder.listed()).await {
        Ok(listing) => listing,
        Err(error) => {
            activity.stop(Reported::recorded(&ApiError::from(error), "fill"));
            return finish(state, activity, started);
        }
    };

    // A folder no mapping of this device reaches has nowhere to put a file, so
    // there is nothing here to bring over (spec: EP-9) — and asking anyway would
    // be one catalog catch-up per file to be told so once per file. The explorer
    // says this over the rows already, out of the listing itself.
    if !listing.mapped {
        activity.status = FillStatus::Done;
        return finish(state, activity, started);
    }

    let wanted: Vec<EntryPath> = listing
        .files
        .iter()
        .filter(|file| file.state == EntryState::Remote)
        .map(|file| file.path.clone())
        .collect();
    activity.total = wanted.len();
    state.fills.publish(&activity);

    for path in wanted {
        if state.fills.superseded() {
            // Someone opened a file in another folder, and the fill follows them
            // there rather than finishing the one they have left.
            activity.status = FillStatus::Superseded;
            return finish(state, activity, started);
        }
        match state.fetches.fetch(&state.library, path.clone()).await {
            Ok(EntryFetch::Placed | EntryFetch::AlreadyPresent) => activity.done += 1,
            Ok(EntryFetch::Surfaced(surfaced)) => {
                activity.decline(
                    &path,
                    Reported::recorded(&ApiError::declined(&surfaced), "fill"),
                );
            }
            // A refusal about this one Entry, recorded like a declined verdict:
            // the next file is a separate question.
            Err(error) if is_about_one_entry(&error) => {
                activity.decline(&path, Reported::recorded(&ApiError::from(error), "fill"));
            }
            Err(error) => {
                activity.stop(Reported::recorded(&ApiError::from(error), "fill"));
                return finish(state, activity, started);
            }
        }
        state.fills.publish(&activity);
    }

    activity.status = FillStatus::Done;
    finish(state, activity, started)
}

/// Whether a failure is about the one Entry that met it, rather than something
/// every other Entry of the folder would meet the same way.
///
/// Read off the failure itself rather than off the kind it goes out to the
/// browser under. They answer alike today — the two below are exactly the two
/// that reach a page as `declined` and `no_such_entry` — but they are different
/// questions: one is what a browser branches on, and this one is whether there
/// is any point in asking for the next file. Asked of the value, a variant added
/// to a fetch's vocabulary stops this compiling until somebody says which of the
/// two it is; asked of the name, it would quietly join whichever side the
/// spelling fell on.
///
/// The two that are about one Entry: a path this device declined to place a file
/// at (spec: EP-11), and a path the Library no longer holds an Entry at
/// (spec: EP-5) — an Entry that went between the listing and the fetch, which
/// says nothing about the next one. Everything else is Storage, the catalog, or
/// this device, and pressing on would be asking a broken question once per file.
fn is_about_one_entry(error: &Error) -> bool {
    // Anything that is not the fetch's own refusal is this device reading its
    // own catalog on the way in, which the next Entry reads the same way.
    let Error::Fetch { cause } = error else {
        return false;
    };
    match cause {
        FetchError::UnmappedEntryPath { .. }
        | FetchError::UnmaterializablePath { .. }
        | FetchError::LocalPathCollision { .. }
        | FetchError::EntryNotCurrent { .. } => true,
        FetchError::Storage(_)
        | FetchError::Index(_)
        | FetchError::Format(_)
        | FetchError::Commit(_)
        | FetchError::Io { .. }
        | FetchError::ContainerUnreachable { .. }
        | FetchError::CiphertextMismatch { .. }
        | FetchError::EntryMissing { .. }
        | FetchError::ContentMismatch { .. }
        | FetchError::UnmappedContainer { .. } => false,
    }
}

/// Publishes what the fill came to, and records it.
///
/// Counts, a duration and an outcome. The folder is an Entry Path and so is the
/// user's own name for it (spec: EP-1): what is recorded of it is how long it
/// was, which is enough to read a run's account of itself without naming
/// anything a person has.
fn finish(state: &ServerState, activity: Activity, started: Instant) {
    info!(
        operation = "fill",
        outcome = activity.status.as_str(),
        path_len = activity.folder.as_str().len(),
        total = activity.total,
        done = activity.done,
        declined = activity.declined.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "a folder was brought over",
    );
    state.fills.publish(&activity);
}

impl Activity {
    /// Records one Entry the fill did not bring over.
    fn decline(&mut self, path: &EntryPath, refusal: Reported) {
        self.declined.push(Declined {
            path: path.as_str().to_owned(),
            refusal,
        });
    }

    /// Records the refusal that stopped the fill.
    fn stop(&mut self, refusal: Reported) {
        self.status = FillStatus::Stopped;
        self.stopped = Some(refusal);
    }
}
