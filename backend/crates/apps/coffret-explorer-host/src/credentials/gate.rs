use std::sync::Arc;

use super::{ExchangeRefused, Exchanged, InstanceId, Session, Shared, Unauthenticated};

/// The handle request handlers are given: it admits, and issues nothing.
///
/// It retires a generation only by failing closed, which the router does when
/// it cannot confirm the server behind it (spec: LA-15).
#[derive(Clone)]
pub struct Gate {
    pub(super) shared: Arc<Shared>,
}

impl Gate {
    /// Exchanges a launch ticket, as the page presented it, for a browser
    /// token, spending the ticket (spec: LA-16, LA-17).
    pub fn exchange(&self, ticket: &str) -> Result<Exchanged, ExchangeRefused> {
        let now = self.shared.clock.now();
        let mut state = self.shared.lock();
        let exchanged = state.exchange(ticket, now)?;
        tracing::info!(
            kind = "exchange",
            sessions = state.held().sessions,
            live_tickets = state.held().tickets,
            "a launch ticket was exchanged for a browser token",
        );
        Ok(exchanged)
    }

    /// The session a browser token, as the page presented it, stands for
    /// (spec: LA-14).
    pub fn authenticate(&self, token: &str) -> Result<Session, Unauthenticated> {
        let now = self.shared.clock.now();
        self.shared.lock().authenticate(token, now)
    }

    /// Ends `session`, as its own page asks to. Whether it was still held.
    pub fn revoke(&self, session: &Session) -> bool {
        let mut state = self.shared.lock();
        let revoked = state.revoke(session);
        if revoked {
            tracing::info!(
                kind = "revocation",
                reason = "page",
                sessions = state.held().sessions,
                "a session was revoked by its own page",
            );
        }
        revoked
    }

    /// Discards every ticket and session and starts the next generation,
    /// because the server behind the host cannot be confirmed as the one they
    /// were issued under: the host fails closed (spec: LA-15).
    pub(crate) fn fail_closed(&self, reason: &'static str) {
        self.shared.retire(reason);
    }

    /// The current generation's public name.
    pub fn instance_id(&self) -> InstanceId {
        self.shared.lock().instance_id()
    }
}
