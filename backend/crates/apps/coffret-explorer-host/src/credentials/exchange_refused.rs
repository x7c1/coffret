use std::fmt;
use std::time::Duration;

use super::NotDrawn;

/// Why an exchange of a launch ticket answered no browser token.
#[derive(Debug)]
pub enum ExchangeRefused {
    /// The ticket was used, expired, never issued, issued before the last
    /// [`retire`](super::Credentials::retire), or not a ticket at all. One
    /// variant for all of them, so that no answer can tell them apart
    /// (spec: LA-17).
    Unauthenticated,
    /// More exchanges arrived across the host than its limit lets through;
    /// the next is let through after `retry_after` (spec: LA-17). Checked
    /// before the ticket is looked at, so a refused exchange spends nothing.
    RateLimited { retry_after: Duration },
    /// The ticket is good, but the host already holds as many sessions as it
    /// may; no session is ended to make room (spec: LA-18). The ticket is not
    /// spent, so a page that revokes a session elsewhere may try again
    /// before it expires.
    SessionsFull,
    /// The ticket is good, but no token could be drawn for it. The ticket is
    /// not spent.
    NotDrawn(NotDrawn),
}

impl fmt::Display for ExchangeRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthenticated => f.write_str("the launch ticket is not one this host holds"),
            Self::RateLimited { .. } => {
                f.write_str("more launch tickets were exchanged than this host lets through")
            }
            Self::SessionsFull => f.write_str("this host already holds as many sessions as it may"),
            Self::NotDrawn(_) => f.write_str("no browser token could be drawn"),
        }
    }
}

impl std::error::Error for ExchangeRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotDrawn(cause) => Some(cause),
            Self::Unauthenticated | Self::RateLimited { .. } | Self::SessionsFull => None,
        }
    }
}
