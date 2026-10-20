use std::sync::{Arc, Mutex, PoisonError};

use crate::Event;

/// Whatever the shell receives [`Event`]s through.
pub type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

/// The sink the shell registered, if it has.
#[derive(Default)]
pub(crate) struct Events(Mutex<Option<EventSink>>);

impl Events {
    /// Sends every event from now on to `sink`, in place of any before it.
    pub(crate) fn listen(&self, sink: EventSink) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(sink);
    }

    /// Hands `event` to the sink, and says in the log where there is none: an
    /// unlock the explorer asked for and nobody heard is one a person waits on.
    ///
    /// The sink is called with the lock released, so that a sink registering
    /// another does not wait on itself.
    pub(crate) fn send(&self, event: Event) {
        let sink = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        match sink {
            Some(sink) => {
                tracing::info!(
                    kind = "desktop_event",
                    event = event.word(),
                    "the shell was told"
                );
                sink(event);
            }
            None => tracing::warn!(
                kind = "desktop_event_unheard",
                event = event.word(),
                "the shell registered nothing to tell",
            ),
        }
    }
}
