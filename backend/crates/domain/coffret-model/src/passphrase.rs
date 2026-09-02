use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// What a person types to unlock this device's Master Key.
///
/// The first entry in the secret-bearing inventory (see
/// [`master_key`](crate::MasterKey)'s module): a Passphrase is not key material
/// in the sense the Master Key is — nothing is encrypted under it directly —
/// but Argon2id turns it into the key that opens the stored form (spec: KD-5),
/// so a copy of it left in freed memory is a copy of the Library's key by one
/// cheap derivation.
///
/// It owns its bytes so that reading one and wiping one are the same act: a
/// terminal hands over a `String` or a `Vec<u8>`, this takes it whole rather
/// than copying out of it, and the buffer that arrives is the buffer that gets
/// overwritten. `Debug` is redacted, there is no `Display`, and it is not
/// `Clone` — a Passphrase is spent once, by the one unlock that needs it.
///
/// Comparison is `PartialEq` on the raw bytes and is deliberately *not*
/// constant-time. The one comparison in the codebase is a person confirming a
/// Passphrase they have just chosen, against the one they typed a moment
/// earlier, on their own terminal; there is no remote party to time it. What
/// checks a Passphrase against a stored form is the AEAD tag, which is
/// constant-time in the cipher.
#[derive(PartialEq, Eq)]
pub struct Passphrase(Vec<u8>);

impl Passphrase {
    /// Takes the bytes a terminal or a script produced.
    ///
    /// By value, so that the caller has nothing left to wipe: a `String` read
    /// from a prompt becomes these bytes through `into_bytes`, which keeps the
    /// allocation rather than copying it.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The raw bytes, as Argon2id reads them.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Whether nothing at all was typed.
    ///
    /// The one refusal every way of giving a Passphrase shares: an empty one
    /// protects nothing, and a stored form written under it is a stored form
    /// anybody opens.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Passphrase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Passphrase(<redacted>)")
    }
}

impl Drop for Passphrase {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for Passphrase {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_what_was_typed() {
        let passphrase = Passphrase::from_bytes(b"correct horse".to_vec());
        assert_eq!(format!("{passphrase:?}"), "Passphrase(<redacted>)");
    }

    #[test]
    fn an_empty_passphrase_says_so() {
        assert!(Passphrase::from_bytes(Vec::new()).is_empty());
        assert!(!Passphrase::from_bytes(b"a".to_vec()).is_empty());
    }

    // DK-7, checked the way the keys' is: on the operation the drop runs.
    #[test]
    fn the_drop_time_wipe_overwrites_what_was_typed() {
        let mut passphrase = Passphrase::from_bytes(b"correct horse".to_vec());
        let length = passphrase.as_bytes().len();
        passphrase.0.zeroize();
        // `Vec::zeroize` wipes the bytes and then clears the length, so what is
        // left is an empty buffer rather than a run of typed characters.
        assert!(passphrase.is_empty());
        assert_ne!(length, 0);
    }
}
