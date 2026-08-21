use std::sync::{Arc, Mutex, OnceLock};

use tracing::level_filters::LevelFilter;
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::{DefaultGuard, Interest};
use tracing::{Dispatch, Event, Level, Metadata, Subscriber};

use super::capture_writer::CaptureWriter;
use super::LoggedEvent;
use crate::jsonl;

/// The events emitted on this thread while it is alive.
///
/// Dropping it puts back whatever subscriber was there before — none, in a test
/// binary that installs nothing.
///
/// The subscriber is built the same way the installed sink's is, JSONL and all,
/// so that what a case reads back is what the file would have been given. A
/// capture in some other shape would prove things about a format nothing
/// writes.
pub struct CapturedLogs {
    events: Arc<Mutex<Vec<u8>>>,
    _installed: DefaultGuard,
}

impl CapturedLogs {
    /// Starts collecting everything emitted on this thread, at every level.
    pub fn capture() -> Self {
        Self::capture_from(None)
    }

    /// Collects only what a crate of ours emitted, by target prefix.
    ///
    /// For the cases that run against a third-party SDK: its own
    /// instrumentation is nobody's business here, and a case asserting that a
    /// credential never reached an event is asking about the events this
    /// workspace writes.
    pub fn capture_target(target: &'static str) -> Self {
        Self::capture_from(Some(target))
    }

    /// Collects what is emitted on this thread, at every level.
    fn capture_from(target: Option<&'static str>) -> Self {
        keep_the_callsites_worth_evaluating();

        let events = Arc::new(Mutex::new(Vec::new()));
        let subscriber = jsonl::subscriber(
            CaptureWriter::new(events.clone(), target),
            // Everything, unlike the installed sink: a case is entitled to ask
            // about an event the sink's own settings would have filtered out.
            Level::TRACE,
        );

        Self {
            events,
            _installed: tracing::subscriber::set_default(subscriber),
        }
    }

    /// Everything emitted so far, as the file would hold it: one JSON object
    /// per line.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.events.lock().expect("no test panics here")).into_owned()
    }

    /// Every event emitted so far, read back.
    pub fn events(&self) -> Vec<LoggedEvent> {
        self.text().lines().map(LoggedEvent::parse).collect()
    }

    /// The events emitted at one level.
    pub fn at(&self, level: Level) -> Vec<LoggedEvent> {
        self.events()
            .into_iter()
            .filter(|event| event.level() == level.as_str())
            .collect()
    }

    /// Fails unless exactly one event was emitted at this level.
    ///
    /// Most cases are about one event in particular, and asserting on the only
    /// one there is says more than finding one among several.
    ///
    /// # Panics
    ///
    /// If no event, or more than one, was emitted at `level`.
    pub fn only(&self, level: Level) -> LoggedEvent {
        let mut events = self.at(level);
        assert_eq!(
            events.len(),
            1,
            "expected one {level} event, got {events:#?}\nin:\n{}",
            self.text(),
        );
        events.remove(0)
    }

    /// Fails if any of these ever reached an event.
    ///
    /// Searched twice: over the file's own bytes, and over every record with
    /// the JSON escaping undone. A secret carrying a quote or a newline is
    /// escaped on its way into the file and would otherwise hide from the first
    /// search while sitting in plain sight of anyone reading through `jq`.
    ///
    /// # Panics
    ///
    /// If any of `secrets` appears anywhere in what was emitted.
    pub fn assert_free_of(&self, secrets: &[&str]) {
        let emitted = self.text();
        let unescaped: String = self
            .events()
            .iter()
            .map(LoggedEvent::plain)
            .collect::<Vec<_>>()
            .concat();

        for secret in secrets {
            assert!(
                !emitted.contains(secret) && !unescaped.contains(secret),
                "{secret:?} reached the log:\n{emitted}",
            );
        }
    }
}

/// Registers, once for the whole test binary, a subscriber that is interested
/// in every callsite and records nothing.
///
/// A capture is the *thread's* default subscriber, which is what keeps cases
/// from reading each other's events. But whether a callsite is worth evaluating
/// at all is cached per callsite for the whole process, and while only one
/// subscriber is registered, `tracing` computes that answer from whatever is
/// installed on the thread that happened to reach the callsite first. A thread
/// carrying no capture answers "nobody is listening" on everyone's behalf, and
/// every capture already running reads an empty log until the next one is
/// built. The failure then lands on whichever case was unlucky — a case that
/// asserts on an event its own code emitted normally, and that says nothing
/// about why it did not arrive.
///
/// Keeping one subscriber registered for as long as the binary runs means the
/// answer is never reached. It is registered, never installed: nothing routes
/// an event to it, it has nowhere to put one, and each case still reads only
/// what its own thread emitted.
fn keep_the_callsites_worth_evaluating() {
    static REGISTERED: OnceLock<Dispatch> = OnceLock::new();

    // Registering is what building a `Dispatch` does, and the registry keeps
    // only a weak reference to it — so staying registered is a matter of the
    // strong one staying somewhere, which is what the `OnceLock` is for.
    REGISTERED.get_or_init(|| Dispatch::new(SomethingMightBeListening));
}

