//! Format v1: the byte form of every coffret Storage Object, and the keys that
//! open them.
//!
//! A Container — the object user data travels in — is a 32-byte plaintext
//! header, an encrypted CBOR meta section, and a sequence of separately
//! encrypted chunks. All of it is XChaCha20-Poly1305 under one Container Key,
//! with deterministic nonces and the header bound in as associated data, so
//! reordering, truncating, extending, or editing any part of the object fails
//! authentication.
//!
//! A control object — a Journal record, a Keyring replica, an Index Snapshot
//! ordinary or epoch-activating — is a 44-byte plaintext header and one AEAD
//! message under the purpose key of its kind; see [`encode_control_object`]. The
//! name it is stored under is
//! [`coffret_model::ControlObjectName`], and it says what the object is for
//! rather than what it is, so the encoder is told the kind outright.
//!
//! What rides inside that message is the kind's own schema:
//! [`encode_journal_record`] writes what a commit records (FM-15) and
//! [`encode_index_snapshot`] writes the Index of a whole Library (FM-16), each
//! producing the [`ControlPayload`] the framing seals.
//!
//! The keys come from one Master Key: [`PurposeKey`] derives a key per
//! [`Purpose`], [`wrap_container_key`] wraps a Container Key into the envelope
//! the Keyring stores, and [`StoredMasterKey`] is the form a device keeps its
//! Master Key in under a Passphrase.
//!
//! Not everything the keys protect is a Storage Object: [`encode_token_cache`]
//! seals the OAuth token cache a device keeps for a Storage provider, which
//! never leaves the device but is a credential for everything on Storage that
//! does.
//!
//! The crate does no I/O of any kind: [`encode`] takes in-memory entry content
//! and returns bytes, [`decode`] takes bytes and returns entry content, and
//! every other entry point here is likewise bytes in, bytes out. Internally
//! those two walk the plaintext stream one chunk at a time, and [`decode`]
//! authenticates a chunk before any of its bytes reach the caller's buffers.
//!
//! ```
//! use coffret_format::{decode, encode, EncodeRequest, EntrySource};
//! use coffret_model::{ContainerKey, ContainerKind, EntryPath, Mtime};
//!
//! # fn main() -> coffret_format::Result<()> {
//! // The Container Key comes from a CSPRNG in real use; one Key opens exactly
//! // one Container.
//! let key = ContainerKey::from_bytes([0x42; ContainerKey::BYTE_LEN]);
//! let entries = [EntrySource::new(
//!     EntryPath::new("photos/spring.jpg"),
//!     Mtime::from_unix_seconds(1_700_000_000),
//!     b"the file's bytes",
//! )];
//!
//! let request = EncodeRequest::new(
//!     coffret_format::generate_container_id()?,
//!     ContainerKind::OneFile,
//!     &key,
//!     &entries,
//! );
//! let container = encode(&request)?;
//! assert!(container.object_name().ends_with(".cfrt"));
//!
//! let opened = decode(container.bytes(), &key)?;
//! assert_eq!(opened.entries[0].content, b"the file's bytes");
//! assert_eq!(opened.entries[0].metadata.path.as_str(), "photos/spring.jpg");
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod aead;

mod chunk_size;
pub use chunk_size::ChunkSize;

mod container_id;
pub use container_id::generate_container_id;

mod container_key;
pub use container_key::generate_container_key;

mod control;
pub use control::{
    decode_control_object, decode_index_snapshot, decode_journal_record, encode_control_object,
    encode_index_snapshot, encode_journal_record, ControlEncodeRequest, ControlHeader,
    ControlPayload, DecodedControlObject, EncodedControlObject, IndexSnapshotPayload,
    SnapshotActivation,
};

mod decode;
pub use decode::decode;

mod decoded_container;
pub use decoded_container::DecodedContainer;

mod decoded_entry;
pub use decoded_entry::DecodedEntry;

mod encode;
pub use encode::encode;

mod encode_request;
pub use encode_request::EncodeRequest;

mod encoded_container;
pub use encoded_container::EncodedContainer;

mod entropy;

mod entry_source;
pub use entry_source::EntrySource;

mod error;
pub use error::{Error, Result};

mod header;
pub use header::Header;

mod key_envelope;
pub use key_envelope::{unwrap_container_key, wrap_container_key};

mod master_key;
pub use master_key::generate_master_key;

mod meta;

mod nonce;

mod padme;
pub use padme::padded_len;

mod purpose;
pub use purpose::Purpose;

mod purpose_key;
pub use purpose_key::PurposeKey;

mod stored_master_key;
pub use stored_master_key::{Argon2Params, StoredMasterKey, UnlockedMasterKey};

mod stream;

mod token_cache;
pub use token_cache::{decode_token_cache, encode_token_cache};
