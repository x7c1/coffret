use std::sync::{Arc, Mutex};

use tracing::subscriber::DefaultGuard;
use tracing::Level;

use super::capture_writer::CaptureWriter;

/// The events emitted on this thread while it is alive.
///
/// Dropping it puts back whatever subscriber was there before — none, in a test
/// binary that installs nothing.
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
        let subscriber = tracing_subscriber::fmt()
            .with_writer(CaptureWriter::new(events.clone(), target))
            .with_ansi(false)
            .with_max_level(Level::TRACE)
            .finish();

        Self {
            events,
            _installed: tracing::subscriber::set_default(subscriber),
        }
    }

    /// Everything emitted so far, as it was formatted.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.events.lock().expect("no test panics here")).into_owned()
    }

    /// The events emitted at one level.
    pub fn at(&self, level: Level) -> Vec<String> {
        let marker = format!(" {} ", level.as_str());
        self.text()
            .lines()
            .filter(|line| line.contains(&marker))
            .map(str::to_owned)
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
    pub fn only(&self, level: Level) -> String {
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
    /// # Panics
    ///
    /// If any of `secrets` appears anywhere in what was emitted.
    pub fn assert_free_of(&self, secrets: &[&str]) {
        let emitted = self.text();
        for secret in secrets {
            assert!(
                !emitted.contains(secret),
                "{secret:?} reached the log:\n{emitted}",
            );
        }
    }
}
