//! Saying what a long run is doing while it does it.
//!
//! A `sync` or a `fetch` over a folder of any size spends minutes inside one
//! call, and a terminal that has printed nothing looks the same whether the
//! transfer is working or has stopped. The consent step already answers that
//! for the wait it owns — it prints a line saying what is being waited for —
//! and this is the same answer for the part that takes the longest.
//!
//! # Where it goes, and why not standard output
//!
//! Standard error, like the consent line and like the log file's name. Standard
//! output carries what was asked for — the summary, the findings, a Recovery
//! Code — and a pipe reading it must find those and nothing else. A progress
//! line is neither an answer nor a failure; it is the run saying it is alive,
//! and that belongs where the other such lines already are.
//!
//! # Two renderings, chosen by what is on the other end
//!
//! A terminal gets one line, rewritten in place and erased when the run ends
//! *successfully*, because a line per file would bury the summary the run
//! finishes with. A run that ends by failing keeps its line and has a newline
//! put after it, so that how far it got is still on screen above the error:
//! that is the run where the position is worth most, since an interrupted sync
//! resumes from the spool and the pending rows it left. What is not a terminal
//! — a pipe, a file, a CI log — gets plain lines and only at the points worth
//! keeping: each phase's first and last step, and each tenth of the way
//! between. A carriage return is not a character a log can hold, and a log that
//! held one per file would be unreadable in the other direction.
//!
//! A phase that cannot say how much work it holds — the catalog catch-up, the
//! settling of what an interrupted run left, the scan — is rendered as what it
//! is doing and no numbers. Those are the phases a person waits through with
//! nothing else on screen, and a run that named only the phases it could count
//! would still say nothing through the longest silence it has.
//!
//! Nothing here hides the cursor or moves it anywhere but to the start of its
//! own line, so a run that is interrupted mid-line leaves a terminal that needs
//! nothing done to it.
//!
//! # Who writes, and why not the run
//!
//! The run never does. A step is reported from inside the flow, on the
//! runtime's own thread, and the contract it reports through says a report
//! must not hold the run up; a write to standard error is exactly what can —
//! a pipe whose reader has stopped reading, a terminal on the far side of a
//! slow ssh link. So [`Progress::step`] only records the step, and a thread of
//! this value's own takes what was recorded and writes it. However long a
//! write takes, what it costs the run is a lock held for as long as it takes to
//! put one step down.
//!
//! What that thread takes differs with the rendering. A terminal is only ever
//! showing one line, so the drawing side takes the latest step at an interval
//! and the ones in between were never going to stay on screen. A log keeps
//! every line it is given, so which steps are kept is decided as they are
//! reported, and the drawing side writes each of them in order.

use std::io::{stderr, IsTerminal, Write};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use coffret_device::{Phase, Progress, Step};
use tracing::warn;

/// How often a terminal's line is rewritten at most.
///
/// Often enough that a count reads as moving, and seldom enough that a run
/// stepping through thousands of small files does not spend a write per file
/// on a line nobody could read at that speed.
const INTERVAL: Duration = Duration::from_millis(100);

/// How long the end of a run waits for the drawing side to write its last.
///
/// The summary must not be printed over a progress line, so the end of a run
/// waits for the line to be taken back or closed — but not for ever: a stderr
/// that will not take a write is no reason for a run that has finished to
/// never say so.
const LAST_WORD: Duration = Duration::from_secs(2);

/// Which command's work the lines count.
///
/// Every phase but one is counted in Containers whatever is running. The
/// packing phase is the exception: it means the same thing in both flows that
/// have one — turning local files into Containers on this device — and cuts its
/// work differently, one file at a time for a sync (spec: PK-15) and one Pack
/// at a time for a freeze (spec: PK-3). The flow below the shell has no
/// business knowing which word a person reads, and the shell knows exactly
/// which command it is running, so the word is chosen here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Units {
    /// A sync, which packs one file at a time.
    Syncing,
    /// A freeze, which packs one Pack at a time.
    Freezing,
    /// A fetch, which packs nothing at all.
    Fetching,
}

impl Units {
    /// The word a person reads for one unit of the packing phase.
    fn packed(self) -> &'static str {
        match self {
            Self::Syncing => "files",
            Self::Freezing => "packs",
            // Never reached: a fetch reports no packing phase, and a word it
            // could not mislead anybody with is better than a panic in a
            // progress line.
            Self::Fetching => "containers",
        }
    }
}

