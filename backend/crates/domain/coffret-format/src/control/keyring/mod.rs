//! The payload of a Keyring replica (FM-17).
//!
//! A Keyring generation records what the committed control state holds for
//! every current Container: its Key Envelope (FM-14), or the explicit key-lost
//! marker saying no envelope is reachable (KL-7). That mapping is the whole of
//! the payload — every replica of one generation carries it identically, which
//! is why reading needs one valid replica and the replica count buys redundancy
//! rather than a quorum (KL-6).
//!
//! Three things a replica states are not in the map, because the framing
//! already carries them and one state must not have two answers: the Keyring's
//! generation and the replica position are the control-object header's (FM-11),
//! and `master_key_epoch` is the payload field FM-13 gives every kind. So
//! [`encode()`] is handed the epoch to seal the mapping under and returns a
//! whole [`ControlPayload`](crate::ControlPayload), and two replicas of one
//! generation differ only in their header and their nonce.
//!
//! [`set_digest()`] is the fourth thing kept out of the map, and the only one
//! kept out for a reason of its own: the digest is taken *over* the mapping, so
//! a field carrying it would make it cover itself. It lives here beside the
//! encoder because the bytes it hashes are the encoder's own `mapping` array —
//! the name a replica is stored under (FM-12), the commitment a commit selects
//! (CP-10, KL-3), and KL-1's validity check all read that one definition.
//!
//! The Container ID order `mapping` is written in is not stated here either:
//! [`KeyringMapping`](coffret_model::KeyringMapping) holds its entries in it, so
//! the encoder writes them out as they stand and the decoder hands what it read
//! to that constructor. A payload out of order is rejected rather than sorted
//! into shape — one mapping has one digest only if one state has one encoding.

mod encode;
pub use encode::encode;

mod decode;
pub use decode::decode;

mod set_digest;
pub use set_digest::set_digest;

#[cfg(test)]
mod rejection_tests;
#[cfg(test)]
mod round_trip_tests;
#[cfg(test)]
mod size_tests;

#[cfg(test)]
mod testing;

/// The schema this crate writes for a Keyring payload (FM-17).
const SCHEMA: u64 = 1;

const MAPPING: &str = "mapping";
const ID: &str = "id";
const ENVELOPE: &str = "envelope";
const KEY_LOST: &str = "key_lost";
