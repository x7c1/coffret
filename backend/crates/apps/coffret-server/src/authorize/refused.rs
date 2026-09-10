use std::borrow::Cow;

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
    /// `Origin` — those are a caller's own text, and a diagnostic event is
    /// read somewhere the request never was.
    ///
    /// The operation is `admit` rather than this module's own name: it is the
    /// field a reader groups a log file by, and `authorize` is already the flow
    /// that renews this device's grant to Storage.
    ///
    /// `authority` is the address this server bound, which one of the three
    /// sentences names. It is an argument rather than a field here so that
    /// [`Refused`] stays what it is: which fence a request did not get past, and
    /// nothing about the server it did not get past it at.
    pub(super) fn recorded(self, authority: &str) -> ApiError {
        warn!(
            operation = "admit",
            refused = self.reason(),
            "a request was refused before any route saw it",
        );
        ApiError::unauthorized(self.message(authority))
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
    ///
    /// The first sentence names the address this server bound, which gives away
    /// nothing: it is an address the caller has already reached, and the request
    /// being refused arrived at it (spec: LA-5). Naming it is what turns a
    /// refusal somebody reads in a browser into something they can act on —
    /// `localhost:8787` and `127.0.0.1:8787` are the same socket and not the
    /// same `Host`, and a person told only that they asked for "a different one"
    /// has no way to see which of the two they are looking at.
    pub(super) fn message(self, authority: &str) -> Cow<'static, str> {
        match self {
            Self::Elsewhere => Cow::Owned(format!(
                "this Library is served at {authority} and nowhere else, and this request named \
                 a different address"
            )),
            Self::Unkeyed => Cow::Borrowed(
                "this Library is served only to whoever can read this device's own files: the \
                 explorer sends the key its server wrote into `server-key`, in this Library's \
                 directory on this device, and this request did not carry it",
            ),
            Self::AnotherSite => Cow::Borrowed(
                "this Library is served to the explorer on this device, and this request came \
                 from a page belonging to another site",
            ),
        }
    }
}