/// How a run ended, which decides what happens to a line still standing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// The run finished and its summary follows: the line is taken back.
    Finished,
    /// The run is being left without [`Reporting::finish`], which is the run
    /// that failed: the line is closed and kept.
    Abandoned,
}

/// What the run has reported and the drawing side has not written yet.
#[derive(Default)]
struct Recorded {
    /// The steps to write, in order. At most one for a terminal — the latest —
    /// and every step worth keeping for a log.
    steps: Vec<Step>,
    /// The last step reported, for deciding whether the next one is worth a
    /// line of its own where every line is kept.
    reported: Option<Step>,
    /// How the run ended, once it has.
    ending: Option<Ending>,
}

impl Recorded {
    /// Whether the drawing side has nothing to do.
    fn is_idle(&self) -> bool {
        self.steps.is_empty() && self.ending.is_none()
    }
}

/// The cell the run records into and the drawing side takes from.
#[derive(Default)]
struct Shared {
    recorded: Mutex<Recorded>,
    /// Rung whenever something is recorded.
    recorded_something: Condvar,
}

impl Shared {
    /// The lock, or the guard of a thread that panicked holding it.
    ///
    /// A poisoned lock means something else has already failed; losing the
    /// progress line as well would replace that failure's report with this
    /// one's.
    fn lock(&self) -> MutexGuard<'_, Recorded> {
        self.recorded
            .lock()
            .unwrap_or_else(|held| held.into_inner())
    }
}

/// The drawing side, while it is running.
struct Drawer {
    thread: JoinHandle<()>,
    /// Signalled when the drawing side has written its last, so that the end
    /// of a run can wait for it with a limit — which joining the thread cannot.
    done: Receiver<()>,
}

/// A run's progress, on the terminal or in a log.
pub struct Reporting {
    /// Whether the other end can take a line back.
    terminal: bool,
    shared: Arc<Shared>,
    /// Taken by whichever of [`finish`](Self::finish) and `Drop` comes first.
    drawer: Mutex<Option<Drawer>>,
}

impl Reporting {
    /// Reports to standard error, rendered for whatever is on the other end.
    pub fn to_stderr(units: Units) -> Self {
        Self::new(units, stderr().is_terminal(), Box::new(stderr()))
    }

    /// The same, with both decisions made by the caller.
    fn new(units: Units, terminal: bool, out: Box<dyn Write + Send>) -> Self {
        let shared = Arc::new(Shared::default());
        let (finished, done) = mpsc::channel();
        let drawing = Drawing {
            units,
            terminal,
            out,
            standing: false,
        };
        let taking = Arc::clone(&shared);
        let drawer = thread::Builder::new()
            .name("progress".to_owned())
            .spawn(move || {
                drawing.run(&taking);
                // Nobody waiting any more is the end of a run that gave up on
                // this, and there is nothing to tell it.
                let _ = finished.send(());
            })
            .map(|thread| Drawer { thread, done })
            // A run is not stopped for want of a progress line; it runs
            // without one, and the log says why nothing was drawn.
            .inspect_err(|error| warn!(%error, "the progress line could not be started"))
            .ok();
        Self {
            terminal,
            shared,
            drawer: Mutex::new(drawer),
        }
    }

    /// Takes back the line standing on the terminal, if one is.
    ///
    /// Called when the run has finished and before anything else is printed, so
    /// that the summary is not written over a progress line. Erasing is right
    /// here and only here: what follows is the summary, which says everything
    /// the progress line was standing in for.
    pub fn finish(&self) {
        self.end(Ending::Finished);
    }

    /// Tells the drawing side how the run ended, and waits a while for it to
    /// say its last.
    ///
    /// Whichever ending comes first is the one written: `Drop` after `finish`
    /// is the ordinary end of a run that succeeded, and must not close a line
    /// that has already been taken back.
    fn end(&self, ending: Ending) {
        let Some(drawer) = self
            .drawer
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .take()
        else {
            return;
        };
        {
            let mut recorded = self.shared.lock();
            recorded.ending.get_or_insert(ending);
        }
        self.shared.recorded_something.notify_all();
        if drawer.done.recv_timeout(LAST_WORD).is_ok() {
            let _ = drawer.thread.join();
        }
        // Otherwise the write in hand has not come back, and the thread is left
        // to finish it or not: what follows the run is its summary or its
        // error, and neither waits on a stderr that will not take a line.
    }
}

