use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::rate_limit::RateLimit;
use super::secret::{BrowserToken, Digest, LaunchTicket};
use super::{
    ExchangeRefused, Exchanged, InstanceId, NotDrawn, Session, SessionId, Unauthenticated,
};

/// How long a launch ticket is good for after it was issued (spec: LA-17).
pub(super) const TICKET_LIFETIME: Duration = Duration::from_secs(60);

/// How many launch tickets may be live at once (spec: LA-17).
pub(super) const TICKET_CAP: usize = 4;

/// How long a session lasts after its token was issued (spec: LA-18).
pub(super) const SESSION_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);

/// How many sessions may be held at once; an exchange past it revokes the
/// session used least recently (spec: LA-18).
pub(super) const SESSION_CAP: usize = 8;

/// Every ticket and session the host holds, as one state machine.
///
/// Only digests are kept (spec: LA-20). Every transition takes the time it
/// happens at rather than reading a clock, so that what one call sees is one
/// instant throughout, and so that this is plain data a case can drive.
pub(super) struct State {
    /// Drawn as the host starts, and public: what an [`InstanceId`] is made
    /// of beside the generation.
    host: String,
    generation: u64,
    /// In the order they were issued, so that the front is the oldest.
    tickets: VecDeque<HeldTicket>,
    /// In the order they were issued, too.
    sessions: Vec<HeldSession>,
    next_session: u64,
    rate: RateLimit,
}

struct HeldTicket {
    digest: Digest,
    expires_at: Instant,
}

/// How many tickets and sessions a host holds.
#[derive(Debug, Clone, Copy)]
pub(super) struct Held {
    pub(super) tickets: usize,
    pub(super) sessions: usize,
}

struct HeldSession {
    id: SessionId,
    digest: Digest,
    expires_at: Instant,
    /// When its token last authenticated a request, or was issued: what the
    /// session cap revokes the oldest of (spec: LA-18).
    last_used: Instant,
}

/// What an exchange made: the page's token, and whether a session was revoked
/// to make room for it.
pub(super) struct Admission {
    pub(super) exchanged: Exchanged,
    /// Whether the session cap was reached, so that the session used least
    /// recently was revoked to admit this one (spec: LA-18).
    pub(super) evicted: bool,
}

impl State {
    pub(super) fn new(host: String, now: Instant) -> Self {
        Self {
            host,
            generation: 0,
            tickets: VecDeque::new(),
            sessions: Vec::new(),
            next_session: 0,
            rate: RateLimit::new(now),
        }
    }

    pub(super) fn instance_id(&self) -> InstanceId {
        InstanceId {
            host: self.host.clone(),
            generation: self.generation,
        }
    }

    /// Issues a ticket, expiring the oldest unexchanged one where the cap
    /// would be passed (spec: LA-17).
    pub(super) fn issue_ticket(&mut self, now: Instant) -> Result<LaunchTicket, NotDrawn> {
        let ticket = LaunchTicket::draw()?;
        self.expire(now);
        while self.tickets.len() >= TICKET_CAP {
            self.tickets.pop_front();
        }
        self.tickets.push_back(HeldTicket {
            digest: ticket.digest(),
            expires_at: now + TICKET_LIFETIME,
        });
        Ok(ticket)
    }

    /// Exchanges `presented` for a browser token, spending the ticket.
    ///
    /// The limit is checked first, so that every attempt counts against it,
    /// a malformed one included; the token's draw comes before the ticket is
    /// spent, so that a failed draw costs the page neither its ticket nor
    /// another tab its session. Where the host already holds as many sessions
    /// as it may, the one used least recently is revoked to make room
    /// (spec: LA-18).
    pub(super) fn exchange(
        &mut self,
        presented: &str,
        now: Instant,
    ) -> Result<Admission, ExchangeRefused> {
        self.rate
            .take(now)
            .map_err(|retry_after| ExchangeRefused::RateLimited { retry_after })?;
        let presented = Digest::of_presented(presented).ok_or(ExchangeRefused::Unauthenticated)?;
        self.expire(now);
        let held = find(self.tickets.iter().map(|held| &held.digest), &presented)
            .ok_or(ExchangeRefused::Unauthenticated)?;
        let token = BrowserToken::draw().map_err(ExchangeRefused::NotDrawn)?;

        self.tickets.remove(held);
        let evicted = self.sessions.len() >= SESSION_CAP;
        if evicted {
            self.evict_least_recently_used();
        }
        let id = SessionId(self.next_session);
        self.next_session += 1;
        self.sessions.push(HeldSession {
            id,
            digest: token.digest(),
            expires_at: now + SESSION_LIFETIME,
            last_used: now,
        });
        Ok(Admission {
            exchanged: Exchanged {
                token,
                expires_in: SESSION_LIFETIME,
                instance_id: self.instance_id(),
            },
            evicted,
        })
    }

