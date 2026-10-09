use std::ops::Deref;
use std::sync::Arc;

use coffret_device::OpenLibrary;

use crate::lock::Idle;

/// One piece of work's hold on the open Library, from the moment it is taken to
/// the moment it is let go.
///
/// The `Arc` inside is what keeps the keys alive while the work runs, so a lock
/// that lands in the middle of one cannot tear it in half (spec: DK-2): the cell
/// gives up its own reference and this one finishes. Everything the Library can
/// be asked reaches through it, so a caller holds one of these wherever it would
/// otherwise have held the Library itself.
///
/// What it adds is the span. Taking it and letting it go are both somebody being
/// here, and the stretch between them counts as well (spec: DK-4) — because a
/// lock landing in the middle of a long piece of work would leave that work with
/// nowhere to go: it completes on the handle it holds, and then everything it
/// arms next is refused, so the explorer offers to pack the book again and
/// cannot. Since the mark is made here rather than at each caller, work added
/// later is counted for its span by holding one of these.
pub(crate) struct KeyHandle {
    /// The open Library, for as long as this handle lives.
    library: Arc<OpenLibrary>,
    /// The clock this span is written on, both ends of it.
    idle: Arc<Idle>,
}

impl KeyHandle {
    /// Takes hold of the open Library, which is somebody being here.
    pub(crate) fn taken(library: Arc<OpenLibrary>, idle: Arc<Idle>) -> Self {
        idle.taken();
        Self { library, idle }
    }

    /// The open Library itself, for work that goes on after this handle's span
    /// has ended.
    ///
    /// A reader's fetch is answered the moment its Entry is placed, and the rest
    /// of the parcel it came out of is read by a task of its own (see
    /// [`EntryFetches::fetch`](coffret_device::EntryFetches::fetch)). That task
    /// holds the keys until it ends, as this handle would (spec: DK-2), and it
    /// is not somebody being here: it is the tail of a request that has already
    /// been answered, so it marks no span (spec: DK-4).
    pub(crate) fn shared(&self) -> Arc<OpenLibrary> {
        Arc::clone(&self.library)
    }
}

impl Deref for KeyHandle {
    type Target = OpenLibrary;

    fn deref(&self) -> &Self::Target {
        &self.library
    }
}

impl Drop for KeyHandle {
    /// Ends the span, and the next interval is counted from here.
    fn drop(&mut self) {
        self.idle.released();
    }
}
