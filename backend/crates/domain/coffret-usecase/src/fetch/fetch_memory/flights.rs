use std::sync::Arc;

use tokio::sync::watch;

use super::{FetchMemory, Kept};

/// The reads of one kind under way, each under the key a second reader of the
/// same thing would ask by.
///
/// A handful at most — one listing, one key table, a folder's fronts — so a
/// list searched in full is all the structure it needs.
#[derive(Debug)]
pub(super) struct Flights<K, T> {
    under_way: Vec<(K, Landing<T>)>,
}

/// Where one read under way lands what it came to, `None` until it has.
type Landing<T> = Arc<watch::Sender<Option<T>>>;

impl<K, T> Default for Flights<K, T> {
    fn default() -> Self {
        Self {
            under_way: Vec::new(),
        }
    }
}

impl<K: PartialEq, T> Flights<K, T> {
    /// A wait on the read under way for `key`, where one is.
    pub(super) fn join(&self, key: &K) -> Option<Waiter<T>> {
        self.under_way
            .iter()
            .find(|(under_way, _)| under_way == key)
            .map(|(_, landing)| Waiter(landing.subscribe()))
    }

    /// Records a read for `key` as under way, and answers with where its
    /// reader lands what it came to.
    pub(super) fn start(&mut self, key: K) -> Landing<T> {
        let (landing, _) = watch::channel(None);
        let landing = Arc::new(landing);
        self.under_way.push((key, Arc::clone(&landing)));
        landing
    }

    /// Forgets the read that lands at `landing`, and no other: a read for the
    /// same key started since is another reader's.
    fn end(&mut self, landing: &Landing<T>) {
        self.under_way
            .retain(|(_, under_way)| !Arc::ptr_eq(under_way, landing));
    }

    /// How many readers wait on the reads under way, between them.
    #[cfg(all(test, feature = "conformance"))]
    pub(super) fn waiting(&self) -> usize {
        self.under_way
            .iter()
            .map(|(_, landing)| landing.receiver_count())
            .sum()
    }
}

/// What a reader of something [`FetchMemory`] keeps is to do.
pub(crate) enum Turn<'m, V, K: PartialEq, T> {
    /// It is kept: here it is, and nothing is read.
    Kept(V),
    /// Another reader is reading it: wait for what that read comes to.
    Join(Waiter<T>),
    /// Nobody is: read it, and land what the read came to for whoever joins
    /// meanwhile.
    Read(Flight<'m, K, T>),
}

/// One reader's read of something [`FetchMemory`] keeps, under way.
///
/// Dropped however the read ends — landed, failed, or its task dropped with it
/// — and the drop is what takes it off the reads under way, so a reader that
/// never lands leaves its waiters to read for themselves rather than waiting on
/// nothing.
pub(crate) struct Flight<'m, K: PartialEq, T> {
    memory: &'m FetchMemory,
    landing: Landing<T>,
    /// Which of the memory's reads under way this is one of.
    flights: fn(&mut Kept) -> &mut Flights<K, T>,
}

impl<'m, K: PartialEq, T> Flight<'m, K, T> {
    /// Starts a read for `key`, under the lock the caller found nothing kept
    /// and nothing under way in.
    pub(super) fn start(
        memory: &'m FetchMemory,
        kept: &mut Kept,
        flights: fn(&mut Kept) -> &mut Flights<K, T>,
        key: K,
    ) -> Self {
        let landing = flights(kept).start(key);
        Self {
            memory,
            landing,
            flights,
        }
    }

    /// Hands what the read came to to every reader waiting on it, and to any
    /// that joins before this is dropped.
    pub(crate) fn land(&self, outcome: T) {
        self.landing.send_replace(Some(outcome));
    }
}

impl<K: PartialEq, T> Drop for Flight<'_, K, T> {
    fn drop(&mut self) {
        let mut kept = self.memory.locked();
        (self.flights)(&mut kept).end(&self.landing);
    }
}

/// A reader's wait on another's read.
pub(crate) struct Waiter<T>(watch::Receiver<Option<T>>);

impl<T: Clone> Waiter<T> {
    /// What the read came to, or `None` where its reader was dropped before it
    /// landed anything — and the read is then the waiter's own to make.
    pub(crate) async fn landed(mut self) -> Option<T> {
        // An error is the reader gone without landing: whatever it had landed
        // would have been seen first.
        let landed = self.0.wait_for(Option::is_some).await.ok()?;
        landed.clone()
    }
}
