//! Control objects: the framing every kind of Library bookkeeping shares.
//!
//! A control object is a 44-byte plaintext header and one AEAD message: the
//! payload, encrypted under the purpose key of the header's kind, with the whole
//! header as associated data and the header's random nonce. Journal records,
//! Keyring replicas, and Index Snapshots — ordinary and activation — differ only
//! in their kind byte, their purpose key, and the fields inside the payload map;
//! a future kind is a new kind byte and a new info string, not a new framing.
//!
//! What an object is stored *as* is its kind, and it rides in the authenticated
//! header. What it is stored *for* is its
//! [`ControlObjectName`](coffret_model::ControlObjectName), which the framing
//! only checks against the header through FM-12's admission table: one name form
//! covers the whole control-head chain, so a name determines no kind.
//!
//! This module owns the framing and one payload field, `master_key_epoch`, which
//! every control object carries whatever its kind. The rest of a payload is the
//! kind's own schema: [`journal_record`] is FM-15's, [`index_snapshot`] is
//! FM-16's, and [`keyring`] is FM-17's, each a module of its own beside the
//! framing rather than inside it.
//!
//! What those schemas share sits here as well: the map a Container is recorded
//! with, which an addition and a Snapshot's `containers` element spell
//! identically; the CBOR readers that name the field a payload went wrong at;
//! and the canonical orders every array in a payload is written in.

mod canonical_order;
mod cbor;
mod wire_container;

mod decode;
pub use decode::decode_control_object;

mod decoded_object;
pub use decoded_object::DecodedControlObject;

mod encode;
pub use encode::encode_control_object;

mod encode_request;
pub use encode_request::ControlEncodeRequest;

mod encoded_object;
pub use encoded_object::EncodedControlObject;

mod header;
pub use header::ControlHeader;

mod index_snapshot;
pub use index_snapshot::{
    decode as decode_index_snapshot, encode as encode_index_snapshot, IndexSnapshotPayload,
    SnapshotActivation,
};

mod journal_record;
pub use journal_record::{decode as decode_journal_record, encode as encode_journal_record};

mod keyring;
pub use keyring::{
    decode as decode_keyring, encode as encode_keyring, set_digest as keyring_set_digest,
};

mod payload;
pub use payload::ControlPayload;

#[cfg(test)]
mod rejection_tests;
#[cfg(test)]
mod round_trip_tests;

#[cfg(test)]
mod testing;
