use std::sync::{Arc, PoisonError, RwLock};

use coffret_device::OpenLibrary;

/// Where the unlocked Library is kept, and the one thing that can be emptied.
///
/// One cell rather than a field per thing a key reaches. What the Passphrase
/// produced is an [`OpenLibrary`], the keys are inside it, and putting the whole
/// of it here means there is exactly one place a lock has to act on — and
/// nowhere for a second reference to the keys to be left behind when it does.
///
/// Handed out as an `Arc` and never as a borrow. A borrow would have to be held
/// for as long as the work using it, which is the whole span of a request, and
/// a lock would then either wait for every reader or tear one in half. A handle
/// costs one atomic increment and lets the two happen at once: the lock takes
/// the cell's own reference away, the work that already has one finishes, and
/// the keys are wiped by the last handle to go (spec: DK-7).
pub(crate) struct Custody {
    held: RwLock<Option<Arc<OpenLibrary>>>,
}

impl Custody {
    /// Holds a Library the Passphrase has just opened (spec: DK-1).
    pub(crate) fn holding(library: OpenLibrary) -> Self {
        Self {
            held: RwLock::new(Some(Arc::new(library))),
        }
    }

    /// A handle on the open Library, and nothing at all once it is locked.
    pub(crate) fn unlocked(&self) -> Option<Arc<OpenLibrary>> {
        self.read().clone()
    }

    /// Empties it, which is the lock (spec: DK-3).
    ///
    /// It has taken effect when this returns: the cell is empty before the
    /// guard is let go, so nothing that asks after this point is handed a key.
    /// What was taken is dropped outside the guard, because dropping it runs
    /// every gateway's own teardown and none of that belongs inside a lock this
    /// narrow.
    ///
    /// `true` where this call is the one that emptied it, which is what tells a
    /// first lock from a second.
    pub(crate) fn lock(&self) -> bool {
        let taken = self
            .held
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        taken.is_some()
    }

    /// The cell, whatever a guard was left holding.
    ///
    /// A lock is poisoned by a panic under its own guard and by nothing else,
    /// and nothing inside either of these two can panic — a clone of an `Arc`
    /// and a `take`. So this is a state that cannot be reached rather than one
    /// that is handled, and recovering rather than unwrapping is how that is
    /// said: what is behind the lock is a whole `Option` either way, and a
    /// server that refused to read its own cell ever again would answer nothing
    /// and could not even be locked.
    fn read(&self) -> std::sync::RwLockReadGuard<'_, Option<Arc<OpenLibrary>>> {
        self.held.read().unwrap_or_else(PoisonError::into_inner)
    }
}
