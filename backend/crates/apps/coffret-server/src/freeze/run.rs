use std::collections::BTreeSet;
use std::time::Instant;

use coffret_device::{EntryPath, Findings, DEFAULT_PACK_TARGET};
use tracing::info;

use crate::adds::{log_undone, undo};
use crate::api_error::ApiError;
use crate::finding::Finding;
use crate::pending_rows::TurnTaker;
use crate::reported::Reported;
use crate::state::ServerState;
use crate::watched::Watched;

use super::{Book, FreezeRun, FreezeStatus};

/// Packs one folder of the Library, once.
///
/// [`freeze`](coffret_device::OpenLibrary::freeze) and nothing around it. What
/// is eligible is the pack policy's answer and not this server's — a file new to
/// the Library, or one whose Entry a one-file Container holds — and an Entry
/// already inside a Pack is never among them (spec: PK-1, PK-2). The folder is
/// the run's prefix, which narrows it and never widens it (spec: PK-17): a
/// folder outside every mapping selects nothing, which is why the route refuses
/// one before arming this at all. Where the book names its files — a drop
/// does — the run is narrowed again to exactly those, so a one-file Entry
/// already in the folder is not drawn into a Pack nobody asked for.
///
/// The target is the device layer's default and not a choice made here. What
/// size a Pack should be is a measurement question (spec: PK-5, PK-6), and a
/// second answer to it living in a server would be a Library packed differently
/// depending on which shell asked.
///
/// The run is this function's own value, published once at the end — and
/// while it runs, what the flow says of itself is written onto the run on
/// record. There is still no partial *outcome* to publish: a freeze commits one
/// batch, so until it has committed no count of Packs or Entries would be true.
/// What there is is where the run has got to, which is the flow's own answer
/// reported through the same port the command line draws its line from — and it
/// is the difference between a book of several hundred pages showing one fixed
/// sentence for minutes and showing that it is moving.
///
/// # Stopped by a person
///
/// A Stop on the run's line, or on an add whose files it packs, asks it to
/// stop, and the flow is raced against that ask, which wins only while the run
/// has not begun committing (spec: LA-35). A freeze commits one batch
/// (spec: PK-7), so a run cut short before its commit leaves no Entry of its
/// book in a Pack it built: what it spooled is settled by the next run, and
/// what it sent to Storage is left to the cleanup of orphaned objects
/// (spec: OC-2). Then, under the turn, it undoes the adds a person stopped
/// (spec: LA-36), logs each, and arms again the part of its book nobody
/// stopped.
pub(super) async fn freeze(state: &ServerState, book: Book) {
    let started = Instant::now();
    let prefix = book.folder.listed().cloned();
    let only = book.only.clone();
    let mut run = FreezeRun::starting(book);
    // A book one of whose adds was stopped while it waited is asked from the
    // start.
    if !state.adds.stopping_of(&run.adds).is_empty() {
        state.freezes.ask_stop();
    }

    // This device's pending rows are the freeze's for the whole run, and a
    // sync or a deletion already running holds them: wait for the turn rather
    // than being refused them (spec: OC-2). Before the keys, so the wait is
    // not counted as somebody being here (spec: DK-4) and a lock that lands
    // meanwhile stops this run below.
    let _turn = state.pending_rows.take(TurnTaker::Freeze).await;

    // The keys, once, for the whole run, as the sync and the fill take them: a
    // lock that lands while a book is being packed leaves this holding what it
    // took, so the batch it is building is committed or abandoned whole
    // (spec: CP-1, DK-2), and a book armed after one stops here.
    let library = match state.unlocked() {
        Ok(library) => library,
        Err(refusal) => {
            run.status = FreezeStatus::Stopped(Reported::recorded(&refusal, "freeze"));
            return finish(state, run, started);
        }
    };

    if !state.freezes.asked() {
        // No terminal to draw a line on, so the steps go where this process
        // says what it is doing: the work answer a browser polls (spec: LA-1).
        // It is the same port and the same steps the command line renders, so
        // the two shells cannot disagree about how far a run has got.
        let watched = Watched::by(|step| state.freezes.step(step));
        let ran = tokio::select! {
            // The ask first: one standing is one the run has not turned away
            // by saying it is committing.
            biased;
            () = state.freezes.until_asked() => None,
            ran = library.freeze(prefix, only, DEFAULT_PACK_TARGET, &watched) => Some(ran),
        };
        if let Some(ran) = ran {
            match ran {
                Ok(outcome) => {
                    // Counts and findings, and not `outcome.mappings`: a device
                    // that maps nothing reads here as a folder with nothing to
                    // pack, and the explorer tells the two apart out of its own
                    // listing, which says whether the folder is mapped. So the
                    // work answer carries no count of mappings.
                    run.packs = outcome.packs.len();
                    run.entries = outcome.frozen_entries();
                    run.findings = Finding::all_of(&Findings::from(&outcome));
                    run.status = FreezeStatus::Done;
                }
                Err(error) => {
                    // What the run did before it stopped still happened: the
                    // Keyring replicas its commit put back stand on Storage
                    // whatever became of the batch, and a repair performed is
                    // never silent (spec: KL-15).
                    run.findings = Finding::all_of(&Findings::repaired_before(&error));
                    run.status =
                        FreezeStatus::Stopped(Reported::recorded(&ApiError::from(error), "freeze"));
                }
            }
            return finish(state, run, started);
        }
    }

    // Stopped by a person, before the commit.
    let stopped = state.adds.stopping_of(&run.adds);
    let mut taken_away: BTreeSet<EntryPath> = BTreeSet::new();
    for id in &stopped {
        if let Some(add) = state.adds.get(id) {
            taken_away.extend(add.reserved.iter().cloned());
            log_undone(&state.activity, undo(&library, add).await);
            state.adds.end(id);
        }
    }
    let rest = Book {
        folder: run.folder.clone(),
        only: run
            .only
            .as_ref()
            .map(|only| only.difference(&taken_away).cloned().collect()),
        adds: run.adds.difference(&stopped).cloned().collect(),
        requested: state.freezes.requested(),
        withdrawn: false,
    };
    // Armed again for what is still asked for once the stopped adds are
    // undone: the adds nobody stopped, and a request of the book's own that
    // a Stop on its line did not take away (spec: LA-36). A book that was
    // only its adds, every one of them stopped, is not armed again — not even
    // where a join left it with no selection, which would pack the whole
    // folder nobody asked for. A request made after the stop was asked is
    // not taken into the run (see `Progress::arming`) and waits its own turn.
    let rearm = !rest.adds.is_empty() || rest.requested;
    run.status = FreezeStatus::Cancelled;
    finish(state, run, started);
    if rearm {
        state.freezes.arm_dropped(rest);
    }
}

/// Publishes what the freeze came to, and records it.
///
/// Counts, a duration and an outcome. The folder is an Entry Path and so is the
/// user's own name for it (spec: EL-1): what is recorded of it is how long it
/// was, which is enough to read a run's account of itself without naming
/// anything a person has.
fn finish(state: &ServerState, run: FreezeRun, started: Instant) {
    info!(
        operation = "freeze",
        outcome = run.status.as_str(),
        path_len = run.folder.as_str().len(),
        selection = run.only.as_ref().map(BTreeSet::len),
        packs = run.packs,
        entries = run.entries,
        findings = run.findings.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "a folder was packed into the Library",
    );
    // The adds a run that ended carried end with it, whatever it came to: a
    // stop that met its commit is answered by that commit (spec: LA-35).
    if run.status != FreezeStatus::Cancelled {
        state.adds.end_all(&run.adds);
    }
    state.freezes.publish(&run);
}
