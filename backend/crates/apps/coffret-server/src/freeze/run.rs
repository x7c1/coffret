use std::time::Instant;

use coffret_device::{Findings, DEFAULT_PACK_TARGET};
use tracing::info;

use crate::api_error::ApiError;
use crate::finding::Finding;
use crate::folder::Folder;
use crate::reported::Reported;
use crate::state::ServerState;
use crate::watched::Watched;

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
/// The activity is this function's own value, published once at the end — and
/// while it runs, what the flow says of itself is written onto the activity on
/// record. There is still no partial *outcome* to publish: a freeze commits one
/// batch, so until it has committed no count of Packs or Entries would be true.
/// What there is is where the run has got to, which is the flow's own answer
/// reported through the same port the command line draws its line from — and it
/// is the difference between a book of several hundred pages showing one fixed
/// sentence for minutes and showing that it is moving.
pub(super) async fn freeze(state: &ServerState, folder: &Folder) {
    let started = Instant::now();
    let mut activity = FreezeActivity::starting(folder.clone());

    // The keys, once, for the whole run, as the sync and the fill take them: a
    // lock that lands while a book is being packed leaves this holding what it
    // took, so the batch it is building is committed or abandoned whole
    // (spec: CP-1, DK-2), and a book armed after one stops here.
    let library = match state.unlocked() {
        Ok(library) => library,
        Err(refusal) => {
            activity.status = FreezeStatus::Stopped;
            activity.stopped = Some(Reported::recorded(&refusal, "freeze"));
            return finish(state, activity, started);
        }
    };

    // No terminal to draw a line on, so the steps go where this process says
    // what it is doing: the activity a browser polls (spec: LA-1). It is the
    // same port and the same steps the command line renders, so the two shells
    // cannot disagree about how far a run has got.
    let watched = Watched::by(|step| state.freezes.step(step));
    match library
        .freeze(folder.listed().cloned(), DEFAULT_PACK_TARGET, &watched)
        .await
    {
        Ok(outcome) => {
            activity.packs = outcome.packs.len();
            activity.entries = outcome.frozen_entries();
            activity.findings = Findings::from(&outcome)
                .iter()
                .filter_map(Finding::of)
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
/// user's own name for it (spec: EL-1): what is recorded of it is how long it
/// was, which is enough to read a run's account of itself without naming
/// anything a person has.
fn finish(state: &ServerState, activity: FreezeActivity, started: Instant) {
    info!(
        operation = "freeze",
        outcome = activity.status.as_str(),
        path_len = activity.folder.as_str().len(),
        packs = activity.packs,
        entries = activity.entries,
        findings = activity.findings.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "a folder was packed into the Library",
    );
    state.freezes.publish(&activity);
}
