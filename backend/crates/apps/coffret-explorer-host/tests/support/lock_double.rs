use std::sync::atomic::{AtomicBool, Ordering};

use coffret_explorer_host::LibraryLock;

/// Whether the Library is locked, as the case says.
#[derive(Default)]
pub struct LockDouble(AtomicBool);

impl LockDouble {
    pub fn set(&self, locked: bool) {
        self.0.store(locked, Ordering::SeqCst);
    }
}

impl LibraryLock for LockDouble {
    fn is_locked(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