    /// Revokes the session whose token was used least recently; of two used
    /// at the same instant, the one issued first, since the sessions are held
    /// in the order they were issued and the first of equals is the one taken.
    fn evict_least_recently_used(&mut self) {
        let oldest = self
            .sessions
            .iter()
            .enumerate()
            .min_by_key(|(_, held)| held.last_used)
            .map(|(at, _)| at);
        if let Some(at) = oldest {
            self.sessions.remove(at);
        }
    }

    /// The session `presented` stands for, while it lasts, marked as used
    /// at `now` (spec: LA-14, LA-18).
    pub(super) fn authenticate(
        &mut self,
        presented: &str,
        now: Instant,
    ) -> Result<Session, Unauthenticated> {
        let presented = Digest::of_presented(presented).ok_or(Unauthenticated)?;
        self.expire(now);
        let held = find(self.sessions.iter().map(|held| &held.digest), &presented)
            .ok_or(Unauthenticated)?;
        let instance_id = self.instance_id();
        let held = &mut self.sessions[held];
        held.last_used = now;
        Ok(Session {
            id: held.id,
            instance_id,
            expires_in: held.expires_at - now,
        })
    }

    /// Whether `ticket` is still good for an exchange: issued in this
    /// generation, not exchanged, and not expired by `now` (spec: LA-17).
    pub(super) fn is_live(&self, ticket: &LaunchTicket, now: Instant) -> bool {
        let digest = ticket.digest();
        find(
            self.tickets
                .iter()
                .filter(|held| now < held.expires_at)
                .map(|held| &held.digest),
            &digest,
        )
        .is_some()
    }

    /// Ends one session. Whether it was still held.
    pub(super) fn revoke(&mut self, session: &Session) -> bool {
        if session.instance_id != self.instance_id() {
            return false;
        }
        let before = self.sessions.len();
        self.sessions.retain(|held| held.id != session.id);
        self.sessions.len() != before
    }

    /// Ends every session, and spends every ticket not yet exchanged, so that
    /// no ticket makes a session after every session was revoked (spec: LA-18).
    pub(super) fn revoke_all(&mut self) {
        self.tickets.clear();
        self.sessions.clear();
    }

    /// Discards everything and starts the next generation (spec: LA-18).
    pub(super) fn retire(&mut self) {
        self.revoke_all();
        self.generation += 1;
    }

    /// How many tickets are live and how many sessions are held, as of the
    /// last transition: the counts an event about one may carry (spec: EL-1).
    pub(super) fn held(&self) -> Held {
        Held {
            tickets: self.tickets.len(),
            sessions: self.sessions.len(),
        }
    }

    /// Forgets every ticket and session that has expired by `now`.
    fn expire(&mut self, now: Instant) {
        self.tickets.retain(|held| now < held.expires_at);
        self.sessions.retain(|held| now < held.expires_at);
    }
}

/// Where `presented` is among `held`, comparing each without stopping at the
/// first byte that differs (spec: LA-20).
///
/// Every digest held is compared, a match or not: the lists are a handful long,
/// and a search that stopped would say how far it got.
fn find<'a>(held: impl Iterator<Item = &'a Digest>, presented: &Digest) -> Option<usize> {
    held.enumerate().fold(None, |found, (at, digest)| {
        if digest.same_as(presented) {
            Some(at)
        } else {
            found
        }
    })
}
