use std::collections::VecDeque;
use std::sync::Arc;

use coffret_device::{EntryPath, RenamePair};
use tokio::sync::watch;

use crate::activity_log::{ActivityLog, Ended};
use crate::api_error::ApiError;
use crate::reported::Reported;

use super::{MoveRun, MoveStatus};

/// The moves this server has accepted and not yet finished, and the one
/// worker carrying them out.
#[derive(Debug)]
pub struct Moves {
    /// The one place any of this is written, so that every reader sees a whole
    /// answer and a case can wait on one.
    progress: watch::Sender<Progress>,
    /// Where every move that ends is logged, shared with the rest of the
    /// server's work (see [`ActivityLog`]).
    log: Arc<ActivityLog>,
}

/// One move accepted and not yet ended: waiting its turn, or under way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingMove {
    /// Which move this is, counted from 1 by this process as moves are
    /// accepted: what the `POST` answers with, and what its log entry carries.
    pub run: u64,
    /// What it was asked to move, in the order the request named it.
    pub pairs: Vec<RenamePair>,
    /// Whether the worker has taken it up: it is the move under way, waiting
    /// for this device's turn at its pending rows or holding it. Every other
    /// pending move waits behind it in the order it was accepted.
    pub taken: bool,
}

impl PendingMove {
    /// Whether this move names any of the rows `path` names: a side of one
    /// of its pairs at `path`, under it, or above it.
    fn touches(&self, path: &EntryPath) -> bool {
        self.pairs.iter().any(|pair| {
            [&pair.from, &pair.to]
                .into_iter()
                .any(|side| side.is_under(path) || path.is_under(side))
        })
    }

    /// Whether this move names the folder `folder` or one above it: a drop
    /// there lands among the rows it moves.
    fn holds(&self, folder: &EntryPath) -> bool {
        self.pairs
            .iter()
            .any(|pair| folder.is_under(&pair.from) || folder.is_under(&pair.to))
    }
}

impl Default for Moves {
    fn default() -> Self {
        Self::new()
    }
}

impl Moves {
    /// Nothing being moved.
    pub fn new() -> Self {
        Self {
            progress: watch::channel(Progress::default()).0,
            log: Arc::new(ActivityLog::new()),
        }
    }

    /// Nothing being moved, logging what ends into `log` — the server's one log
    /// of its finished work, which [`new`](Self::new) keeps apart for a case
    /// that drives this flow alone.
    pub fn logging_to(log: Arc<ActivityLog>) -> Self {
        Self { log, ..Self::new() }
    }

    /// Every move accepted and not yet ended, in the order they were accepted:
    /// the one under way first.
    pub fn listed(&self) -> Vec<PendingMove> {
        self.progress.borrow().pending.iter().cloned().collect()
    }

    /// Waits until no move is pending and no worker is running.
    ///
    /// What a case drives the work with: accepting is synchronous, so a case
    /// that has had its `202` has already put the move on this value by the
    /// time it waits.
    pub async fn until_idle(&self) {
        let mut watched = self.progress.subscribe();
        // The sender is a field of the state this was reached through, so it
        // outlives the wait; a channel that closed anyway leaves nothing to wait
        // for.
        let _ = watched.wait_for(|progress| !progress.working).await;
    }

