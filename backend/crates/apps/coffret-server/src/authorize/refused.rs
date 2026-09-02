use tracing::warn;

use crate::api_error::ApiError;

/// Which fence a request did not get past.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refused {
    /// The `Host` names somewhere this server is not.
    Elsewhere,
    /// No key was shown, or not this server's.
    Unkeyed,
    /// A browser said the page asking belongs to another site.
    AnotherSite,
}

impl Refused {
    /// The refusal a caller is answered with, put into the log on the way.
    ///
    /// What is logged is which fence, and nothing the request carried: not the
    /// key that was shown, not the one that was expected, not the `Host` or the
    /// `Origin` — those are a caller's own text, and a log line is read
    /// somewhere the request never was.
    ///
    /// The operation is `admit` rather than this module's own name: it is the
    /// field a reader groups a log file by, and `authorize` is already the flow
    /// that renews this device's grant to Storage.
    pub(super) fn recorded(self) -> ApiError {
        warn!(
            operation = "admit",
            refused = self.reason(),
            "a request was refused before any route saw it",
        );
        ApiError::unauthorized(self.message())
    }

    /// Which fence it was, for whoever reads the log.
    fn reason(self) -> &'static str {
        match self {
            Self::Elsewhere => "host",
            Self::Unkeyed => "key",
            Self::AnotherSite => "site",
        }
    }

    /// The one sentence the caller is answered with.
    ///
    /// A key that was shown and is wrong is answered exactly as no key at all,
    /// so that a caller guessing learns nothing from the answer — not even that
    /// the header is the one to guess at.
    fn message(self) -> &'static str {
        match self {
            Self::Elsewhere => {
                "this Library is served at the address its server was started at, and this \
                 request asked for a different one"
            }
            Self::Unkeyed => {
                "this Library is served only to whoever can read this device's own files: the \
                 explorer sends the key its server wrote into `server-key`, in this Library's \
                 directory on this device, and this request did not carry it"
            }
            Self::AnotherSite => {
                "this Library is served to the explorer on this device, and this request came \
                 from a page belonging to another site"
            }
        }
    }
}
