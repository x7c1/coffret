use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::state::{SESSION_CAP, SESSION_LIFETIME, TICKET_CAP, TICKET_LIFETIME};
use super::*;

/// A clock a case moves by hand.
struct ManualClock {
    start: Instant,
    elapsed: Mutex<Duration>,
}

impl ManualClock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            start: Instant::now(),
            elapsed: Mutex::new(Duration::ZERO),
        })
    }

    fn advance(&self, by: Duration) {
        *self.elapsed.lock().unwrap() += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.start + *self.elapsed.lock().unwrap()
    }
}

fn credentials() -> (Credentials, Gate, Arc<ManualClock>) {
    let clock = ManualClock::new();
    let credentials = Credentials::new(clock.clone()).expect("the CSPRNG answers");
    let gate = credentials.gate();
    (credentials, gate, clock)
}

/// A ticket, issued and encoded the way the page will present it.
fn ticket(credentials: &Credentials) -> String {
    credentials
        .issue_ticket()
        .expect("the CSPRNG answers")
        .encoded()
        .to_string()
}

/// A token, exchanged for a fresh ticket and encoded the way the page sends
/// it.
fn token(credentials: &Credentials, gate: &Gate) -> String {
    gate.exchange(&ticket(credentials))
        .expect("a fresh ticket exchanges")
        .token
        .encoded()
        .to_string()
}

fn is_unauthenticated(outcome: Result<Exchanged, ExchangeRefused>) -> bool {
    matches!(outcome, Err(ExchangeRefused::Unauthenticated))
}

// LA-17: a ticket is good for one exchange, and what it answers admits the
// page.
#[test]
fn a_ticket_exchanges_once() {
    let (credentials, gate, _) = credentials();
    let ticket = ticket(&credentials);

    let exchanged = gate.exchange(&ticket).expect("the first exchange succeeds");
    assert_eq!(exchanged.expires_in, SESSION_LIFETIME);
    assert_eq!(exchanged.instance_id, credentials.instance_id());
    let session = gate
        .authenticate(&exchanged.token.encoded())
        .expect("the token admits");
    assert_eq!(session.instance_id(), &exchanged.instance_id);

    assert!(is_unauthenticated(gate.exchange(&ticket)));
}

// LA-17: used, expired, never issued, retired and malformed tickets are one
// refusal that names none of them.
#[test]
fn every_ticket_that_is_not_good_fails_the_same_way() {
    let (credentials, gate, clock) = credentials();

    let used = ticket(&credentials);
    gate.exchange(&used).unwrap();
    assert!(is_unauthenticated(gate.exchange(&used)));

    let expired = ticket(&credentials);
    clock.advance(TICKET_LIFETIME);
    assert!(is_unauthenticated(gate.exchange(&expired)));

    let unknown = LaunchTicket::draw().unwrap().encoded().to_string();
    assert!(is_unauthenticated(gate.exchange(&unknown)));

    let retired = ticket(&credentials);
    credentials.retire();
    assert!(is_unauthenticated(gate.exchange(&retired)));

    assert!(is_unauthenticated(gate.exchange("not a ticket")));
}

// LA-17: a ticket is still good just before a minute after it was issued.
#[test]
fn a_ticket_is_good_for_a_minute() {
    let (credentials, gate, clock) = credentials();
    let ticket = ticket(&credentials);
    clock.advance(TICKET_LIFETIME - Duration::from_millis(1));
    assert!(gate.exchange(&ticket).is_ok());
}

