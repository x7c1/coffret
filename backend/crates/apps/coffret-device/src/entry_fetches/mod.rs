//! One fetch per Entry Path at a time, for a process that serves more than one
//! reader.
//!
//! A command line asks for one Entry and waits for it, so nothing there can ask
//! twice at once. A server can: a reader that opens a page and prefetches the
//! next one, two tabs on one folder, a browser retrying a request it thinks
//! stalled — all of them arrive as two requests for one Entry Path, overlapping.
//!
//! Both would run the whole flow. Both would catch the catalog up, read the
//! committed Keyring, read the same parcels of the same Container, write a
//! scratch, and rename onto one path — the second one over a file the
//! first had already placed and marked present. Nothing is corrupted by that,
//! because a rename is atomic and each file is fully verified before it happens
//! (spec: EP-11); what is spent is the Container read twice, and what is
//! confused is the device's own bookkeeping, which briefly says a file was
//! materialized twice at one moment.
//!
//! So the second caller waits, and then asks the cheapest question there is:
//! whether the file is there now. Where the first caller placed it, the second
//! answers [`AlreadyPresent`](EntryFetch::AlreadyPresent) out of the catalog and
//! the file it names, having read nothing from Storage. Where the first caller
//! declined the path, the second runs the flow and arrives at the same finding
//! by itself — the wasted read being the price of not caching a verdict about a
//! folder that anything on this device may have changed in the meantime.
//!
//! The same holds one level up. A fetch reads whole parcels and keeps them
//! (spec: PK-16, PK-21), so two Entries of one Container asked for at once —
//! a reader's page and the next one its prefetch asks for — would both read
//! the parcel they share and both write the one file it is kept in. So fetches
//! of one Container take turns too, and the second one, once its turn comes,
//! usually finds its Entry already placed out of the parcel the first one read.
//!
//! It is a property of the process rather than of the Library, which is why it
//! is a value a process holds and not a field of
//! [`OpenLibrary`](crate::OpenLibrary): two Libraries open in one process have
//! nothing to coordinate, and one Library open in two processes cannot be
//! coordinated from here anyway.

use std::sync::{Arc, Mutex};

use coffret_model::{ContainerId, EntryPath, Redacted};
use coffret_usecase::fetch::{
    Cancellation, EntryFetch, EntryFetchOutcome, UnheldParcel, NEVER_CANCELLED, UNHEEDED,
};
use coffret_usecase::UNWATCHED;
use tokio::sync::oneshot;
use tracing::{debug, warn};

mod gates;
use gates::{Gates, Turn};

use crate::browse::EntryState;
use crate::error::{Error, Result};
use crate::open_library::OpenLibrary;

/// The Entry Paths, and the Containers, this process is fetching right now.
#[derive(Debug, Default)]
pub struct EntryFetches {
    in_flight: Gates<EntryPath>,
    containers: Gates<ContainerId>,
}

/// One caller's turn at an Entry Path and at the Container holding it.
struct Turns {
    _path: Turn<EntryPath>,
    _container: Option<Turn<ContainerId>>,
}

/// Where a reader's fetch is answered from: the moment its Entry is published,
/// or the run's own end where it never is.
///
/// Whichever comes first takes the sender, so the reader is answered once.
type Answer = Mutex<Option<oneshot::Sender<Result<EntryFetchOutcome>>>>;

