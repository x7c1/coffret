use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use coffret_usecase::fetch::{EntryFetchOutcome, Publication, UnheldParcel};

use super::answered::{take, Answer, Answered};

/// What the run of a reader's fetch tells as it goes: the publication, which
/// answers the reader, and each kept parcel it finds not held.
///
/// The parcels are kept here, by the caller, rather than only on what the run
/// returns: a run that fails after its answer returns a refusal and none of
/// what it found, and a parcel that did not arrive whole is said however the
/// rest ends (spec: PK-21).
pub(super) struct Listener {
    pub(super) answer: Arc<Answer>,
    /// Every kept parcel found not held, in the order they were met.
    found: Mutex<Vec<UnheldParcel>>,
    /// How many of them went out with the answer, so `later` hears of the rest
    /// and of none twice.
    told: AtomicUsize,
}

impl Listener {
    pub(super) fn new(answer: Arc<Answer>) -> Self {
        Self {
            answer,
            found: Mutex::new(Vec::new()),
            told: AtomicUsize::new(0),
        }
    }

    /// The kept parcels found not held after the answer went.
    pub(super) fn found_after_answer(self) -> Vec<UnheldParcel> {
        // A push is all anybody does under it, so a panic there left a list.
        let mut found = self
            .found
            .into_inner()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let told = self.told.load(Ordering::SeqCst).min(found.len());
        found.split_off(told)
    }
}

impl Publication for Listener {
    fn published(&self, outcome: &EntryFetchOutcome) {
        if let Some(sender) = take(&self.answer) {
            self.told.store(outcome.unheld.len(), Ordering::SeqCst);
            // A reader that stopped waiting has nobody to hand this to, and the
            // run goes on regardless.
            let _ = sender.send(Answered::Ran(Ok(outcome.clone())));
        }
    }

    fn unheld(&self, parcel: &UnheldParcel) {
        self.found
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(parcel.clone());
    }
}
