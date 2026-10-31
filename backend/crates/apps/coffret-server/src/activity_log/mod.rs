use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

mod ended;
pub use ended::Ended;
use ended::Key;

mod logged;
pub use logged::Logged;

mod queue;
pub use queue::Queue;

/// How many finished entries the log keeps before it drops the oldest.
///
/// A screenful and then some: what the log is for is reading back what
/// happened while somebody was not looking, and a person scrolling past a
/// hundred lines of it is reading history the server log holds better.
pub const KEPT: usize = 100;

/// How many entries the log holds at most, failures nobody has settled
/// included.
///
/// A failure is kept past `KEPT` because it is the one entry a person has
/// something to do about (see [`ActivityLog::record`]), but a Storage outage
/// stopping every folder a reader walks into could otherwise grow the log
/// without bound. Past this the oldest goes whatever it is: what the log holds
/// is the process's own memory, and it is bounded whatever happens.
pub const CEILING: usize = 2 * KEPT;

/// What the server keeps of the work it has finished, for as long as it runs
/// and no longer (spec: LA-12).
///
/// The work answer says what each flow is doing now and what its last run came
/// to; this is the rest — every fill, sync, freeze, deletion and move that
/// ended, every add a person stopped,
/// every folder a queue lost, every fetch a reader asked for and was refused
/// (save the refusals [`Ended::Fetch`] leaves out) — newest first, so a page
/// that was closed, reloaded, or looking elsewhere when one of them ended can
/// still read it.
///
/// Held in memory and in nothing serialized, so a server started afterwards
/// answers with an empty log: the account of what this device was doing goes
/// with the process, as the rest of what a server holds about its work does,
/// and none of it reaches a file.
///
/// Behind a mutex rather than a watch channel: nothing waits on it, and every
/// writer holds it for the length of one push.
#[derive(Debug, Default)]
pub struct ActivityLog {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// What the next entry is numbered, counted from 1 in each process.
    next: u64,
    /// The entries, newest at the front.
    entries: VecDeque<Logged>,
}

impl ActivityLog {
    /// An empty log, which is what every process starts with.
    pub fn new() -> Self {
        Self::default()
    }

    /// Logs what just ended, newest first.
    ///
    /// Whatever stood before it about the same work is settled by it, whichever
    /// way this one went: a retry that worked has answered the failure, and one
    /// that failed again is the failure now — standing beside the old one, the
    /// two would offer the same second attempt twice.
    ///
    /// Then the oldest entries past `KEPT` are dropped, except a failure
    /// nothing has settled: that one is what a person still has something to
    /// do about, and dropping it for being old would take the offer of a second
    /// attempt with it. Past `CEILING` the oldest goes whatever it is.
    pub fn record(&self, ended: Ended) {
        let at = SystemTime::now();
        let mut inner = self.lock();
        let key = ended.key();
        settle(&mut inner.entries, &key);
        inner.next += 1;
        let id = inner.next;
        inner.entries.push_front(Logged {
            id,
            at,
            ended,
            settled: false,
        });
        trim(&mut inner.entries);
    }

    /// Settles the refused fetches of `path`, now that a fetch of it was
    /// answered.
    pub fn fetched(&self, path: &str) {
        settle(&mut self.lock().entries, &Key::Fetch(path.to_owned()));
    }

    /// Everything the log holds, newest first.
    pub fn entries(&self) -> Vec<Logged> {
        self.lock().entries.iter().cloned().collect()
    }