impl EntryFetches {
    /// Nothing in flight.
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes the Entry at `path` available for a reader waiting on it,
    /// waiting for whoever is already fetching it.
    ///
    /// The same answers [`OpenLibrary::fetch_entry`] gives, and the same
    /// meaning for each, with one difference in timing: a run that places the
    /// Entry out of parcels answers [`Placed`](EntryFetch::Placed) when the
    /// chunks covering it have arrived, and what it had placed alongside and
    /// found not held by then. A parcel is read whole whatever Entry it was
    /// read for, and it is tens of megabytes; a reader shown nothing until the
    /// rest of it had arrived would wait for every page after theirs.
    ///
    /// The rest of the run — the parcel read to its end and kept, the Entries
    /// it covers placed, the parcels nothing waits for let go (spec: PK-21) —
    /// goes on in a task of its own, which keeps this Entry's and its
    /// Container's turns until it ends. So a second caller for the same
    /// Container still waits for the whole of it, and finds what it placed. A
    /// refusal the rest meets is logged and goes no further: the reader has
    /// its file, and a parcel that did not arrive whole is not held, so it is
    /// read again whole by whoever wants it next (spec: PK-21). The kept
    /// parcels the rest finds not held go to `later`, which is called once
    /// with them where there are any.
    ///
    /// The task holds the Library open until it ends, as any piece of work
    /// that took the keys finishes with them however a lock lands (spec:
    /// DK-2). A caller that stops waiting does not stop it either: the run is
    /// carried to its end whoever is still listening.
    pub async fn fetch<F>(
        &self,
        library: Arc<OpenLibrary>,
        path: EntryPath,
        later: F,
    ) -> Result<EntryFetchOutcome>
    where
        F: FnOnce(Vec<UnheldParcel>) + Send + 'static,
    {
        let turns = self.turns(&library, &path).await?;
        if already_present(&library, &path).await? {
            return Ok(EntryFetchOutcome::of(EntryFetch::AlreadyPresent));
        }

        let (sender, answered) = oneshot::channel();
        let answer: Arc<Answer> = Arc::new(Mutex::new(Some(sender)));
        let run = tokio::spawn({
            let answer = Arc::clone(&answer);
            async move {
                let _turns = turns;
                // How many kept parcels found not held went out with the
                // answer, so `later` hears of the rest and of none twice.
                let told = Mutex::new(0usize);
                let publication = |outcome: &EntryFetchOutcome| {
                    if let Some(sender) = take(&answer) {
                        *told.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
                            outcome.unheld.len();
                        // A reader that stopped waiting has nobody to hand
                        // this to, and the run goes on regardless.
                        let _ = sender.send(Ok(outcome.clone()));
                    }
                };
                // Nothing watches a reader's fetch, and nothing cancels it:
                // the request it serves is answered when the Entry is there,
                // and a phase in between has nowhere to go.
                let ran = library
                    .fetch_entry(path, &UNWATCHED, &NEVER_CANCELLED, &publication)
                    .await;
                if let Some(sender) = take(&answer) {
                    // Never published: the run's own end is the answer.
                    let _ = sender.send(ran);
                    return;
                }
                let told = told
                    .into_inner()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                finished_after_answer(ran, told, later);
            }
        });

        match answered.await {
            Ok(answer) => answer,
            // The sender is only ever dropped unsent by a task that panicked,
            // and the panic is the account of it.
            Err(_) => match run.await {
                Err(failed) if failed.is_panic() => std::panic::resume_unwind(failed.into_panic()),
                _ => unreachable!("a fetch task that ended answers before it does"),
            },
        }
    }

    /// The same, for a caller that waits for the whole run and may stop
    /// wanting the Entry: `cancellation` is asked before each parcel the run
    /// would request from Storage, and a run it stops answers
    /// [`EntryFetch::Cancelled`] with the parcels read so far still held
    /// (spec: PK-21).
    ///
    /// Answered at the run's end rather than at the Entry's publication,
    /// because what a caller filling a folder needs is the whole of it: which
    /// Entries the parcels placed alongside, so it does not ask for them again.
    pub async fn fetch_until(
        &self,
        library: &OpenLibrary,
        path: EntryPath,
        cancellation: &dyn Cancellation,
    ) -> Result<EntryFetchOutcome> {
        let _turns = self.turns(library, &path).await?;
        if already_present(library, &path).await? {
            return Ok(EntryFetchOutcome::of(EntryFetch::AlreadyPresent));
        }
        library
            .fetch_entry(path, &UNWATCHED, cancellation, &UNHEEDED)
            .await
    }

    /// Waits for whoever is fetching `path`, then for whoever is reading the
    /// Container holding it, and takes both turns.
    async fn turns(&self, library: &OpenLibrary, path: &EntryPath) -> Result<Turns> {
        let path_turn = self.in_flight.take(path.clone()).await;
        // The Container the catalog places the Entry in now. The run catches
        // the catalog up before it reads, so this can name the Container an
        // Entry has just left; what that costs is two runs over two Containers
        // not taking turns, which is what this gate added and nothing it
        // protects beyond.
        let container = library
            .index
            .entry_at(path)
            .await
            .map_err(|cause| Error::Index { cause })?
            .map(|location| location.container_id);
        let container_turn = match container {
            Some(container_id) => Some(self.containers.take(container_id).await),
            None => None,
        };
        Ok(Turns {
            _path: path_turn,
            _container: container_turn,
        })
    }
}

