use std::sync::Arc;

use coffret_device::{ShortKeyring, UnheldParcel};
use tokio::sync::watch;

use crate::activity_log::{ActivityLog, Ended, Queue};
use crate::folder::Folder;
use crate::latest::Latest;

use super::progress::Progress;
use super::{EndedFills, FillRun, FillStatus};

/// What the server is bringing over, and what it has brought over last.
#[derive(Debug)]
pub struct Fills {
    /// The one place any of this is written, so that every reader sees a whole
    /// answer and a case can wait on one.
    progress: watch::Sender<Progress>,
    /// The folders a run has ended on, noted where each run ends.
    ended: EndedFills,
    /// Where every run that ends is logged, shared with the rest of the
    /// server's work (see [`ActivityLog`]).
    log: Arc<ActivityLog>,
}

impl Default for Fills {
    fn default() -> Self {
        Self::new()
    }
}

impl Fills {
    /// Nothing being filled.
    pub fn new() -> Self {
        Self {
            progress: watch::channel(Progress::default()).0,
            ended: EndedFills::new(),
            log: Arc::new(ActivityLog::new()),
        }
    }

    /// Nothing being filled, logging what ends into `log` — the server's one
    /// log of its finished work, which [`new`](Self::new) keeps apart for a
    /// case that drives this flow alone.
    pub fn logging_to(log: Arc<ActivityLog>) -> Self {
        Self { log, ..Self::new() }
    }

    /// The latest fill, the runs it took the record from, the folders waiting
    /// behind it and the ones its queue lost — and `None` where no fill has run.
    ///
    /// A finished fill is kept rather than cleared, because the two things a
    /// browser most needs from this are things a finished fill says: which
    /// Entries were declined, and whether Storage stopped it — the state the
    /// retry is offered from. The three lists are read beside it rather than out
    /// of it, because none of them is any one run's property: the folder on
    /// record is the one being brought over, the queue is what nothing has been
    /// said about yet, what a worker that died threw away outlives the run it was
    /// queued behind, and a run Storage stopped goes on being a folder somebody
    /// asked for and did not get after the next one has taken the record from it.
    ///
    /// All four under one borrow, which is the whole reason they live in one
    /// value. They change together: a worker that dies marks its run stopped and
    /// moves the folders behind it onto the discarded list in the same stroke, a
    /// folder taken off the queue becomes the run on record and puts the stopped
    /// one it replaced onto the displaced list in another, and an arming that
    /// finds nothing running announces the run and queues the folder in a third.
    /// Read one at a time, an answer could carry half of any of them — a run
    /// still saying `filling` beside the folders that very ending threw away, or
    /// a run still saying `done` beside a queue already taken up, which is a
    /// browser told there is nothing left to follow at the moment the next run
    /// starts.
    pub fn reported(&self) -> Option<Latest<FillRun>> {
        let progress = self.progress.borrow();
        let run = progress.on_record.clone()?;
        Some(Latest {
            on_record: run,
            displaced: progress.displaced().to_vec(),
            waiting: progress.waiting(),
            discarded: progress.discarded().to_vec(),
        })
    }

    /// Waits until nothing is being filled and nothing is armed.
    ///
    /// What a case drives the work with. Arming is synchronous, so a case that
    /// has asked for a file has already put the fill on this value by the time
    /// it awaits here — which is what lets the cases assert on a finished fill
    /// without sleeping on one.
    pub async fn until_idle(&self) {
        let mut watched = self.progress.subscribe();
        // The sender is a field of the state this was reached through, so it
        // outlives the wait; a channel that closed anyway leaves nothing to
        // wait for.
        let _ = watched.wait_for(Progress::idle).await;
    }

    /// Makes `folder` what is filled next, and says whether a worker has to be
    /// started for it. See [`Progress::arm`].
    ///
    /// `heard` is what the fetch arming it found of the committed Keyring, and
    /// `unheld` the kept parcels it found not held, which the run that takes
    /// the arming up reports as its own (see [`Progress::hear`] and
    /// [`Progress::hear_unheld`]). In the same stroke as the arming, so that no
    /// run is published in between that could take them for a folder they are
    /// not about.
    pub(super) fn arm(
        &self,
        folder: Folder,
        heard: Option<ShortKeyring>,
        unheld: Vec<UnheldParcel>,
    ) -> bool {
        let mut start = false;
        self.progress.send_modify(|progress| {
            start = progress.arm(folder);
            if let Some(found) = heard {
                progress.hear(found);
            }
            progress.hear_unheld(unheld);
        });
        start
    }

