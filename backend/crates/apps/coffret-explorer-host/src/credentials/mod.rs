//! The credentials the host admits a browser by: one-time launch tickets, and
//! the browser tokens they are exchanged for (spec: LA-17, LA-18).
//!
//! A browser cannot be handed the server key (spec: LA-3), and checking the
//! `Host` and `Origin` a request names keeps another site's page out but not
//! another program on the device, which writes those headers as it likes
//! (spec: LA-14). So the host admits a browser only by a token of its own,
//! which the explorer's page gets by exchanging a ticket handed to it in the
//! fragment of the URL it was loaded from (spec: LA-21).
//!
//! Every ticket and session is held in one state behind one lock, so that
//! each transition — an issue, an exchange, a revocation, a retirement — is
//! one step that nothing else is interleaved with, and of concurrent exchanges
//! of one ticket exactly one finds it there.
//!
//! The state is reached through two handles, and which one a caller holds is
//! what it may do:
//!
//! - [`Credentials`], held by the app that owns the host: it issues tickets,
//!   revokes every session, and retires a generation. Issuing a ticket is a
//!   call on this handle and on nothing else, and no request handler is given
//!   it, so no HTTP request can issue one (spec: LA-17).
//! - [`Gate`], cloned into whatever answers requests: it exchanges a ticket,
//!   authenticates a token, and revokes the one session a token stands for.
//!
//! Nothing here knows HTTP. What each outcome is answered with is the
//! router's to decide.
//!
//! Every transition that changes what admits is recorded as an event by its
//! kind and the counts it left — never a ticket, a token or either one's
//! digest (spec: EL-1). A refused exchange or authentication is recorded by
//! the router, which knows what it answered.

mod clock;
pub use clock::Clock;

mod exchange_refused;
pub use exchange_refused::ExchangeRefused;

mod exchanged;
pub use exchanged::Exchanged;

mod gate;
pub use gate::Gate;

mod instance_id;
pub use instance_id::InstanceId;

mod not_drawn;
pub use not_drawn::NotDrawn;

mod rate_limit;

mod secret;
pub use secret::{BrowserToken, Digest, LaunchTicket, Secret};

mod session;
pub use session::Session;
use session::SessionId;

mod state;
use state::State;

mod system_clock;
pub use system_clock::SystemClock;

mod unauthenticated;
pub use unauthenticated::Unauthenticated;

#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex, MutexGuard};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

/// How many bytes the host's part of an [`InstanceId`] is drawn from. Not a
/// secret, so it need only be wide enough that two hosts never share one.
const HOST_ID_BYTES: usize = 16;

/// The state and the clock it is read against, shared by both handles.
struct Shared {
    state: Mutex<State>,
    clock: Arc<dyn Clock>,
}

impl Shared {
    /// The state, for one transition.
    ///
    /// A lock poisoned by a panic in another transition may hold a state that
    /// transition left half done. It is not trusted: every ticket and session
    /// is discarded and the generation retired, so that what fails, fails
    /// closed (spec: LA-15).
    fn lock(&self) -> MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.retire();
                self.state.clear_poison();
                tracing::warn!(
                    "a credentials transition panicked; every launch ticket and session was discarded",
                );
                state
            }
        }
    }

    /// [`State::revoke_all`], recorded as a revocation for `reason`.
    fn revoke_all(&self, reason: &'static str) {
        let mut state = self.lock();
        let before = state.held();
        state.revoke_all();
        revoked(reason, before);
    }

    /// [`State::retire`], recorded as a revocation for `reason`.
    fn retire(&self, reason: &'static str) {
        let mut state = self.lock();
        let before = state.held();
        state.retire();
        revoked(reason, before);
    }
}

/// Records that every ticket and session in `before` was discarded.
fn revoked(reason: &'static str, before: state::Held) {
    tracing::info!(
        kind = "revocation",
        reason,
        revoked_sessions = before.sessions,
        spent_tickets = before.tickets,
        "every session was revoked and every launch ticket spent",
    );
}

/// The handle the app that owns the host holds: the one that issues tickets.
///
/// Not `Clone`. Hand request handlers a [`Gate`] instead.
pub struct Credentials {
    shared: Arc<Shared>,
}

impl Credentials {
    /// Credentials for a new host, read against `clock`, holding nothing.
    pub fn new(clock: Arc<dyn Clock>) -> Result<Self, NotDrawn> {
        let mut host = [0_u8; HOST_ID_BYTES];
        getrandom::fill(&mut host).map_err(NotDrawn)?;
        let state = State::new(URL_SAFE_NO_PAD.encode(host), clock.now());
        Ok(Self {
            shared: Arc::new(Shared {
                state: Mutex::new(state),
                clock,
            }),
        })
    }

    /// The handle request handlers are given.
    pub fn gate(&self) -> Gate {
        Gate {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Issues a launch ticket, good for one exchange within a minute
    /// (spec: LA-17). Where four are already live, the oldest of them is
    /// expired to make room.
    pub fn issue_ticket(&self) -> Result<LaunchTicket, NotDrawn> {
        let now = self.shared.clock.now();
        let mut state = self.shared.lock();
        let ticket = state.issue_ticket(now)?;
        tracing::info!(
            kind = "issue",
            live_tickets = state.held().tickets,
            "a launch ticket was issued",
        );
        Ok(ticket)
    }

    /// Whether `ticket` is still good for an exchange: not exchanged, not
    /// expired, and issued in the current generation (spec: LA-17).
    ///
    /// What whoever delivered it reads to know when the file it was delivered
    /// in has served its turn and is to be removed (spec: LA-21).
    pub fn is_live(&self, ticket: &LaunchTicket) -> bool {
        let now = self.shared.clock.now();
        self.shared.lock().is_live(ticket, now)
    }

    /// Ends every session, and spends every ticket not yet exchanged
    /// (spec: LA-18).
    pub fn revoke_all(&self) {
        self.shared.revoke_all("app");
    }

    /// Discards every ticket and session and starts the next generation: what
    /// the host does when it or the server behind it restarts, when the
    /// Library it serves changes, and when it fails closed (spec: LA-15,
    /// LA-18).
    pub fn retire(&self) {
        self.shared.retire("app");
    }

    /// The current generation's public name.
    pub fn instance_id(&self) -> InstanceId {
        self.shared.lock().instance_id()
    }
}