    /// The pairs of a pending move that names any of the rows `paths` name —
    /// a path it names, one under it, or one above it — and `None` where no
    /// pending move does.
    ///
    /// What a second move, a rename and a deletion are refused for up front,
    /// rather than queued behind a move whose rows they would be acting on
    /// before it has put them where it said.
    pub fn touched_by<'a>(
        &self,
        paths: impl IntoIterator<Item = &'a EntryPath>,
    ) -> Option<Vec<RenamePair>> {
        let progress = self.progress.borrow();
        paths.into_iter().find_map(|path| {
            progress
                .pending
                .iter()
                .find(|pending| pending.touches(path))
                .map(|pending| pending.pairs.clone())
        })
    }

    /// The pairs of a pending move that names the folder `folder` or one above
    /// it, and `None` where none does or `folder` is the Library root.
    ///
    /// What a drop onto that folder is refused for: it would land among the
    /// rows the move takes away. A drop onto a folder above a pending move's
    /// rows is not refused: what it writes lands beside them, and the move's
    /// own refusals, asked again under its turn, are what stand between a file
    /// it wrote under a moved folder and that file leaving this device
    /// (spec: LA-31).
    pub fn holding(&self, folder: Option<&EntryPath>) -> Option<Vec<RenamePair>> {
        let folder = folder?;
        let progress = self.progress.borrow();
        progress
            .pending
            .iter()
            .find(|pending| pending.holds(folder))
            .map(|pending| pending.pairs.clone())
    }

    /// Accepts a move of `pairs`, and says which move it is and whether a
    /// worker has to be started for it.
    ///
    /// Refused, with nothing accepted, where a move of any of its rows is
    /// pending already — exactly the same set pressed twice among them — as
    /// the preview says it would be.
    pub(super) fn accept(&self, pairs: Vec<RenamePair>) -> Result<(u64, bool), ApiError> {
        // Asked and accepted under one lock, so two requests touching the
        // same rows cannot both find nothing pending and both be accepted.
        let mut answer = Ok((0, false));
        self.progress.send_if_modified(|progress| {
            let touched = progress.pending.iter().find(|pending| {
                pairs
                    .iter()
                    .any(|pair| pending.touches(&pair.from) || pending.touches(&pair.to))
            });
            if let Some(pending) = touched {
                answer = Err(ApiError::move_under_way(&pending.pairs));
                return false;
            }
            progress.runs += 1;
            progress.pending.push_back(PendingMove {
                run: progress.runs,
                pairs,
                taken: false,
            });
            let start = !progress.working;
            progress.working = true;
            answer = Ok((progress.runs, start));
            true
        });
        answer
    }

    /// The next move to carry out, or nothing — in which case the worker is
    /// done and stops.
    pub(super) fn take_next(&self) -> Option<PendingMove> {
        let mut taken = None;
        self.progress
            .send_modify(|progress| match progress.pending.front_mut() {
                Some(next) => {
                    next.taken = true;
                    taken = Some(next.clone());
                }
                None => progress.working = false,
            });
        taken
    }

    /// Says what the move under way came to: it is no longer pending, and it
    /// is in the log.
    ///
    /// Both under the one lock that publishes the pending moves, so no reader
    /// sees the move over before the log has it.
    pub(super) fn finish(&self, run: &MoveRun) {
        self.progress.send_modify(|progress| {
            self.log.record(Ended::Move(run.clone()));
            progress.pending.retain(|pending| pending.run != run.run);
        });
    }

    /// Puts back what a worker that ended without taking its leave left set:
    /// every pending move is logged as stopped, since there is no worker left
    /// to carry any of them out, and none committed anything — every Entry
    /// they named is still where it was, for the person to move again.
    pub(super) fn abandon(&self) {
        self.progress.send_if_modified(|progress| {
            if !progress.working {
                return false;
            }
            progress.working = false;
            for pending in progress.pending.drain(..) {
                self.log.record(Ended::Move(MoveRun {
                    status: MoveStatus::Stopped(Reported::unfinished()),
                    ..MoveRun::starting(pending.run, pending.pairs)
                }));
            }
            true
        });
    }
}

/// Everything the server knows about moving, in one value behind one lock.
#[derive(Clone, Debug, Default)]
struct Progress {
    /// Whether a worker is running at all — from the moment it is spawned,
    /// which is before it has taken its first move. Two workers would race two
    /// records over one catalog into the commit, so what decides whether to
    /// spawn one is this.
    working: bool,
    /// How many moves have been accepted since the process started.
    runs: u64,
    /// The moves accepted and not yet ended, oldest first: the one the worker
    /// is on, and the ones it takes up after it.
    ///
    /// A queue and not "latest wins": every move here was confirmed by a
    /// person, and none of them touches the rows of another.
    pending: VecDeque<PendingMove>,
}