    /// Takes in the kept parcels the rest of a reader's fetch found not held,
    /// after the reader was answered and the fill it armed was already under
    /// way (see [`Progress::hear_unheld`]).
    ///
    /// Nothing is armed by it: the fetch that found them armed its fill when it
    /// answered. The next run published takes them in — which in the ordinary
    /// case is that fill, whose next fetch out of the same Container waited for
    /// this one to end.
    pub(crate) fn hear_unheld(&self, unheld: Vec<UnheldParcel>) {
        if unheld.is_empty() {
            return;
        }
        self.progress
            .send_modify(|progress| progress.hear_unheld(unheld));
    }

    /// Takes in what a read ahead's fetch found of the committed Keyring and
    /// the kept parcels it found not held, arming nothing (spec: LA-33).
    ///
    /// A read ahead arms no fill, so there is no arming for these to go with;
    /// they go where [`hear_unheld`](Self::hear_unheld)'s go, to the next run
    /// published — the fill of the folder being read, in the ordinary case, or
    /// the one the reader arms on stepping into the folder read ahead of. A
    /// run's line is where a person who only opens files hears of either, and
    /// a finding about the Keyring is about the whole Library rather than one
    /// folder of it, as a parcel is part of a Container rather than of a
    /// folder.
    pub(crate) fn hear(&self, heard: Option<ShortKeyring>, unheld: Vec<UnheldParcel>) {
        if heard.is_none() && unheld.is_empty() {
            return;
        }
        self.progress.send_modify(|progress| {
            if let Some(found) = heard {
                progress.hear(found);
            }
            progress.hear_unheld(unheld);
        });
    }

    /// Puts `folder` at the back of the queue, and says whether a worker has to
    /// be started for it. See [`Progress::queue`].
    pub(super) fn queue(&self, folder: Folder) -> bool {
        let mut start = false;
        self.progress
            .send_modify(|progress| start = progress.queue(folder));
        start
    }

    /// The next folder to fill, or nothing — in which case the worker is done
    /// and stops. See [`Progress::take_next`].
    pub(super) fn take_next(&self) -> Option<Folder> {
        let mut taken = None;
        self.progress
            .send_modify(|progress| taken = progress.take_next());
        taken
    }

    /// Puts back what a worker that ended without taking its leave left set.
    ///
    /// For a worker that panicked, and for no other: the ordinary ending clears
    /// all of this itself, and by the time it is over another worker may own
    /// the flag. Notified only where there was something to put back, which is
    /// why this goes through [`send_if_modified`](watch::Sender::send_if_modified).
    ///
    /// The run it stops is noted as ended and logged, as one that stopped
    /// through [`publish`](Self::publish) is, and every folder it throws away
    /// is logged as lost.
    pub(super) fn abandon(&self) {
        self.progress.send_if_modified(|progress| {
            let filling = progress.is_filling();
            let lost_before = progress.discarded().len();
            let modified = progress.abandon();
            if let Some(run) = &progress.on_record {
                self.ended.note(run);
                if modified && filling {
                    self.log.record(Ended::Fill(run.clone()));
                }
            }
            for folder in &progress.discarded()[lost_before..] {
                self.log.record(Ended::Discarded {
                    queue: Queue::Fill,
                    folder: folder.clone(),
                });
            }
            modified
        });
    }

    /// Whether a run has ended on `folder` in this process, done or stopped
    /// (see [`EndedFills`]).
    pub(crate) fn has_ended(&self, folder: &Folder) -> bool {
        self.ended.has_ended(folder)
    }

    /// Whether another folder has been armed under the fill in progress.
    pub(super) fn superseded(&self) -> bool {
        self.progress.borrow().superseded()
    }