    /// The log, whatever a writer that panicked left it as: every write is one
    /// push and one trim, so what a panic can leave is a log one entry short
    /// or one entry long, and refusing to answer for the rest of the process
    /// would be worse than either.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Marks every standing failure about `key` settled.
fn settle(entries: &mut VecDeque<Logged>, key: &Key) {
    for entry in entries.iter_mut() {
        if !entry.settled && entry.ended.failed() && entry.ended.key() == *key {
            entry.settled = true;
        }
    }
}

/// Drops the oldest entries past `KEPT` that are not standing failures, and
/// the oldest of any kind past `CEILING`.
fn trim(entries: &mut VecDeque<Logged>) {
    while entries.len() > KEPT {
        let Some(oldest) = entries.iter().rposition(|entry| !entry.standing()) else {
            break;
        };
        entries.remove(oldest);
    }
    entries.truncate(CEILING);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::delete::{DeleteRun, DeleteStatus, Target};
    use crate::entry_paths::entry_path;
    use crate::fill::{FillRun, FillStatus};
    use crate::folder::Folder;
    use crate::freeze::{FreezeRun, FreezeStatus};
    use crate::reported::Reported;

    fn folder(path: &str) -> Folder {
        Folder::named(Some(entry_path(path)))
    }

    fn fill(path: &str, status: FillStatus) -> Ended {
        let mut run = FillRun::starting(folder(path));
        run.status = status;
        Ended::Fill(run)
    }

    fn stopped() -> Reported {
        Reported::unfinished()
    }

    fn folders(log: &ActivityLog) -> Vec<String> {
        log.entries()
            .iter()
            .map(|entry| match &entry.ended {
                Ended::Fill(run) => run.folder.as_str().to_owned(),
                other => panic!("only fills are logged here: {other:?}"),
            })
            .collect()
    }

    // LA-12: what a page reads first is what ended last.
    #[test]
    fn the_newest_entry_comes_first() {
        let log = ActivityLog::new();
        log.record(fill("albums", FillStatus::Done));
        log.record(fill("books", FillStatus::Done));
        log.record(fill("scans", FillStatus::Done));

        assert_eq!(folders(&log), ["scans", "books", "albums"]);
        let ids: Vec<u64> = log.entries().iter().map(|entry| entry.id).collect();
        assert_eq!(ids, [3, 2, 1], "each entry is numbered as it is logged");
    }

    // LA-12: the log is bounded — past the cap the oldest finished entry goes.
    #[test]
    fn the_oldest_entry_is_dropped_past_the_cap() {
        let log = ActivityLog::new();
        for n in 0..=KEPT {
            log.record(fill(&format!("folder-{n}"), FillStatus::Done));
        }

        let kept = folders(&log);
        assert_eq!(kept.len(), KEPT);
        assert_eq!(
            kept.first().map(String::as_str),
            Some(format!("folder-{KEPT}").as_str())
        );
        assert!(
            !kept.contains(&"folder-0".to_owned()),
            "the oldest entry is the one dropped",
        );
    }

    // LA-12: a failure is what a person still has something to do about, so
    // it is kept past the cap however old it is — and once the same work ends
    // again, it is settled and ages out like any other entry.
    #[test]
    fn a_failure_is_kept_until_a_retry_settles_it() {
        let log = ActivityLog::new();
        log.record(fill("albums", FillStatus::Stopped(stopped())));
        for n in 0..KEPT {
            log.record(fill(&format!("folder-{n}"), FillStatus::Done));
        }

        let entries = log.entries();
        assert_eq!(entries.len(), KEPT);
        let oldest = entries.last().expect("the log holds entries");
        assert!(
            oldest.standing(),
            "the failure stands past the cap, older than anything kept",
        );
        assert!(
            !folders(&log).contains(&"folder-0".to_owned()),
            "the oldest finished entry went in its place",
        );

        // A retry of the same folder that worked settles it…
        log.record(fill("albums", FillStatus::Done));
        let entries = log.entries();
        assert!(
            entries.iter().all(|entry| !entry.standing()),
            "nothing is standing once the retry has ended",
        );
        // …and the settled failure is the oldest entry, so it goes first.
        assert_eq!(entries.len(), KEPT);
        assert!(entries.iter().all(|entry| !entry.ended.failed()));
    }

    // LA-12: a retry that failed again is the failure now — the old one is
    // settled, so one failure stands and offers one second attempt.
    #[test]
    fn a_retry_that_fails_again_takes_the_failure_over() {
        let log = ActivityLog::new();
        log.record(fill("albums", FillStatus::Stopped(stopped())));
        log.record(fill("books", FillStatus::Stopped(stopped())));
        log.record(fill("albums", FillStatus::Stopped(stopped())));

        let standing: Vec<(u64, bool)> = log
            .entries()
            .iter()
            .map(|entry| (entry.id, entry.standing()))
            .collect();
        assert_eq!(standing, [(3, true), (2, true), (1, false)]);
    }

    // LA-12: a folder a queue lost is settled by the next run on that folder,
    // and a refused fetch by the next fetch of that file answered.
    #[test]
    fn a_lost_folder_and_a_refused_fetch_are_settled_by_their_own_work() {
        let log = ActivityLog::new();
        log.record(Ended::Discarded {
            queue: Queue::Freeze,
            folder: folder("books/vol-1"),
        });
        log.record(Ended::Fetch {
            path: "albums/cover.png".to_owned(),
            refusal: stopped(),
        });
        // A fill of the book's folder is not its freeze.
        log.record(fill("books/vol-1", FillStatus::Done));
        assert_eq!(
            log.entries()
                .iter()
                .filter(|entry| entry.standing())
                .count(),
            2
        );

        let mut run = FreezeRun::starting(crate::freeze::Book::whole(folder("books/vol-1")));
        run.status = FreezeStatus::Done;
        log.record(Ended::Freeze(run));
        log.fetched("albums/cover.png");
        assert_eq!(
            log.entries()
                .iter()
                .filter(|entry| entry.standing())
                .count(),
            0
        );
    }

    // LA-12: a deletion is the same work only where it names the same things.
    #[test]
    fn a_deletion_is_settled_by_one_of_the_same_target() {
        let deletion = |path: &str, status: DeleteStatus| {
            let mut run = DeleteRun::starting(Target {
                folders: BTreeSet::from([entry_path(path)]),
                paths: BTreeSet::new(),
            });
            run.status = status;
            Ended::Delete(run)
        };
        let log = ActivityLog::new();
        log.record(deletion("albums", DeleteStatus::Stopped(stopped())));
        log.record(deletion("books", DeleteStatus::Done));
        assert!(
            log.entries()[1].standing(),
            "another target settles nothing"
        );
        log.record(deletion("albums", DeleteStatus::Done));
        assert!(log.entries().iter().all(|entry| !entry.standing()));
    }

    // LA-12: bounded whatever happens — failures nobody settles still stop at
    // the ceiling, the oldest going first.
    #[test]
    fn standing_failures_stop_at_the_ceiling() {
        let log = ActivityLog::new();
        for n in 0..=CEILING {
            log.record(fill(&format!("folder-{n}"), FillStatus::Stopped(stopped())));
        }
        let kept = folders(&log);
        assert_eq!(kept.len(), CEILING);
        assert!(!kept.contains(&"folder-0".to_owned()));
    }

    // LA-12: a log is the process's own — a new one, which is what a server
    // started afterwards holds, has nothing in it.
    #[test]
    fn a_new_log_is_empty() {
        let log = ActivityLog::new();
        log.record(fill("albums", FillStatus::Done));
        assert_eq!(log.entries().len(), 1);
        assert!(ActivityLog::new().entries().is_empty());
    }
}
