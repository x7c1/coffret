use std::io::{self, Write};
use std::sync::{Arc, Mutex, MutexGuard};

use tracing_subscriber::fmt::MakeWriter;

/// Collects formatted events into a buffer a test can read.
pub(super) struct CaptureWriter {
    events: Arc<Mutex<Vec<u8>>>,
    target: Option<&'static str>,
}

impl CaptureWriter {
    /// Writes into a buffer, keeping either everything or one target's events.
    pub(super) fn new(events: Arc<Mutex<Vec<u8>>>, target: Option<&'static str>) -> Self {
        Self { events, target }
    }
}

impl<'writer> MakeWriter<'writer> for CaptureWriter {
    type Writer = CaptureGuard<'writer>;

    fn make_writer(&'writer self) -> Self::Writer {
        CaptureGuard {
            events: self.events.lock().expect("no test panics here"),
            keep: true,
        }
    }

    fn make_writer_for(&'writer self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        let keep = match self.target {
            Some(target) => meta.target().starts_with(target),
            None => true,
        };
        CaptureGuard {
            events: self.events.lock().expect("no test panics here"),
            keep,
        }
    }
}

/// One event on its way into the buffer, or on its way nowhere.
pub(super) struct CaptureGuard<'writer> {
    events: MutexGuard<'writer, Vec<u8>>,
    keep: bool,
}

impl Write for CaptureGuard<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.keep {
            return Ok(buf.len());
        }
        self.events.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.events.flush()
    }
}
