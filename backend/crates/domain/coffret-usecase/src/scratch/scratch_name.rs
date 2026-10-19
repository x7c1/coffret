use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// A scratch name nothing else in a destination directory is using, held for
/// as long as the value lives.
///
/// What a writer keeps for the whole life of its scratch, and drops only after
/// the scratch was renamed onto its final name or removed: while it is held, a
/// sync in this process leaves the scratch alone, however long the writer takes
/// ([`is_stale`](super::is_stale)).
#[derive(Debug)]
pub struct ScratchName {
    name: String,
}

impl ScratchName {
    /// Holds `name` from now until the value drops.
    pub(super) fn held(name: String) -> Self {
        held().insert(name.clone());
        Self { name }
    }

    /// Whether a writer in this process holds `name` now.
    pub(super) fn is_held(name: &str) -> bool {
        held().contains(name)
    }

    /// The name itself.
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl Drop for ScratchName {
    fn drop(&mut self) {
        held().remove(&self.name);
    }
}

/// The names this process's writers hold.
///
/// A poisoned lock is taken all the same: the set is inserted into and removed
/// from one name at a time, so a panic elsewhere cannot leave it half-written.
fn held() -> MutexGuard<'static, BTreeSet<String>> {
    static HELD: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    HELD.lock().unwrap_or_else(PoisonError::into_inner)
}