/// Whether whoever went first left the Entry on disk, asked once the turns are
/// taken.
///
/// Asked after the wait rather than before it, which is what makes it worth
/// asking: whoever went first may have placed the file — this Entry itself, or
/// one that came with the parcels it read — and the catalog is where that
/// shows (spec: EP-10).
///
/// The row is not the whole of the question, because a row outlives the file
/// somebody deleted out of the mapped folder. So the shortcut asks for the row
/// *and* a file standing at the path it names. That is not EP-11's
/// *agreement*, which holds the record's length and modification time against
/// the disk's and is the selection's question rather than this one; what is
/// decided here is only whether there is anything to serve. Where there is
/// not, the flow runs and states the finding, rather than this answering
/// "already present" about a file that is not there and leaving the caller
/// with nothing to say.
async fn already_present(library: &OpenLibrary, path: &EntryPath) -> Result<bool> {
    if library.state_of(path).await? == EntryState::Present
        && placed_file_stands(library, path).await
    {
        debug!(
            operation = "fetch_entry",
            verdict = "already present",
            "another caller had fetched this Entry",
        );
        return Ok(true);
    }
    Ok(false)
}

/// Takes the one sender, where nobody has yet.
fn take(answer: &Answer) -> Option<oneshot::Sender<Result<EntryFetchOutcome>>> {
    // A send is all anybody does with what is behind it, so a panic there left
    // the sender or nothing, and either is still the answer's state.
    answer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}

/// Says what the rest of a run answered at its Entry's publication came to.
///
/// Nobody is waiting on it: the reader has its file. A refusal is logged,
/// because it is the one account of a parcel that is now not held; the Entry
/// Path stays out of it (spec: EL-1). The kept parcels found not held after the
/// answer go to `later`, the ones before it having gone with the answer.
fn finished_after_answer<F>(ran: Result<EntryFetchOutcome>, told: usize, later: F)
where
    F: FnOnce(Vec<UnheldParcel>),
{
    match ran {
        Ok(mut outcome) => {
            let unheld = outcome.unheld.split_off(told.min(outcome.unheld.len()));
            if !unheld.is_empty() {
                later(unheld);
            }
        }
        Err(error) => warn!(
            operation = "fetch_entry",
            error = %error.redacted(),
            "the rest of a parcel read failed after its Entry was answered; \
             what did not arrive whole is not held",
        ),
    }
}

/// Whether the file the row names can be opened right now.
///
/// The second half of the shortcut's question, and the one answer it is allowed
/// to give is `true`. Every way of not being able to say so is `false` here
/// rather than a failure of the call: the file gone is the case the question is
/// asked for, and a link on the way down to it, a path no mapping reaches any
/// more, or a row the Library holds no current Entry at are each a verdict the
/// flow below owns and states in its own vocabulary. This is a shortcut past
/// that flow and not a second place to reach one of its verdicts, so it declines
/// to take itself and lets the flow answer.
///
/// Raising here instead would cost the caller that sentence twice over: it
/// would reach a reader as "the server could not answer" about a path the flow
/// would have named a reason for, and it would reach a caller filling a folder
/// as a failure about this device rather than about one Entry — the reading
/// that stops such a run on every file it had left. What being wrong costs is
/// one run of the flow instead, which is the price this module already puts on
/// not caching a verdict about a folder anything on this device may have
/// changed.
async fn placed_file_stands(library: &OpenLibrary, path: &EntryPath) -> bool {
    match library.open_local_file(path).await {
        Ok(standing) => standing.is_some(),
        // Not swallowed: the flow is about to ask the same question of the same
        // path and say what is wrong with it, and this is the one line saying
        // the shortcut met it first. The Entry Path stays out of the event as it
        // does everywhere else (spec: EL-1), and the cause goes in redacted.
        Err(cause) => {
            debug!(
                operation = "fetch_entry",
                verdict = "no shortcut",
                error = %cause.redacted(),
                "the placed file would not open, so the flow is what states why",
            );
            false
        }
    }
}
