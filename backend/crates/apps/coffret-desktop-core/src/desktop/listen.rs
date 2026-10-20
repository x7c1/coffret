use std::sync::Arc;

use super::Desktop;
use crate::events::EventSink;
use crate::Event;

impl Desktop {
    /// Hands every [`Event`] from now on to `sink`, in place of any before it.
    ///
    /// `sink` is called from the runtime's threads, so it hands the event on
    /// rather than doing the shell's work there.
    pub fn listen(&self, sink: impl Fn(Event) + Send + Sync + 'static) {
        let sink: EventSink = Arc::new(sink);
        self.shared.events.listen(sink);
    }
}
