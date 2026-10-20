use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest as _, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::{is_base64url, ENCODED_LEN, SECRET_BYTES};

/// The SHA-256 digest of a ticket or a token: all the host holds of either
/// (spec: LA-20).
///
/// A digest is not a credential — nobody can present one — but it is held as
/// though it were: wiped when dropped, not `Clone`, compared without stopping
/// at the first byte that differs, and redacted in `Debug` and `Display`, so
/// that it never becomes an identifier an event could carry (spec: EL-1).
pub struct Digest(pub(super) [u8; 32]);

impl Digest {
    pub(super) fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// The digest of a credential as it was presented, or `None` where what
    /// was presented is not 43 characters of unpadded base64url standing for
    /// 32 bytes (spec: LA-20).
    ///
    /// The decoded bytes are wiped before this returns; only the digest is
    /// kept. A presented string that is not one is not decoded at all.
    pub fn of_presented(presented: &str) -> Option<Self> {
        let presented = presented.as_bytes();
        if presented.len() != ENCODED_LEN || !presented.iter().all(|&b| is_base64url(b)) {
            return None;
        }
        // Room for the decoder's own estimate, so that it never grows the
        // buffer and leaves an unwiped copy behind in the allocation it left.
        let mut decoded = Zeroizing::new(Vec::with_capacity(ENCODED_LEN));
        URL_SAFE_NO_PAD.decode_vec(presented, &mut decoded).ok()?;
        (decoded.len() == SECRET_BYTES).then(|| Self::of(&decoded))
    }

    /// Whether two digests are the same, compared without stopping at the
    /// first byte that differs (spec: LA-20).
    pub fn same_as(&self, other: &Self) -> bool {
        self.0
            .iter()
            .zip(&other.0)
            .fold(0_u8, |sofar, (a, b)| sofar | (a ^ b))
            == 0
    }
}

redacted!(Digest, "Digest(<redacted>)", "<redacted digest>");

impl Drop for Digest {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for Digest {}