// LA-17: of concurrent exchanges of one ticket, exactly one succeeds.
#[test]
fn of_concurrent_exchanges_exactly_one_succeeds() {
    // Within the exchange limit's burst, so that every refusal is the
    // ticket's and none the limit's.
    const EXCHANGES: usize = 16;

    let (credentials, gate, _) = credentials();
    let ticket = Arc::new(ticket(&credentials));
    let start = Arc::new(Barrier::new(EXCHANGES));

    let outcomes: Vec<_> = (0..EXCHANGES)
        .map(|_| {
            let (gate, ticket, start) = (gate.clone(), Arc::clone(&ticket), Arc::clone(&start));
            thread::spawn(move || {
                start.wait();
                gate.exchange(&ticket)
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|exchange| exchange.join().expect("an exchange does not panic"))
        .collect();

    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    assert!(outcomes
        .into_iter()
        .filter(Result::is_err)
        .all(is_unauthenticated));
}

// LA-17: past four live tickets, issuing expires the oldest unexchanged one.
#[test]
fn the_ticket_cap_expires_the_oldest_unused_ticket() {
    let (credentials, gate, _) = credentials();

    // One exchanged early, so that it is not among the live ones.
    let exchanged = ticket(&credentials);
    gate.exchange(&exchanged).unwrap();

    let live: Vec<String> = (0..TICKET_CAP).map(|_| ticket(&credentials)).collect();
    let newest = ticket(&credentials);

    assert!(is_unauthenticated(gate.exchange(&live[0])));
    for ticket in &live[1..] {
        assert!(gate.exchange(ticket).is_ok());
    }
    assert!(gate.exchange(&newest).is_ok());
}

// LA-18: past eight sessions, an exchange is refused rather than ending one,
// and the ticket it presented is not spent.
#[test]
fn the_session_cap_refuses_a_new_session_rather_than_evicting_one() {
    let (credentials, gate, _) = credentials();
    let tokens: Vec<String> = (0..SESSION_CAP)
        .map(|_| token(&credentials, &gate))
        .collect();

    let ticket = ticket(&credentials);
    assert!(matches!(
        gate.exchange(&ticket),
        Err(ExchangeRefused::SessionsFull)
    ));
    for token in &tokens {
        assert!(gate.authenticate(token).is_ok());
    }

    let first = gate.authenticate(&tokens[0]).unwrap();
    assert!(gate.revoke(&first));
    assert!(gate.exchange(&ticket).is_ok());
}

// LA-18: an expired session makes room for a new one.
#[test]
fn an_expired_session_does_not_count_against_the_cap() {
    let (credentials, gate, clock) = credentials();
    for _ in 0..SESSION_CAP {
        token(&credentials, &gate);
    }
    clock.advance(SESSION_LIFETIME);
    token(&credentials, &gate);
}

// LA-17: exchanges past the burst are refused with when to come back, and
// are let through again once that time has passed.
#[test]
fn the_exchange_limit_refuses_past_its_burst_and_recovers() {
    let (credentials, gate, clock) = credentials();
    let unknown = LaunchTicket::draw().unwrap().encoded().to_string();

    for _ in 0..20 {
        assert!(is_unauthenticated(gate.exchange(&unknown)));
    }
    let ticket = ticket(&credentials);
    let Err(ExchangeRefused::RateLimited { retry_after }) = gate.exchange(&ticket) else {
        panic!("the 21st exchange is past the burst");
    };
    assert_eq!(retry_after, Duration::from_millis(100));

    // The refused exchange did not spend the ticket.
    clock.advance(retry_after);
    assert!(gate.exchange(&ticket).is_ok());
    assert!(matches!(
        gate.exchange(&unknown),
        Err(ExchangeRefused::RateLimited { .. })
    ));

    clock.advance(Duration::from_secs(2));
    for _ in 0..20 {
        assert!(is_unauthenticated(gate.exchange(&unknown)));
    }
}

// LA-18: a token authenticates until its session expires, and not after.
#[test]
fn a_token_authenticates_until_it_expires() {
    let (credentials, gate, clock) = credentials();
    let token = token(&credentials, &gate);

    clock.advance(SESSION_LIFETIME - Duration::from_secs(1));
    let session = gate.authenticate(&token).expect("not yet expired");
    assert_eq!(session.expires_in(), Duration::from_secs(1));

    clock.advance(Duration::from_secs(1));
    assert!(matches!(gate.authenticate(&token), Err(Unauthenticated)));
}

// LA-18: a session revoked by its page admits nobody, and no other session
// goes with it.
#[test]
fn a_token_authenticates_until_its_session_is_revoked() {
    let (credentials, gate, _) = credentials();
    let revoked = token(&credentials, &gate);
    let other = token(&credentials, &gate);

    let session = gate.authenticate(&revoked).unwrap();
    assert!(gate.revoke(&session));
    assert!(!gate.revoke(&session));

    assert!(matches!(gate.authenticate(&revoked), Err(Unauthenticated)));
    assert!(gate.authenticate(&other).is_ok());
}

// LA-18: the app revokes every session at once, and the tickets not yet
// exchanged with them.
#[test]
fn a_token_authenticates_until_every_session_is_revoked() {
    let (credentials, gate, _) = credentials();
    let tokens = [token(&credentials, &gate), token(&credentials, &gate)];
    let pending = ticket(&credentials);
    let instance_id = credentials.instance_id();

    credentials.revoke_all();

    for token in &tokens {
        assert!(matches!(gate.authenticate(token), Err(Unauthenticated)));
    }
    assert!(is_unauthenticated(gate.exchange(&pending)));
    assert_eq!(credentials.instance_id(), instance_id);
    assert!(gate.exchange(&ticket(&credentials)).is_ok());
}

// LA-18: a retired generation's tokens admit nobody, and a session of it
// cannot reach into the next one.
#[test]
fn a_token_authenticates_until_its_generation_is_retired() {
    let (credentials, gate, _) = credentials();
    let token = token(&credentials, &gate);
    let session = gate.authenticate(&token).unwrap();
    let before = credentials.instance_id();

    credentials.retire();

    assert!(matches!(gate.authenticate(&token), Err(Unauthenticated)));
    assert!(!gate.revoke(&session));
    let after = gate.instance_id();
    assert_ne!(after, before);

    let exchanged = gate.exchange(&ticket(&credentials)).unwrap();
    assert_eq!(exchanged.instance_id, after);
}

// LA-14: what is not a token at all is the same refusal as a token that is
// not held.
#[test]
fn a_malformed_token_is_refused_like_an_unknown_one() {
    let (_, gate, _) = credentials();
    let unknown = BrowserToken::draw().unwrap().encoded().to_string();
    assert!(matches!(gate.authenticate(&unknown), Err(Unauthenticated)));
    assert!(matches!(gate.authenticate(""), Err(Unauthenticated)));
    assert!(matches!(
        gate.authenticate("Bearer x"),
        Err(Unauthenticated)
    ));
}

// LA-18: two hosts never share an instance_id.
#[test]
fn two_hosts_have_instance_ids_of_their_own() {
    let (one, _, _) = credentials();
    let (other, _, _) = credentials();
    assert_ne!(one.instance_id(), other.instance_id());
}

// LA-15: a transition that panicked leaves nothing that admits.
#[test]
fn a_poisoned_lock_fails_closed() {
    let (credentials, gate, _) = credentials();
    let token = token(&credentials, &gate);
    let before = credentials.instance_id();

    let shared = Arc::clone(&credentials.shared);
    thread::spawn(move || {
        let _held = shared.state.lock().unwrap();
        panic!("a transition panics while it holds the lock");
    })
    .join()
    .unwrap_err();

    assert!(matches!(gate.authenticate(&token), Err(Unauthenticated)));
    assert_ne!(credentials.instance_id(), before);
    assert!(gate.exchange(&ticket(&credentials)).is_ok());
}
