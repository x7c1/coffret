use std::sync::{Mutex, MutexGuard};

use coffret_model::EntryPath;

use crate::commit::DegradedKeyring;
use crate::fetch::entry_fetch::{EntryFetch, EntryFetchOutcome};
use crate::fetch::publication::Publication;
use crate::fetch::unheld_parcel::UnheldParcel;

/// What one read of parcels has to tell its caller the moment the Entry asked
/// for is published, and the kept parcels it found not held, which are part of
/// that.
///
/// The parcels are walked by [`Spread`](super::spread::Spread), which is where
/// the Entry is published from, and the kept parcels found not held are met by
/// the walk around it — so both hand what they know to this one value, which
/// each borrows. The list sits behind a lock that is taken only to push or to
/// copy and never across an await.
pub(super) struct Herald<'h> {
    publication: &'h dyn Publication,
    degraded: Option<DegradedKeyring>,
    unheld: Mutex<Vec<UnheldParcel>>,
}

impl<'h> Herald<'h> {
    /// A read telling `publication`, with what the run read of the committed
    /// Keyring before it started.
    pub(super) fn new(publication: &'h dyn Publication, degraded: Option<DegradedKeyring>) -> Self {
        Self {
            publication,
            degraded,
            unheld: Mutex::new(Vec::new()),
        }
    }

    /// Takes in a kept parcel found not held.
    pub(super) fn unheld(&self, parcel: UnheldParcel) {
        self.locked().push(parcel);
    }

    /// Tells the caller the Entry asked for is on disk, with the Entries
    /// `alongside` it placed before it (spec: PK-16).
    pub(super) fn published(&self, alongside: &[EntryPath]) {
        let unheld = self.locked().clone();
        self.publication.published(&EntryFetchOutcome {
            fetch: EntryFetch::Placed,
            degraded: self.degraded,
            alongside: alongside.to_vec(),
            unheld,
        });
    }

    /// Every kept parcel found not held, in the order they were met.
    pub(super) fn into_unheld(self) -> Vec<UnheldParcel> {
        self.unheld
            .into_inner()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn locked(&self) -> MutexGuard<'_, Vec<UnheldParcel>> {
        // A push or a copy is all anybody does under it, so a panic there left
        // a list behind and nothing half-written.
        self.unheld
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