    /// Says where the fill in progress has got to.
    ///
    /// The run number is stamped on here rather than carried by the caller: the
    /// value a run builds is its own account of one folder, and which run of the
    /// flow that is is the queue's to say.
    ///
    /// What the fetch that armed this run found of the committed Keyring is
    /// taken into the run here too, under the same lock the arming put it there
    /// under, so the run keeps it for every later publish of its own (see
    /// [`Progress::hand_over`]).
    ///
    /// A run that is done or stopped is noted as ended on its folder here, in
    /// the same stroke that publishes it; a run whose worker died is noted by
    /// [`abandon`](Self::abandon) instead.
    pub(super) fn publish(&self, run: &mut FillRun) {
        self.progress.send_modify(|progress| {
            if let Some(found) = progress.hand_over() {
                run.read_keyring(found);
            }
            run.read_unheld(progress.hand_over_unheld());
            self.ended.note(run);
            let published = FillRun {
                run: progress.run(),
                ..run.clone()
            };
            // Logged under the same lock that publishes it, so no reader sees
            // the run over before the log has it. A run superseded by another
            // folder did not end, and is not logged.
            if matches!(published.status, FillStatus::Done | FillStatus::Stopped(_)) {
                self.log.record(Ended::Fill(published.clone()));
            }
            progress.on_record = Some(published);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::Fills;
    use crate::fill::{FillRun, FillStatus};
    use crate::folder::Folder;
    use crate::reported::Reported;

    use crate::entry_paths::entry_path;

    fn albums() -> Folder {
        Folder::named(Some(entry_path("albums")))
    }

    fn photos() -> Folder {
        Folder::named(Some(entry_path("photos")))
    }

    /// Publishes a run on `folder` that came to `status`.
    fn published(fills: &Fills, folder: Folder, status: FillStatus) {
        let mut run = FillRun::starting(folder);
        run.status = status;
        fills.publish(&mut run);
    }

    // A run still on its folder has not ended, and neither has one a fetch
    // into another folder took the worker from: both leave the folder for the
    // next page opened in it to arm.
    #[test]
    fn a_run_filling_or_superseded_is_not_ended() {
        let fills = Fills::new();
        published(&fills, albums(), FillStatus::Filling);
        assert!(!fills.has_ended(&albums()), "still filling");

        published(&fills, albums(), FillStatus::Superseded);
        assert!(!fills.has_ended(&albums()), "superseded, not ended");
    }

    // A run that is done is noted as it publishes its ending, and stays noted
    // after another run takes the record from it.
    #[test]
    fn a_done_run_is_ended_and_stays_so() {
        let fills = Fills::new();
        published(&fills, albums(), FillStatus::Done);
        assert!(fills.has_ended(&albums()));

        published(&fills, photos(), FillStatus::Filling);
        assert!(fills.has_ended(&albums()));
        assert!(!fills.has_ended(&photos()));
    }

    // A run the worker abandoned is reported stopped, which is one way a run
    // ends, though it never publishes that ending itself.
    #[test]
    fn an_abandoned_run_is_ended() {
        let fills = Fills::new();
        assert!(fills.arm(albums(), None, Vec::new()));
        assert_eq!(fills.take_next(), Some(albums()));
        published(&fills, albums(), FillStatus::Filling);

        fills.abandon();
        assert!(fills.has_ended(&albums()));
    }

    // The half of a worker's leaving that `Progress` cannot state: putting the
    // state back is of no use to anyone unless the change is sent. What waits on
    // it is a case awaiting `until_idle` and, through the work route, a
    // browser polling a count — and `send_if_modified` sends nothing at all
    // where the closure reports nothing changed, so a fill abandoned without a
    // notification is exactly the wait that never ends.
    #[test]
    fn abandoning_a_fill_tells_whoever_is_waiting_on_it() {
        let fills = Fills::new();
        assert!(fills.arm(albums(), None, Vec::new()));
        assert_eq!(fills.take_next(), Some(albums()));

        let mut watched = fills.progress.subscribe();
        drop(watched.borrow_and_update());
        fills.abandon();

        assert!(
            watched.has_changed().expect("the sender outlives the case"),
            "a wait for the fill to finish is ended by this and by nothing else",
        );
        let latest = fills
            .reported()
            .expect("a fill that was armed is on record");
        assert_eq!(
            latest.on_record.status,
            FillStatus::Stopped(Reported::unfinished()),
            "the browser is told what became of it, and is offered the retry",
        );
        assert!(
            latest.waiting.is_empty(),
            "and nothing is named as still to come, there being no worker left \
             to take it",
        );
    }
}
