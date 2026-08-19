//! The nonces of coffret's AEAD messages.
//!
//! Inside a Container they are deterministic: `domain(1) ‖ counter(8,
//! big-endian) ‖ zero(15)`. One Container Key encrypts exactly one Container, so
//! these never repeat under a key, and the counter plus the separate
//! final-chunk domain make reordering, truncation, and extension of the chunk
//! sequence fail authentication.
//!
//! The messages that are not part of a Container — control-object payloads, Key
//! Envelopes, a device's stored Master Key — have no such counter to hang a
//! domain off, and their keys each cover many messages, so they draw a
//! [`random`] nonce and carry it in the object.

use crate::entropy;
use crate::error::Result;

/// Length of an XChaCha20-Poly1305 nonce in bytes.
pub(crate) const LEN: usize = 24;

/// The meta section.
const DOMAIN_META: u8 = 0x01;
/// A chunk with more chunks after it.
const DOMAIN_CHUNK: u8 = 0x02;
/// The last chunk of the stream.
const DOMAIN_FINAL_CHUNK: u8 = 0x03;

/// The nonce of the meta section, whose counter is always zero.
pub(crate) fn meta() -> [u8; LEN] {
    build(DOMAIN_META, 0)
}

/// The nonce of the chunk at `index`, counted from 0 across all chunks.
pub(crate) fn chunk(index: u64, is_final: bool) -> [u8; LEN] {
    let domain = if is_final {
        DOMAIN_FINAL_CHUNK
    } else {
        DOMAIN_CHUNK
    };
    build(domain, index)
}

/// Draws a fresh nonce from the operating system's CSPRNG.
///
/// 24 bytes is wide enough that random nonces never practically collide, which
/// is what lets one purpose key cover an unbounded number of objects.
pub(crate) fn random() -> Result<[u8; LEN]> {
    entropy::draw()
}

fn build(domain: u8, counter: u64) -> [u8; LEN] {
    let mut nonce = [0u8; LEN];
    nonce[0] = domain;
    nonce[1..9].copy_from_slice(&counter.to_be_bytes());
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    // FM-7: the nonce is domain(1) ‖ counter(8, big-endian) ‖ zero(15), with
    // domain 0x01 for the meta section (counter 0), 0x02 for a non-final chunk,
    // and 0x03 for the final chunk.
    #[test]
    fn layout_matches_the_rule() {
        assert_eq!(
            meta(),
            [0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            chunk(1, false),
            [0x02, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            chunk(1, true),
            [0x03, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(&chunk(u64::MAX, false)[1..9], &u64::MAX.to_be_bytes());
    }

    // FM-7: a chunk's position and its final-or-not role both change the nonce,
    // which is what makes reordering and truncation detectable.
    #[test]
    fn every_role_gets_a_distinct_nonce() {
        let nonces = [
            meta(),
            chunk(0, false),
            chunk(0, true),
            chunk(1, false),
            chunk(1, true),
        ];
        for (i, left) in nonces.iter().enumerate() {
            for right in &nonces[i + 1..] {
                assert_ne!(left, right);
            }
        }
    }

    // FM-11, FM-14: the messages outside a Container carry a random 24-byte
    // nonce, so two of them never share one under the same key.
    #[test]
    fn random_nonces_are_distinct_and_full_width() {
        let nonces: std::collections::HashSet<[u8; LEN]> = (0..256)
            .map(|_| random().expect("the OS CSPRNG is available"))
            .collect();
        assert_eq!(nonces.len(), 256);
        for nonce in &nonces {
            assert_eq!(nonce.len(), 24);
        }
    }
}