#[cfg(test)]
mod tests {
    use coffret_device::RenamePair;

    use super::Moves;
    use crate::entry_paths::entry_path;
    use crate::moves::{MoveRun, MoveStatus};
    use crate::reported::Reported;

    fn pair(from: &str, to: &str) -> RenamePair {
        RenamePair::new(entry_path(from), entry_path(to))
    }

    // A second move of other rows waits behind the first; one touching the
    // rows of a pending move — the same set, one under it, one above it, one
    // into where it goes — is refused and accepts nothing.
    #[test]
    fn a_second_move_waits_and_one_touching_a_pending_move_is_refused() {
        let moves = Moves::new();
        assert_eq!(
            moves.accept(vec![pair("albums/2026", "books/2026")]).ok(),
            Some((1, true))
        );
        assert_eq!(
            moves.accept(vec![pair("albums/2025", "books/2025")]).ok(),
            Some((2, false))
        );
        for touching in [
            pair("albums/2026", "books/2026"),
            pair("albums/2026/a.jpg", "a.jpg"),
            pair("albums", "kept"),
            pair("scans/b.jpg", "books/2026/b.jpg"),
        ] {
            let refused = moves.accept(vec![touching.clone()]);
            assert!(refused.is_err(), "{touching:?}");
        }
        assert_eq!(moves.listed().len(), 2);

        let first = moves.take_next().expect("the first move");
        assert_eq!(first.run, 1);
        assert!(first.taken);
        moves.finish(&MoveRun::starting(1, first.pairs));
        assert_eq!(moves.listed().len(), 1, "the first is over");
        assert_eq!(moves.take_next().map(|next| next.run), Some(2));
        moves.finish(&MoveRun::starting(
            2,
            vec![pair("albums/2025", "books/2025")],
        ));
        assert_eq!(moves.take_next(), None);
        assert!(moves.listed().is_empty());
        assert_eq!(
            moves.accept(vec![pair("albums/2026", "books/2026")]).ok(),
            Some((3, true))
        );
    }

    // A drop onto a folder a pending move names, or one under it, is refused;
    // one onto a folder above it, or the Library root, is not.
    #[test]
    fn a_drop_among_the_rows_of_a_pending_move_is_refused() {
        let moves = Moves::new();
        assert!(moves
            .accept(vec![pair("albums/2026", "books/2026")])
            .is_ok());
        for folder in ["albums/2026", "albums/2026/spring", "books/2026"] {
            assert!(
                moves.holding(Some(&entry_path(folder))).is_some(),
                "{folder}"
            );
        }
        for folder in ["albums", "books", "albums/2025"] {
            assert!(
                moves.holding(Some(&entry_path(folder))).is_none(),
                "{folder}"
            );
        }
        assert!(moves.holding(None).is_none());
    }

    // A worker that died logs every move it held as stopped, so that the page
    // that drew them at their new place puts them back.
    #[test]
    fn abandoning_the_moves_logs_each_as_stopped() {
        let log = std::sync::Arc::new(crate::ActivityLog::new());
        let moves = Moves::logging_to(std::sync::Arc::clone(&log));
        assert!(moves
            .accept(vec![pair("albums/2026", "books/2026")])
            .is_ok());
        assert!(moves
            .accept(vec![pair("albums/2025", "books/2025")])
            .is_ok());
        moves.take_next();
        moves.abandon();

        assert!(moves.listed().is_empty());
        let logged = log.entries();
        assert_eq!(logged.len(), 2);
        for entry in logged {
            let crate::Ended::Move(run) = &entry.ended else {
                panic!("a move is logged as one");
            };
            assert_eq!(run.status, MoveStatus::Stopped(Reported::unfinished()));
        }
        assert_eq!(
            moves.accept(vec![pair("albums/2026", "books/2026")]).ok(),
            Some((3, true))
        );
    }
}
