use std::sync::{Arc, Mutex};

use crate::in_memory_fs::state::State;

// What a case sets up on the fake before a run, and what it reads back off it
// afterwards. Neither is anything a flow can see: a flow sees the two
// capabilities below and nothing else.
mod arranging;

mod inspecting;

// The two capabilities the fake answers, an impl to a module, as the device's
// own filesystem answers them in the local filesystem gateway. A third would be
// a third module here rather than more of this one.
mod mapped_roots;

mod spool;

// What each of them hands out while a run is still reading or writing through
// it, and the state all four work against.
mod in_memory_source_reader;

mod in_memory_writer;

mod state;

/// A [`Spool`] and a [`MappedRoots`] that keep everything in memory, for tests.
///
/// It stands to those two as [`InMemoryStore`](crate::InMemoryStore) stands to
/// [`ObjectStore`](crate::ObjectStore), and it earns its place for one reason
/// beyond needing no directory: it can be told to fail. The rules a sync and a
/// freeze keep around the local disk are rules about interruption and about
/// absence — [`Spool`] states the first (spec: OC-2, OC-6) and [`MappedRoots`]
/// the second (spec: EP-12) — and a real filesystem cannot be asked to refuse a
/// chosen step. [`fail_on`](Self::fail_on) is what asks.
///
/// One fake for both capabilities, because one device has one disk: a case
/// scripts a folder that will not list and a spool that will not flush against
/// the same thing, and the mapped folders and the spool directory are simply two
/// places in it.
///
/// What it models of a filesystem is only what the flows above can tell apart:
/// which directories were made, what each file holds and when it was last
/// modified and born, which names are neither a file nor a folder, and what
/// filesystem a root stands on. There are no permissions and no links — a
/// planted "other" is what a link is here.
///
/// [`Spool`]: crate::Spool
/// [`MappedRoots`]: crate::MappedRoots
#[derive(Debug, Default)]
pub struct InMemoryFs {
    // Behind an `Arc` because a writer or a reader outlives the call that handed
    // it over and works against the same fake, the way a file handle does.
    state: Arc<Mutex<State>>,
}

impl Clone for InMemoryFs {
    /// A second handle on the *same* disk, and not a copy of it.
    ///
    /// What is behind an [`InMemoryFs`] is already shared — a writer or a reader
    /// it hands out goes on working against it after the call that made it has
    /// returned — so a clone is one more way to reach that same state, the way
    /// two file descriptors on one filesystem are. A case that wants a second,
    /// empty disk calls [`new`](Self::new).
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl InMemoryFs {
    /// An empty filesystem: no directories and no files.
    pub fn new() -> Self {
        Self::default()
    }

    /// The fake's state, taken even from a lock a panicking case poisoned: what
    /// is behind it is a case's own bookkeeping, and a poisoned lock would
    /// replace the failure that panicked with one about the lock.
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
