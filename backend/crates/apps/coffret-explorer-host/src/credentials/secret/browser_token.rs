use zeroize::{ZeroizeOnDrop, Zeroizing};

use super::{Digest, NotDrawn, Secret};

/// A browser token, as an exchange hands it to the page (spec: LA-16,
/// LA-18).
///
/// The host keeps only its digest, so this value is the only copy of the
/// token itself. Not `Clone`; wiped when dropped; `Debug` and `Display` are
/// redacted.
pub struct BrowserToken(pub(super) Secret);

impl BrowserToken {
    pub(in crate::credentials) fn draw() -> Result<Self, NotDrawn> {
        Secret::draw().map(Self)
    }

    pub(in crate::credentials) fn digest(&self) -> Digest {
        self.0.digest()
    }

    /// The token as the page sends it after `Bearer `: 43 characters of
    /// unpadded base64url, in a buffer that is wiped when it is dropped
    /// (spec: LA-20).
    pub fn encoded(&self) -> Zeroizing<String> {
        self.0.encoded()
    }
}

redacted!(
    BrowserToken,
    "BrowserToken(<redacted>)",
    "<redacted browser token>"
);

impl ZeroizeOnDrop for BrowserToken {}
