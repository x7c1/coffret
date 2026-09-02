use std::time::Instant;

use coffret_device::{Findings, DEFAULT_PACK_TARGET};
use tracing::info;

use crate::api_error::ApiError;
use crate::folder::Folder;
use crate::noted::Noted;
use crate::reported::Reported;
use crate::state::ServerState;

use super::{FreezeActivity, FreezeStatus};

/// Packs one folder of the Library, once.
///
/// [`freeze`](coffret_device::OpenLibrary::freeze) and nothing around it. What
/// is eligible is the pack policy's answer and not this server's — a file new to
/// the Library, or one whose Entry a one-file Container holds — and an Entry
/// already inside a Pack is never among them (spec: PK-1, PK-2). The folder is
/// the run's prefix, which narrows it and never widens it (spec: PK-17): a
/// folder outside every mapping selects nothing, which is why the route refuses
/// one before arming this at all.
///
/// The target is the device layer's default and not a choice made here. What
/// size a Pack should be is a measurement question (spec: PK-5, PK-6), and a
/// second answer to it living in a server would be a Library packed differently
/// depending on which shell asked.
///
/// The activity is this function's own value, published once at the end. Like a
/// sync and unlike a fill there is nothing to publish along the way: a freeze
/// commits one batch, so until it has committed there is no partial answer that
/// would be true.
pub(super) async fn freeze(state: &ServerState, folder: &Folder) {
    let started = Instant::now();
    let mut activity = FreezeActivity::starting(folder.clone());

    match state
        .library
        .freeze(folder.listed().cloned(), DEFAULT_PACK_TARGET)
        .await
    {
        Ok(outcome) => {
            activity.packs = outcome.packs.len();
            activity.entries = outcome.frozen_entries();
            activity.noted = Findings::from(&outcome)
                .iter()
                .filter_map(Noted::of)
                .collect();
            activity.status = FreezeStatus::Done;
        }
        Err(error) => {
            activity.status = FreezeStatus::Stopped;
            activity.stopped = Some(Reported::recorded(&ApiError::from(error), "freeze"));
        }
    }
    finish(state, activity, started);
}

/// Publishes what the freeze came to, and records it.
///
/// Counts, a duration and an outcome. The folder is an Entry Path and so is the
/// user's own name for it (spec: EP-1): what is recorded of it is how long it
/// was, which is enough to read a run's account of itself without naming
/// anything a person has.
fn finish(state: &ServerState, activity: FreezeActivity, started: Instant) {
    info!(
        operation = "freeze",
        outcome = activity.status.as_str(),
        path_len = activity.folder.as_str().len(),
        packs = activity.packs,
        entries = activity.entries,
        noted = activity.noted.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "a folder was packed into the Library",
    );
    state.freezes.publish(&activity);
}
