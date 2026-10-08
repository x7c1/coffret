use std::sync::Arc;

use crate::state::ServerState;

use super::run;
use super::Freezes;

/// The one worker, packing folders until none is waiting.
pub(super) async fn work(state: Arc<ServerState>) {
    // Whichever way this ends, what says a worker is running is put back. It
    // ends by finding nothing waiting, which puts it back already — and it ends
    // by panicking, which without this would leave the flag set with nothing
    // behind it: no drop would start another worker for the rest of the process,
    // and the run on record would go on saying `freezing` to a browser that polls it.
    let _leaving = Leaving(&state.freezes);
    while let Some(book) = state.freezes.take_next() {
        run::freeze(&state, book).await;
    }
}

/// The worker's leaving, whether it meant to or not.
///
/// A guard rather than a line at the end of [`work`], because the end of `work`
/// is the one ending that needs no putting back: it is the other one — the job
/// panicking, which unwinds past every line there is — that this exists for.
///
/// So it puts back only while unwinding from a panic. By the time an ordinary
/// ending drops this, `take_next` has already said the worker is gone, and a
/// book dropped in between may have started the next one: putting back then
/// would take that new worker's run for this one's leftovers — discarding the
/// folder it was armed for and reporting it stopped. The worker is spawned and
/// never aborted, so a panic is the only ending that does not go through
/// `take_next`.
struct Leaving<'a>(&'a Freezes);

impl Drop for Leaving<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.abandon();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use super::{Freezes, Leaving};
    use crate::entry_paths::entry_path;
    use crate::folder::Folder;
    use crate::freeze::{Book, FreezeStatus};
    use crate::reported::Reported;

    fn book() -> Book {
        Book::whole(Folder::named(Some(entry_path("books/vol-1"))))
    }

    fn another_book() -> Book {
        Book::whole(Folder::named(Some(entry_path("books/vol-2"))))
    }

    // A worker that has found nothing waiting has left, and a book dropped
    // before its guard goes starts the next worker: the old guard going
    // afterwards is not that new run dying, and must leave its folder waiting.
    #[test]
    fn a_worker_leaving_the_ordinary_way_leaves_the_next_run_alone() {
        let freezes = Freezes::new();
        assert!(freezes.arm(book()));
        let leaving = Leaving(&freezes);
        assert_eq!(freezes.take_next(), Some(book()));
        assert_eq!(freezes.take_next(), None);

        assert!(
            freezes.arm(another_book()),
            "nothing is running, so the arming starts a worker of its own",
        );
        drop(leaving);

        let latest = freezes.reported().expect("an armed freeze is on record");
        assert!(
            latest.discarded.is_empty(),
            "and nothing is thrown away by a worker that did not die",
        );
        assert_ne!(
            latest.on_record.status,
            FreezeStatus::Stopped(Reported::unfinished()),
            "nor is the new run reported stopped",
        );
        assert_eq!(
            freezes.take_next(),
            Some(another_book()),
            "the new arming is still waiting for the worker it started",
        );
    }

    // The ending the guard exists for: a panic unwinding past the worker puts
    // back what it left set.
    #[test]
    fn a_worker_that_panics_is_put_back() {
        let freezes = Freezes::new();
        assert!(freezes.arm(book()));

        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let _leaving = Leaving(&freezes);
            assert_eq!(freezes.take_next(), Some(book()));
            panic!("the job panics mid-freeze");
        }));
        assert!(unwound.is_err());

        let latest = freezes.reported().expect("an armed freeze is on record");
        assert_eq!(
            latest.on_record.status,
            FreezeStatus::Stopped(Reported::unfinished()),
        );
        assert!(
            freezes.arm(book()),
            "and the next arming starts a worker again",
        );
    }
}
