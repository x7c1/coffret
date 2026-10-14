//! The two secrets the host hands out, and the digest it keeps of each.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::NotDrawn;

/// Writes the fixed redacted rendering a secret type has for both `Debug` and
/// `Display`, so that neither can be formatted into anything but its name.
macro_rules! redacted {
    ($type:ident, $debug:literal, $display:literal) => {
        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str($debug)
            }
        }

        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str($display)
            }
        }
    };
}

mod browser_token;
pub use browser_token::BrowserToken;

mod digest;
pub(super) use digest::Digest;

mod launch_ticket;
pub use launch_ticket::LaunchTicket;

#[cfg(test)]
mod tests;

/// How many bytes of the operating system's CSPRNG a ticket or a token is.
const SECRET_BYTES: usize = 32;

/// How many characters those bytes are as unpadded base64url.
const ENCODED_LEN: usize = 43;

/// Thirty-two bytes drawn from the operating system's CSPRNG: the body of a
/// [`LaunchTicket`] and of a [`BrowserToken`] alike (spec: LA-20).
///
/// Not `Clone`, and it overwrites its bytes when it is dropped. It is not on
/// the secret-bearing inventory, which is what carries the Master Key, is
/// derived from it or wrapped under it, or unlocks it (spec: DK-7); it is held
/// the same way because it admits a browser to the Library the host serves.
struct Secret([u8; SECRET_BYTES]);

impl Secret {
    fn draw() -> Result<Self, NotDrawn> {
        let mut bytes = [0_u8; SECRET_BYTES];
        getrandom::fill(&mut bytes).map_err(NotDrawn)?;
        Ok(Self(bytes))
    }

    fn encoded(&self) -> Zeroizing<String> {
        Zeroizing::new(URL_SAFE_NO_PAD.encode(self.0))
    }

    fn digest(&self) -> Digest {
        Digest::of(&self.0)
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for Secret {}

/// The unpadded base64url alphabet (RFC 4648, section 5).
const fn is_base64url(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}
