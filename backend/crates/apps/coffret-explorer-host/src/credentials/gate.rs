use std::sync::Arc;

use super::{ExchangeRefused, Exchanged, InstanceId, Session, Shared, Unauthenticated};

/// The handle request handlers are given: it admits, and issues nothing.
#[derive(Clone)]
pub struct Gate {
    pub(super) shared: Arc<Shared>,
}

impl Gate {
    /// Exchanges a launch ticket, as the page presented it, for a browser
    /// token, spending the ticket (spec: LA-16, LA-17).
    pub fn exchange(&self, ticket: &str) -> Result<Exchanged, ExchangeRefused> {
        let now = self.shared.clock.now();
        self.shared.lock().exchange(ticket, now)
    }

    /// The session a browser token, as the page presented it, stands for
    /// (spec: LA-14).
    pub fn authenticate(&self, token: &str) -> Result<Session, Unauthenticated> {
        let now = self.shared.clock.now();
        self.shared.lock().authenticate(token, now)
    }

    /// Ends `session`, as its own page asks to. Whether it was still held.
    pub fn revoke(&self, session: &Session) -> bool {
        self.shared.lock().revoke(session)
    }

    /// The current generation's public name.
    pub fn instance_id(&self) -> InstanceId {
        self.shared.lock().instance_id()
    }
}