/// Interested in everything, enabled at nothing.
struct SomethingMightBeListening;

impl Subscriber for SomethingMightBeListening {
    fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
        // Anything but `never`, which is the whole reason this exists.
        // `sometimes` rather than `always` because the decision belongs to
        // whatever is installed on the thread that emits, and this is asking
        // for it to be made there rather than making it here.
        Interest::sometimes()
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        // A case is entitled to ask about an event at any level, so this must
        // not be what holds the process's ceiling down.
        Some(LevelFilter::TRACE)
    }

    fn enabled(&self, _: &Metadata<'_>) -> bool {
        false
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        // Unreachable in practice, since nothing here is ever enabled. Id 1
        // rather than 0 because zero is not a span identifier.
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, _: &Event<'_>) {}

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

#[cfg(test)]
mod tests {
    use std::sync::{Barrier, Mutex, MutexGuard};
    use std::thread;

    use tracing::{warn, Level};

    use super::CapturedLogs;

    /// Held for the length of each case below.
    ///
    /// The second case asks what a thread carrying no capture leaves behind,
    /// and it can only ask that while no other capture is alive — a capture
    /// belonging to a case running alongside it would answer for it, and the
    /// case would pass without having asked anything. Cases run in parallel, so
    /// the only way to be sure is to take turns.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// Takes the turn, whether or not the case before it panicked.
    ///
    /// A failure is one case's news to report. Poisoning the lock would make it
    /// every later case's as well, and those failures would say nothing.
    fn a_turn_of_its_own() -> MutexGuard<'static, ()> {
        ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The event the case about two captures at once looks for.
    fn side_by_side_event(marker: &str) {
        warn!(marker, "an event one of two captures is about to look for");
    }

    /// The event the case about a cold callsite looks for.
    ///
    /// Its own function, and so its own callsite: that case is about what
    /// happens to a callsite nothing has reached yet, which it could not ask if
    /// it shared one with a case that reaches it under a capture.
    fn cold_event(marker: &str) {
        warn!(marker, "an event at a callsite nothing has reached yet");
    }

    #[test]
    fn two_captures_at_once_read_only_what_their_own_thread_emitted() {
        let _turn = a_turn_of_its_own();
        let both_installed = Barrier::new(2);
        let both_emitted = Barrier::new(2);

        thread::scope(|scope| {
            let other = scope.spawn(|| {
                let logs = CapturedLogs::capture();
                both_installed.wait();
                side_by_side_event("the other thread's");
                both_emitted.wait();

                let event = logs.only(Level::WARN);
                assert_eq!(event.field("marker"), "the other thread's", "{event}");
            });

            let logs = CapturedLogs::capture();
            both_installed.wait();
            side_by_side_event("this thread's");
            both_emitted.wait();

            // Both captures are alive, both emitted at the same callsite, and
            // each is entitled to exactly one event: whatever keeps the callsite
            // worth evaluating must not become somewhere events pile up, and
            // must not route one thread's event to another thread's capture.
            let event = logs.only(Level::WARN);
            assert_eq!(event.field("marker"), "this thread's", "{event}");

            other.join().expect("the other thread reads its own event");
        });
    }

    #[test]
    fn a_callsite_reached_first_by_a_thread_with_no_capture_still_reaches_one() {
        let _turn = a_turn_of_its_own();
        let capture_installed = Barrier::new(2);
        let callsite_reached = Barrier::new(2);

        thread::scope(|scope| {
            scope.spawn(|| {
                capture_installed.wait();
                // Nothing is installed on this thread. It is the thread that
                // decides, for the whole process, whether this callsite is worth
                // evaluating — and the answer it used to leave behind was "no".
                cold_event("from a thread with nothing installed");
                callsite_reached.wait();
            });

            let logs = CapturedLogs::capture();
            capture_installed.wait();
            callsite_reached.wait();
            cold_event("from the thread that is capturing");

            let event = logs.only(Level::WARN);
            assert_eq!(
                event.field("marker"),
                "from the thread that is capturing",
                "a case must not lose its own events to a thread that captured none: {event}",
            );
        });
    }
}
