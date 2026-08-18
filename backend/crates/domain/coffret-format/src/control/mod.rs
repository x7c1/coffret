//! Control objects: the framing every kind of Library bookkeeping shares.
//!
//! A control object is a 44-byte plaintext header and one AEAD message: the
//! payload, encrypted under the purpose key of the header's kind, with the whole
//! header as associated data and the header's random nonce. Journal records,
//! Keyring replicas, and Index Snapshots differ only in their kind byte, their
//! purpose key, and the fields inside the payload map — a future kind is a new
//! kind byte and a new info string, not a new framing.
//!
//! This module owns the framing and one payload field, `master_key_epoch`, which
//! every control object carries whatever its kind. The rest of a payload is the
//! kind's own schema and does not live here.

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

mod object_name;
pub use object_name::ControlObjectName;

mod payload;
pub use payload::ControlPayload;

#[cfg(test)]
mod rejection_tests;
#[cfg(test)]
mod round_trip_tests;

#[cfg(test)]
mod testing;
