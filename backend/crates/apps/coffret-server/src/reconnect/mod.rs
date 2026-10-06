//! Renewing the grant a running server reaches Storage through, from the
//! explorer.
//!
//! A Drive grant issued by a consent screen in testing runs out after seven
//! days, and when it does every reach for Storage is refused as
//! `unauthenticated`. The command line's answer is `coffret authorize`, which
//! asks for the Passphrase to open the Library and reach the account's grant.
//! A running server has no Passphrase to ask for and needs none: the Library is
//! open, and the account-cache key the renewal writes under is the one its own
//! store already reads with (spec: KD-12). So this runs the same consent flow
//! inside the server (spec: SA-1, SA-2) — a loopback listener the server owns,
//! the grant verified before anything is cached (spec: SA-4), and the
//! account's one cache replaced (spec: SA-6) — and the page is handed the
//! consent page's URL to open, and nothing else.
//!
//! Nothing has to be reopened once the grant is renewed, for the reason
//! [`DriveGrant::renew`](coffret_device::DriveGrant::renew) gives; what this
//! does afterwards is catch the catalog up (spec: CK-9), because the catch-up
//! the grant running out refused is the thing a person was looking at when
//! they pressed the button.
//!
//! # One flow at a time
//!
//! A second press while a flow waits answers with the URL the first one
//! produced rather than binding a second listener: two consent pages open for
//! one account would leave whichever was answered second replacing the grant
//! the first one cached, and a person who answered one of two tabs could not
//! tell which.
//!
//! # What the explorer is told
//!
//! What the flow came to rides in the work answer the explorer already polls,
//! as [`Reconnect`]: waiting, renewed, refused by the person, unanswered, or
//! failed. It is this process's own account and nothing in it is a secret: no
//! token, no code, and not even the URL — that goes back only to the request
//! that asked for it.

use coffret_device::Unrenewed;

mod consent;
pub use consent::{Consent, ConsentFlow};

mod drive_consent;
pub use drive_consent::DriveConsent;

mod reconnects;
pub use reconnects::Reconnects;

mod run;
pub(crate) use run::start;

/// Where the last reconnect stands.
///
/// The whole set is named here, because a browser writes a branch per state and
/// one it has never heard of is one it falls off the end of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reconnect {
    /// A consent page is open and the server's loopback listener is waiting for
    /// the browser to come back from it.
    ///
    /// The URL is kept so that a second press answers with the same page rather
    /// than a second flow.
    Waiting {
        /// The consent page the flow is waiting on.
        url: String,
    },
    /// The grant was renewed and cached, and the catalog has been asked to
    /// catch up with it.
    Renewed,
    /// The person declined on the consent page.
    Refused,
    /// Nobody came back from the consent page before the flow stopped waiting.
    TimedOut,
    /// The flow ended some other way, which the log says.
    Failed,
}

impl From<Unrenewed> for Reconnect {
    fn from(unrenewed: Unrenewed) -> Self {
        match unrenewed {
            Unrenewed::Refused => Self::Refused,
            Unrenewed::TimedOut => Self::TimedOut,
            Unrenewed::Failed => Self::Failed,
        }
    }
}
