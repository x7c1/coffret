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

use std::io::{stderr, IsTerminal, Write};
use std::sync::Mutex;

use coffret_device::{Phase, Progress, Step};

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

/// What the run has said so far, and where it says it.
struct Showing {
    /// Where the lines go.
    out: Box<dyn Write + Send>,
    /// The last step that was shown, for deciding whether the next one is worth
    /// showing at all.
    shown: Option<Step>,
    /// Whether a line is standing on the terminal, waiting to be taken back.
    standing: bool,
}

/// A run's progress, on the terminal or in a log.
pub struct Reporting {
    units: Units,
    /// Whether the other end can take a line back.
    terminal: bool,
    showing: Mutex<Showing>,
}

impl Reporting {
    /// Reports to standard error, rendered for whatever is on the other end.
    pub fn to_stderr(units: Units) -> Self {
        Self::new(units, stderr().is_terminal(), Box::new(stderr()))
    }

    /// The same, with both decisions made by the caller.
    fn new(units: Units, terminal: bool, out: Box<dyn Write + Send>) -> Self {
        Self {
            units,
            terminal,
            showing: Mutex::new(Showing {
                out,
                shown: None,
                standing: false,
            }),
        }
    }

    /// Takes back the line standing on the terminal, if one is.
    ///
    /// Called when the run has finished and before anything else is printed, so
    /// that the summary is not written over a progress line. Erasing is right
    /// here and only here: what follows is the summary, which says everything
    /// the progress line was standing in for.
    pub fn finish(&self) {
        let mut showing = self.lock();
        if !showing.standing {
            return;
        }
        showing.standing = false;
        // Back to the start of the line and clear to its end: the cursor ends
        // where it began, and nothing of the progress is left behind.
        let _ = showing.out.write_all(b"\r\x1b[K");
        let _ = showing.out.flush();
    }

    /// The lock, or the guard of a thread that panicked holding it.
    ///
    /// A poisoned lock means something else has already failed; losing the
    /// progress line as well would replace that failure's report with this
    /// one's.
    fn lock(&self) -> std::sync::MutexGuard<'_, Showing> {
        self.showing.lock().unwrap_or_else(|held| held.into_inner())
    }

    /// The line one step reads as.
    ///
    /// A phase that can count its work reads as what it is doing, how far
    /// through it is, and the unit it counts in. One that cannot — a catch-up
    /// that learns what it has to replay by replaying it, a scan that is
    /// itself the count — reads as what it is doing and stops there: a number
    /// it does not have would have to be invented, and the phase's name alone
    /// already answers the question the silence raised.
    fn line(&self, step: Step) -> String {
        let (doing, unit) = match step.phase {
            Phase::CatchingUp => ("catching up with the Library", None),
            Phase::Reconciling => ("settling what the last run left", None),
            Phase::Scanning => ("scanning the mapped folders", None),
            Phase::Packing => ("packing", Some(self.units.packed())),
            Phase::Uploading => ("uploading", Some("containers")),
            Phase::Fetching => ("fetching", Some("containers")),
        };
        match (step.total, unit) {
            (Some(total), Some(unit)) => format!("{doing} {}/{total} {unit}", step.done),
            (Some(total), None) => format!("{doing} {}/{total}", step.done),
            (None, _) => doing.to_owned(),
        }
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
        let mut showing = self.lock();
        if !self.terminal && !worth_keeping(showing.shown, step) {
            showing.shown = Some(step);
            return;
        }
        showing.shown = Some(step);

        let line = self.line(step);
        let written = match self.terminal {
            true => {
                showing.standing = true;
                showing.out.write_all(format!("\r{line}\x1b[K").as_bytes())
            }
            false => showing.out.write_all(format!("{line}\n").as_bytes()),
        };
        // A progress line that will not go out is not a reason to stop a
        // transfer, and the run has its own way of reporting what it did.
        let _ = written.and_then(|()| showing.out.flush());
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
        let mut showing = self.lock();
        if !showing.standing {
            return;
        }
        showing.standing = false;
        let _ = showing
            .out
            .write_all(b"\n")
            .and_then(|()| showing.out.flush());
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
    use std::sync::{Arc, Mutex};

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

    /// A reporting that writes into `sink`, rendered for a terminal or not.
    fn reporting(units: Units, terminal: bool, sink: &Sink) -> Reporting {
        Reporting::new(units, terminal, Box::new(sink.clone()))
    }

    // The whole point of the callback being a trait: what a run says can be
    // read without a terminal to read it on, which is also how the case below
    // asserts on the rendering a terminal would get.
    #[test]
    fn a_terminal_gets_one_line_rewritten_in_place_and_taken_back() {
        let sink = Sink::default();
        let reporting = reporting(Units::Fetching, true, &sink);

        reporting.step(Step::new(Phase::Fetching, 0, 2));
        reporting.step(Step::new(Phase::Fetching, 1, 2));
        reporting.step(Step::new(Phase::Fetching, 2, 2));
        assert_eq!(
            sink.text(),
            "\rfetching 0/2 containers\x1b[K\
             \rfetching 1/2 containers\x1b[K\
             \rfetching 2/2 containers\x1b[K",
        );

        reporting.finish();
        assert!(
            sink.text().ends_with("\r\x1b[K"),
            "the line must be taken back before the summary is printed: {:?}",
            sink.text(),
        );
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
            (Phase::Reconciling, "settling what the last run left"),
            (Phase::Scanning, "scanning the mapped folders"),
        ] {
            let sink = Sink::default();
            let reporting = reporting(Units::Syncing, false, &sink);
            reporting.step(Step::begun(phase));
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

        assert_eq!(
            sink.text().lines().collect::<Vec<_>>(),
            [
                "scanning the mapped folders",
                "packing 0/2 files",
                "packing 2/2 files",
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
            assert_eq!(sink.text().trim_end(), expected);
        }
    }
}
