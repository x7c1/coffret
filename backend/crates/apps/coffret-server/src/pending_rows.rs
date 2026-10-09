use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::{Mutex, MutexGuard};

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
#[derive(Debug, Default)]
pub struct PendingRows {
    /// The turn itself. Tokio's mutex hands itself over first come, first
    /// served, which is the whole of the ordering.
    turn: Mutex<()>,
    /// How many runs are waiting for the turn, for a case to wait on rather
    /// than sleep.
    waiting: AtomicUsize,
}

/// One run's turn at the pending rows, given back when it is dropped.
pub(crate) struct Turn<'a> {
    _held: MutexGuard<'a, ()>,
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
    pub(crate) async fn take(&self) -> Turn<'_> {
        let _counted = Counted::on(&self.waiting);
        Turn {
            _held: self.turn.lock().await,
        }
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

    use super::PendingRows;

    // The turns go in the order they were asked for, and a run waiting is
    // counted as one until it has its turn.
    #[tokio::test]
    async fn turns_are_taken_in_the_order_they_were_asked_for() {
        let rows = Arc::new(PendingRows::new());
        let first = rows.take().await;
        let (order, mut taken) = mpsc::unbounded_channel();

        for run in ["second", "third"] {
            let waiting = rows.waiting();
            let (mine, order) = (Arc::clone(&rows), order.clone());
            tokio::spawn(async move {
                let _turn = mine.take().await;
                order.send(run).expect("the case is listening");
            });
            while rows.waiting() == waiting {
                tokio::task::yield_now().await;
            }
        }
        assert_eq!(rows.waiting(), 2);

        drop(first);
        assert_eq!(taken.recv().await, Some("second"));
        assert_eq!(taken.recv().await, Some("third"));
        assert_eq!(rows.waiting(), 0);
    }
}