impl Progress for Reporting {
    fn step(&self, step: Step) {
        // A phase with nothing in it is not worth a line: the run passed
        // through it, and saying "0/0" would be one more thing to read. A
        // phase that has begun and cannot say how much it holds is a different
        // state and does get one — that is the silence this exists for.
        if step.total == Some(0) {
            return;
        }
        let mut recorded = self.shared.lock();
        // The bytes an upload reports part way are the browser's: a line here
        // counts Containers, and a step that moved only its bytes would be the
        // same line again — one more of it in every log.
        let counted = Step {
            bytes: None,
            ..step
        };
        if recorded.reported == Some(counted) {
            return;
        }
        let reported = recorded.reported.replace(counted);
        match self.terminal {
            // Only the latest is ever on screen, so it replaces whatever the
            // drawing side has not got to yet.
            true => recorded.steps = vec![step],
            false if worth_keeping(reported, step) => recorded.steps.push(step),
            false => return,
        }
        drop(recorded);
        self.shared.recorded_something.notify_all();
    }
}

impl Drop for Reporting {
    /// Closes a line still standing, rather than taking it back.
    ///
    /// A line is still standing here only on the path [`finish`](Self::finish)
    /// was never reached on, which is the run that failed: the commands call
    /// `finish` and then print their summary, and the one thing that skips it
    /// is the `?` on the way to the error. What a person reads then is the
    /// error, and the line above it is how far the run got — "uploading
    /// 873/2000 containers" is what says an interrupted sync left a spool and
    /// pending rows at 873 of 2000, and it is the one thing an error about the
    /// 874th cannot say. Erasing it would take that away exactly where it is
    /// worth most, so the line is ended instead and the error prints beneath
    /// it.
    fn drop(&mut self) {
        self.end(Ending::Abandoned);
    }
}

/// The drawing side: where the lines are written, and what is on screen.
///
/// Owned by the drawing thread alone, so nothing the run holds is ever held
/// across a write.
struct Drawing {
    units: Units,
    /// Whether the other end can take a line back.
    terminal: bool,
    /// Where the lines go.
    out: Box<dyn Write + Send>,
    /// Whether a line is standing on the terminal, waiting to be taken back.
    standing: bool,
}

impl Drawing {
    /// Writes what the run records until the run ends, and then ends the line
    /// the way the ending says.
    fn run(mut self, shared: &Shared) {
        loop {
            let (steps, ending) = {
                let mut recorded = shared.lock();
                while recorded.is_idle() {
                    recorded = shared
                        .recorded_something
                        .wait(recorded)
                        .unwrap_or_else(|held| held.into_inner());
                }
                (std::mem::take(&mut recorded.steps), recorded.ending)
            };
            for step in steps {
                self.draw(step);
            }
            if let Some(ending) = ending {
                self.end(ending);
                return;
            }
            if self.terminal {
                // The interval, cut short by the end of the run so that the
                // summary is not kept waiting for it.
                let recorded = shared.lock();
                let _ =
                    shared
                        .recorded_something
                        .wait_timeout_while(recorded, INTERVAL, |recorded| {
                            recorded.ending.is_none()
                        });
            }
        }
    }

    /// Writes the line one step reads as.
    fn draw(&mut self, step: Step) {
        let line = line(self.units, step);
        let written = match self.terminal {
            true => {
                self.standing = true;
                self.out.write_all(format!("\r{line}\x1b[K").as_bytes())
            }
            false => self.out.write_all(format!("{line}\n").as_bytes()),
        };
        // A progress line that will not go out is not a reason to stop a
        // transfer, and the run has its own way of reporting what it did.
        let _ = written.and_then(|()| self.out.flush());
    }

    /// Takes back or closes the line standing on the terminal, if one is.
    fn end(&mut self, ending: Ending) {
        if !self.standing {
            return;
        }
        self.standing = false;
        let ended: &[u8] = match ending {
            // Back to the start of the line and clear to its end: the cursor
            // ends where it began, and nothing of the progress is left behind.
            Ending::Finished => b"\r\x1b[K",
            // Ended rather than erased, so the error prints beneath it.
            Ending::Abandoned => b"\n",
        };
        let _ = self.out.write_all(ended).and_then(|()| self.out.flush());
    }
}

