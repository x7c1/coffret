//! The payload of a Journal record (FM-15).
//!
//! A record is the commit point of a batch (CP-1), and its payload is the whole
//! of what a device needs to replay that commit without opening a Container:
//! the Keyring tuple the commit selected (CP-10), the two slots the head
//! reserves (CP-2, CK-10), the Containers the batch added with their entry
//! tables (CP-11), and the Container IDs it removed (CP-14).
//!
//! Two of the record's fields are not here, because the framing already carries
//! them and one state must not have two answers: the record's own generation is
//! the control-object header's (FM-11), and `master_key_epoch` is the payload
//! field FM-13 gives every kind. [`encode`] therefore hands back a whole
//! [`ControlPayload`](crate::ControlPayload), and [`decode`] is told the
//! generation the header carried.
//!
//! Putting `additions` and `removals` in Container ID order is this module's
//! job, and checking that order is its reader's: see
//! [`canonical_order`](super::canonical_order) for why a decoder rejects a
//! payload rather than sorting it.

mod encode;
pub use encode::encode;

mod decode;
pub use decode::decode;

#[cfg(test)]
mod rejection_tests;
#[cfg(test)]
mod round_trip_tests;

#[cfg(test)]
mod testing;

/// The schema this crate writes for a Journal record payload (FM-15).
const SCHEMA: u64 = 1;

const PREV: &str = "prev";
const NEXT_COMMIT_SLOT: &str = "next_commit_slot";
const SNAPSHOT_SLOT: &str = "snapshot_slot";
const KEYRING_GENERATION: &str = "keyring_generation";
const KEYRING_REPLICA_COUNT: &str = "keyring_replica_count";
const KEYRING_SET_DIGEST: &str = "keyring_set_digest";
const ADDITIONS: &str = "additions";
const ENTRIES: &str = "entries";
const REMOVALS: &str = "removals";
