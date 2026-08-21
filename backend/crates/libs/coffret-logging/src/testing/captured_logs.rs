use std::sync::{Arc, Mutex};

use tracing::subscriber::DefaultGuard;
use tracing::Level;

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