/// The line one step reads as.
///
/// A phase that can count its work reads as what it is doing, how far
/// through it is, and the unit it counts in. One that cannot — a catch-up
/// that learns what it has to replay by replaying it, a scan that is
/// itself the count — reads as what it is doing and stops there: a number
/// it does not have would have to be invented, and the phase's name alone
/// already answers the question the silence raised.
fn line(units: Units, step: Step) -> String {
    let (doing, unit) = match step.phase {
        Phase::CatchingUp => ("catching up with the Library", None),
        Phase::Settling => ("settling what an interrupted run left", None),
        Phase::Scanning => ("scanning the mapped folders", None),
        Phase::Packing => ("packing", Some(units.packed())),
        Phase::Uploading => ("uploading", Some("containers")),
        Phase::Committing => ("committing", Some("objects")),
        Phase::Fetching => ("fetching", Some("containers")),
    };
    match (step.total, unit) {
        (Some(total), Some(unit)) => format!("{doing} {}/{total} {unit}", step.done),
        (Some(total), None) => format!("{doing} {}/{total}", step.done),
        (None, _) => doing.to_owned(),
    }
}

/// Whether a step is worth a line of its own where every line is kept.
///
/// The first and the last step of a phase always are: one says what the run is
/// about to do and the other says it finished doing it. Between them a tenth of
/// the way is enough to show a long run moving without writing a line per file.
///
/// A phase that cannot count its work says one thing — that it has begun — so
/// it is kept the first time and never again: a log holding the same line twice
/// would say the run had gone round rather than that it is still in there.
fn worth_keeping(shown: Option<Step>, step: Step) -> bool {
    let Some(total) = step.total else {
        return match shown {
            Some(shown) => shown.phase != step.phase,
            None => true,
        };
    };
    if step.done == 0 || step.done == total {
        return true;
    }
    let Some(shown) = shown else {
        return true;
    };
    if shown.phase != step.phase {
        return true;
    }
    tenth(shown) != tenth(step)
}

