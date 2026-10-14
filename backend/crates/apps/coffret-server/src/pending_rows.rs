use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex as Slot;

use tokio::sync::{Mutex, MutexGuard};

/// One of the three flows that take turns at the pending rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnTaker {
    /// A sync, carrying the mapped folders into the Library.
    Sync,
    /// A freeze, packing one folder.
    Freeze,
    /// A deletion, taking Entries out of the Library.
    Delete,
}

impl TurnTaker {
    /// The word the work answer says the flow in.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sync => "sync",
            Self::Freeze => "freeze",
            Self::Delete => "delete",
        }
    }
}

/// This device's pending rows, as the server's own three flows take turns at
/// them.
///
/// A sync, a freeze and a deletion each own those rows for the whole of a run,
/// and the use case refuses a second owner at once rather than making it wait
/// (spec: OC-2). That refusal is the right answer to another *process* — one
/// that cannot be waited on blindly — but this server runs the three flows on
/// three workers of its own, and without this it would be that second owner
/// against itself: a file dropped while a book is packed would start a sync
/// that stops on its first line, and the person would be offered a retry for
/// something that never had a chance.
///
/// So each run takes its turn here before it asks for the Library, and holds it
/// across the whole of its use-case call. One fair lock, so the turns go in the
/// order the runs asked for them: a run asks as its worker takes it up, which
/// for a flow already running is once the run before it in the same flow has
/// finished. What the use case refuses after that is only ever another process.
///
/// It also knows whose turn it is, because a browser cannot work that out: a
/// run that has taken the turn and not yet said a step looks exactly like one
/// still waiting for it, and a screen guessing between the two can have two
/// runs each claim to be waiting for the other.
#[derive(Debug, Default)]
pub struct PendingRows {
    /// The turn itself. Tokio's mutex hands itself over first come, first
    /// served, which is the whole of the ordering.
    turn: Mutex<()>,
    /// How many runs are waiting for the turn, for a case to wait on rather
    /// than sleep.
    waiting: AtomicUsize,
    /// Which flow's run holds the turn, and `None` while nobody does.
    ///
    /// Set once the turn has been handed over and cleared as it is given back,
    /// both while the turn itself is held, so it never names a flow that is
    /// not holding it.
    holder: Slot<Option<TurnTaker>>,
}

/// One run's turn at the pending rows, given back when it is dropped.
pub(crate) struct Turn<'a> {
    holder: &'a Slot<Option<TurnTaker>>,
    _held: MutexGuard<'a, ()>,
}

impl Drop for Turn<'_> {
    // Runs before the guard is dropped, so the holder is cleared while the
    // turn is still this run's and the next taker cannot have set itself yet.
    fn drop(&mut self) {
        *held_by(self.holder) = None;
    }
}

/// The holder's slot, read through a lock a panicking writer cannot poison:
/// what it guards is one word that is never left half written.
fn held_by(slot: &Slot<Option<TurnTaker>>) -> std::sync::MutexGuard<'_, Option<TurnTaker>> {
    slot.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl PendingRows {
    /// Nobody holding the rows, and nobody waiting for them.
    pub fn new() -> Self {
        Self::default()
    }

    /// Waits for this run's turn, behind every run that asked before it.
    ///
    /// Taken before the keys and never after them: a run waiting here holds no
    /// handle on the Library, so the wait is not counted as somebody being here
    /// (spec: DK-4), and a lock that lands meanwhile stops the run once its
    /// turn comes, exactly as it stops one that never waited.
    pub(crate) async fn take(&self, taker: TurnTaker) -> Turn<'_> {
        let counted = Counted::on(&self.waiting);
        let held = self.turn.lock().await;
        *held_by(&self.holder) = Some(taker);
        drop(counted);
        Turn {
            holder: &self.holder,
            _held: held,
        }
    }

    /// Which flow's run holds the turn right now, and `None` where nobody does.
    ///
    /// What the work answer says the turn is, and what every run of the other
    /// flows still running is waiting for.
    pub fn holder(&self) -> Option<TurnTaker> {
        *held_by(&self.holder)
    }

    /// How many runs are waiting for their turn right now.
    ///
    /// What a case reads to know a run it armed is queued behind the one
    /// running, rather than guessing that the scheduler got to it.
    pub fn waiting(&self) -> usize {
        self.waiting.load(Ordering::SeqCst)
    }
}

/// One waiter, counted from when it asks to when it has its turn — or gave up
/// asking, if the future waiting was dropped.
struct Counted<'a>(&'a AtomicUsize);

impl<'a> Counted<'a> {
    fn on(waiting: &'a AtomicUsize) -> Self {
        waiting.fetch_add(1, Ordering::SeqCst);
        Self(waiting)
    }
}

impl Drop for Counted<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::mpsc;

    use super::{PendingRows, TurnTaker};

    // The turns go in the order they were asked for, a run waiting is counted
    // as one until it has its turn, and the turn names whoever holds it and
    // nobody once it has been given back.
    #[tokio::test]
    async fn turns_are_taken_in_the_order_they_were_asked_for() {
        let rows = Arc::new(PendingRows::new());
        let first = rows.take(TurnTaker::Delete).await;
        assert_eq!(rows.holder(), Some(TurnTaker::Delete));
        let (order, mut taken) = mpsc::unbounded_channel();

        for (run, taker) in [("second", TurnTaker::Sync), ("third", TurnTaker::Freeze)] {
            let waiting = rows.waiting();
            let (mine, order) = (Arc::clone(&rows), order.clone());
            tokio::spawn(async move {
                let _turn = mine.take(taker).await;
                order
                    .send((run, mine.holder()))
                    .expect("the case is listening");
            });
            while rows.waiting() == waiting {
                tokio::task::yield_now().await;
            }
        }
        assert_eq!(rows.waiting(), 2);
        assert_eq!(rows.holder(), Some(TurnTaker::Delete));

        drop(first);
        assert_eq!(taken.recv().await, Some(("second", Some(TurnTaker::Sync))));
        assert_eq!(taken.recv().await, Some(("third", Some(TurnTaker::Freeze))));
        assert_eq!(rows.waiting(), 0);
        while rows.holder().is_some() {
            tokio::task::yield_now().await;
        }
    }
}
