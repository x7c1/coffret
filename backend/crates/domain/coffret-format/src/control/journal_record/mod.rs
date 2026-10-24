//! The payload of a Journal record (spec: FM-15).
//!
//! A record is the commit point of a batch (spec: CP-1), and its payload is the
//! whole of what a device needs to replay that commit without opening a
//! Container: the Keyring tuple the commit selected (spec: CP-10), the two
//! slots the head reserves (spec: CP-2, CK-10), the Containers the batch
//! added with their entry tables (spec: CP-11), the Container IDs it
//! removed (spec: CP-14), and the Entries it gave a new current Entry Path,
//! each named by its Container and entry number (spec: FM-15).
//!
//! Two of the record's fields are not here, because the framing already carries
//! them and one state must not have two answers: the record's own generation is
//! the control-object header's (spec: FM-11), and `master_key_epoch` is the
//! payload field FM-13 gives every kind. [`encode()`] therefore hands back a
//! whole [`ControlPayload`](crate::ControlPayload), and [`decode()`] is told
//! the generation the header carried.
//!
//! Neither half of this module states the order `additions`, `removals`, and
//! `renames` are written in: [`JournalRecord`](coffret_model::JournalRecord)
//! holds them in it, so the encoder writes them out as they stand and the
//! decoder hands what it read to that constructor. A payload out of order is
//! therefore rejected rather than sorted into shape — sorting it would accept
//! two encodings of one state and hide the writer that produced the second.

mod encode;
pub use encode::encode;

mod decode;
pub use decode::decode;

mod rename;

#[cfg(test)]
mod rejection_tests;
#[cfg(test)]
mod round_trip_tests;

#[cfg(test)]
mod testing;

/// The schema this crate writes for a Journal record payload, and the one it
/// implements (spec: FM-15).
///
/// It is also what every record this build writes names in `required_schema`:
/// a reader that ignored `deposited_at` would hold a catalog without deposit
/// times and could commit records no reader of this schema accepts, so the
/// field is must-understand and every record says so (spec: FM-9, FM-15).
///
/// Not what a reader refuses below: that is [`LOWEST_READ_SCHEMA`], so that a
/// later build writing a new field under a higher schema goes on reading the
/// records written before it.
const SCHEMA: u64 = 4;

/// The lowest schema a reader accepts, in `schema` and in `required_schema`
/// alike: 4, the first whose addition entries carry `deposited_at`
/// (spec: FM-15).
///
/// A record of an earlier schema carries Entries without a deposit time, which
/// no catalog this build keeps can hold, so it is refused as a schema this
/// build does not read rather than read with a time made up. Libraries holding
/// such records are recreated, not migrated.
///
/// Equal to [`SCHEMA`] for now.
const LOWEST_READ_SCHEMA: u64 = 4;

const PREV: &str = "prev";
const NEXT_COMMIT_SLOT: &str = "next_commit_slot";
const SNAPSHOT_SLOT: &str = "snapshot_slot";
const KEYRING_GENERATION: &str = "keyring_generation";
const KEYRING_REPLICA_COUNT: &str = "keyring_replica_count";
const KEYRING_SET_DIGEST: &str = "keyring_set_digest";
const ADDITIONS: &str = "additions";
const ENTRIES: &str = "entries";
const REMOVALS: &str = "removals";
const RENAMES: &str = "renames";
const REQUIRED_SCHEMA: &str = "required_schema";
