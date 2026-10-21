use zeroize::{ZeroizeOnDrop, Zeroizing};

use super::{Digest, NotDrawn, Secret};

/// A one-time launch ticket, as the host hands it to the process it runs in,
/// which puts it in the fragment of the URL a page is loaded from: the one the
/// desktop app loads its own window from, or the one a launch file refreshes
/// to (spec: LA-17, LA-21).
///
/// The host keeps only its digest, so this value is the only copy of the
/// ticket itself. Not `Clone`; wiped when dropped; `Debug` and `Display` are
/// redacted.
pub struct LaunchTicket(pub(super) Secret);

impl LaunchTicket {
    pub(in crate::credentials) fn draw() -> Result<Self, NotDrawn> {
        Secret::draw().map(Self)
    }

    pub(in crate::credentials) fn digest(&self) -> Digest {
        self.0.digest()
    }

    /// The ticket as the page presents it: 43 characters of unpadded
    /// base64url, in a buffer that is wiped when it is dropped (spec: LA-20).
    pub fn encoded(&self) -> Zeroizing<String> {
        self.0.encoded()
    }
}

redacted!(
    LaunchTicket,
    "LaunchTicket(<redacted>)",
    "<redacted launch ticket>"
);

impl ZeroizeOnDrop for LaunchTicket {}