/// Which tenth of its phase a step stands in, where it has one to stand in.
fn tenth(step: Step) -> usize {
    match step.total {
        None | Some(0) => 0,
        Some(total) => step.done * 10 / total,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Sender;

    use coffret_device::ByteCount;

    use super::*;

    /// A sink a case can read back what was written to it.
    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Sink {
        /// Everything written so far, as text.
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().expect("no case poisons this").clone())
                .expect("everything written here is text")
        }
    }

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("no case poisons this").extend(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A sink whose first write never comes back, like a pipe nobody reads.
    ///
    /// It says when it has been entered, so a case knows the drawing side is
    /// stuck in it before asking anything of the run.
    struct NeverReturns(Mutex<Sender<()>>);

    impl Write for NeverReturns {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            let _ = self.0.lock().expect("no case poisons this").send(());
            loop {
                thread::park();
            }
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A reporting that writes into `sink`, rendered for a terminal or not.
    fn reporting(units: Units, terminal: bool, sink: &Sink) -> Reporting {
        Reporting::new(units, terminal, Box::new(sink.clone()))
    }

    // The whole point of the callback being a trait: what a run says can be
    // read without a terminal to read it on, which is also how the case below
    // asserts on the rendering a terminal would get.
    //
    // Which of the steps in between reach the screen is the drawing side's to
    // decide — it takes the latest at an interval — so what is asserted is the
    // shape of every rewrite and the one the line ends on before it is taken
    // back.
    #[test]
    fn a_terminal_gets_one_line_rewritten_in_place_and_taken_back() {
        let sink = Sink::default();
        let reporting = reporting(Units::Fetching, true, &sink);

        reporting.step(Step::new(Phase::Fetching, 0, 2));
        reporting.step(Step::new(Phase::Fetching, 1, 2));
        reporting.step(Step::new(Phase::Fetching, 2, 2));
        reporting.finish();

        let written = sink.text();
        let drawn = written.strip_suffix("\r\x1b[K").unwrap_or_else(|| {
            panic!("the line must be taken back before the summary: {written:?}")
        });
        assert!(
            drawn.ends_with("\rfetching 2/2 containers\x1b[K"),
            "the line stands at the latest step until it is taken back: {written:?}",
        );
        for rewrite in drawn.split_terminator("\x1b[K") {
            assert!(
                [
                    "\rfetching 0/2 containers",
                    "\rfetching 1/2 containers",
                    "\rfetching 2/2 containers",
                ]
                .contains(&rewrite),
                "every rewrite goes back to the start of the one line: {written:?}",
            );
        }
    }

    // What this is for. The run reports from the runtime's own thread, and a
    // stderr that will not take a write — a pipe nobody is reading, a terminal
    // over a slow link — must not hold the transfer up with it: the step is
    // recorded and the run goes on, whatever the drawing side is stuck in.
    #[test]
    fn a_step_returns_while_the_line_cannot_be_written() {
        let (entered, stuck) = mpsc::channel();
        let reporting = Arc::new(Reporting::new(
            Units::Syncing,
            true,
            Box::new(NeverReturns(Mutex::new(entered))),
        ));

        reporting.step(Step::new(Phase::Uploading, 0, 3));
        stuck
            .recv_timeout(Duration::from_secs(5))
            .expect("the drawing side takes the first step and is stuck writing it");

        let (returned, reported) = mpsc::channel();
        let running = Arc::clone(&reporting);
        thread::spawn(move || {
            for done in 1..=3 {
                running.step(Step::new(Phase::Uploading, done, 3));
            }
            let _ = returned.send(());
        });
        assert!(
            reported.recv_timeout(Duration::from_secs(5)).is_ok(),
            "a step must return while the write before it has not",
        );

        // Dropping it would wait out `LAST_WORD` for a write that never comes
        // back, which is what `end` is bounded for and not what this case is
        // about.
        std::mem::forget(reporting);
    }

    // What a pipe, a file, or a CI log gets: no control character anywhere, and
    // few enough lines that the summary after them is still findable.
    #[test]
    fn what_is_not_a_terminal_gets_plain_lines_and_no_control_characters() {
        let sink = Sink::default();
        let reporting = reporting(Units::Freezing, false, &sink);

        for done in 0..=20 {
            reporting.step(Step::new(Phase::Packing, done, 20));
        }
        reporting.finish();

        let written = sink.text();
        assert!(
            !written.contains('\r') && !written.contains('\x1b'),
            "a log must hold what was written: {written:?}",
        );
        assert_eq!(
            written.lines().collect::<Vec<_>>(),
            [
                "packing 0/20 packs",
                "packing 2/20 packs",
                "packing 4/20 packs",
                "packing 6/20 packs",
                "packing 8/20 packs",
                "packing 10/20 packs",
                "packing 12/20 packs",
                "packing 14/20 packs",
                "packing 16/20 packs",
                "packing 18/20 packs",
                "packing 20/20 packs",
            ],
        );
    }

    // What a run that failed leaves on screen. `finish` is never reached on
    // that path — the commands are `run_*(..).await?` — so `Drop` is what
    // decides, and what it must not do is erase the one line saying how far
    // the run got before the error prints beneath it.
    #[test]
    fn a_run_that_failed_leaves_the_line_saying_how_far_it_got() {
        let sink = Sink::default();
        {
            let reporting = reporting(Units::Syncing, true, &sink);
            reporting.step(Step::new(Phase::Uploading, 873, 2000));
            // No `finish`: this is the path a `?` takes out of the run.
        }

        let written = sink.text();
        assert!(
            written.contains("uploading 873/2000 containers"),
            "the position must still be on screen: {written:?}",
        );
        assert!(
            written.ends_with('\n'),
            "and the line must be closed, so the error prints under it: {written:?}",
        );
        assert!(
            !written.ends_with("\r\x1b[K"),
            "erasing it would take the position away: {written:?}",
        );
    }

    // The success path is unchanged: the summary follows it and says
    // everything the line was standing in for, so the line goes.
    #[test]
    fn a_run_that_finished_takes_its_line_back_and_drops_quietly() {
        let sink = Sink::default();
        {
            let reporting = reporting(Units::Syncing, true, &sink);
            reporting.step(Step::new(Phase::Uploading, 2000, 2000));
            reporting.finish();
        }
        assert!(
            sink.text().ends_with("\r\x1b[K"),
            "nothing of the progress may be left for the summary to sit under: {:?}",
            sink.text(),
        );
    }

    // The phases a person waits through with nothing on screen: they cannot
    // say how much they hold, and the name alone is what answers the silence.
    #[test]
    fn a_phase_that_cannot_count_its_work_is_named_without_numbers() {
        for (phase, expected) in [
            (Phase::CatchingUp, "catching up with the Library"),
            (Phase::Settling, "settling what an interrupted run left"),
            (Phase::Scanning, "scanning the mapped folders"),
        ] {
            let sink = Sink::default();
            let reporting = reporting(Units::Syncing, false, &sink);
            reporting.step(Step::begun(phase));
            reporting.finish();
            assert_eq!(sink.text().trim_end(), expected);
        }
    }

    // A phase says once that it has begun, and the counted steps that follow
    // read as they always did — which is what makes a run that starts silent
    // and then finds work to do read as one story.
    #[test]
    fn a_phase_that_has_begun_says_so_once_and_then_counts_when_it_can() {
        let sink = Sink::default();
        let reporting = reporting(Units::Syncing, false, &sink);

        reporting.step(Step::begun(Phase::Scanning));
        reporting.step(Step::begun(Phase::Scanning));
        reporting.step(Step::new(Phase::Packing, 0, 2));
        reporting.step(Step::new(Phase::Packing, 2, 2));
        reporting.finish();

        assert_eq!(
            sink.text().lines().collect::<Vec<_>>(),
            [
                "scanning the mapped folders",
                "packing 0/2 files",
                "packing 2/2 files",
            ],
        );
    }

    // An upload reports how many bytes have gone between its unit boundaries,
    // for the browser. The line here counts Containers, so those reports are
    // the same line again and a log keeps none of them.
    #[test]
    fn bytes_alone_moving_write_no_line_of_their_own() {
        let sink = Sink::default();
        let reporting = reporting(Units::Freezing, false, &sink);
        let sent = |done, bytes| {
            Step::new(Phase::Uploading, done, 1).with_bytes(ByteCount {
                done: bytes,
                total: 60,
            })
        };

        reporting.step(sent(0, 0));
        reporting.step(sent(0, 20));
        reporting.step(sent(0, 40));
        reporting.step(sent(1, 60));
        reporting.finish();

        assert_eq!(
            sink.text().lines().collect::<Vec<_>>(),
            ["uploading 0/1 containers", "uploading 1/1 containers"],
        );
    }

    // The commit after an upload is a phase of its own, counted in the objects
    // it stores, so a log does not end on an upload that reads as finished
    // while the run is still at work.
    #[test]
    fn a_commit_counts_the_objects_it_stores() {
        let sink = Sink::default();
        let reporting = reporting(Units::Freezing, false, &sink);

        reporting.step(Step::new(Phase::Uploading, 1, 1));
        for done in 0..=4 {
            reporting.step(Step::new(Phase::Committing, done, 4));
        }
        reporting.finish();

        assert_eq!(
            sink.text().lines().collect::<Vec<_>>(),
            [
                "uploading 1/1 containers",
                "committing 0/4 objects",
                "committing 1/4 objects",
                "committing 2/4 objects",
                "committing 3/4 objects",
                "committing 4/4 objects",
            ],
        );
    }

    // A phase the run passed through with nothing to do says nothing: a sync
    // that uploaded nothing is the ordinary second run over a folder.
    #[test]
    fn a_phase_with_nothing_in_it_says_nothing() {
        let sink = Sink::default();
        let reporting = reporting(Units::Fetching, true, &sink);

        reporting.step(Step::new(Phase::Uploading, 0, 0));
        reporting.finish();
        assert_eq!(sink.text(), "");
    }

    // The one word that differs between the two commands that pack, because the
    // unit differs: a sync encodes files and a freeze encodes Packs.
    #[test]
    fn the_packing_unit_is_the_one_the_command_works_in() {
        for (units, expected) in [
            (Units::Syncing, "packing 1/2 files"),
            (Units::Freezing, "packing 1/2 packs"),
        ] {
            let sink = Sink::default();
            let reporting = reporting(units, false, &sink);
            reporting.step(Step::new(Phase::Packing, 1, 2));
            reporting.finish();
            assert_eq!(sink.text().trim_end(), expected);
        }
    }
}
