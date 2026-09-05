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
//! [`encode_journal_record`] writes what a commit records (FM-15),
//! [`encode_index_snapshot`] writes the Index of a whole Library (FM-16), and
//! [`encode_keyring`] writes the mapping every replica of a Keyring generation
//! carries (FM-17), each producing the [`ControlPayload`] the framing seals.
//! [`keyring_set_digest`] is the one value a payload does not carry: the digest
//! a replica's name and a commit's selection both name the mapping by.
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
//! [`RecoveryCode`] is the one form key material takes outside a machine
//! altogether: the Master Key and its epoch as a checksummed string short
//! enough to write on paper and type into the next device (spec: KD-11).
//!
//! Not everything drawn here is key material either: [`generate_library_id`]
//! draws the identifier a Library's app folder is named after (spec: FM-18)
//! from the same CSPRNG.
//!
//! The crate does no I/O of any kind: [`encode()`] takes in-memory entry content
//! and returns bytes, [`decode()`] takes bytes and returns entry content, and
//! every other entry point here is likewise bytes in, bytes out. Internally
//! those two walk the plaintext stream one chunk at a time, and [`decode()`]
//! authenticates a chunk before any of its bytes reach the caller's buffers.
//!
//! [`ContainerOutline`] is [`decode()`] for a Container nobody wants to hold, or
//! wants only one Entry out of. A Container's shape is settled by its header and
//! meta section, so reading those few kilobytes off the front says where every
//! Entry's bytes are; [`ChunkRun`] turns an Entry's extent into the chunks that
//! cover it, and [`ChunkRunReader`] opens exactly those as their ciphertext
//! arrives, chunk by chunk. It is what lets one page be read out of a Pack
//! without fetching the Pack (spec: PK-16), and what lets a whole Container be
//! decoded to disk without ever being in memory.
//!
//! [`ContainerWriter`] is [`encode()`] for a Container nobody wants to hold: it
//! is told what each Entry will be — [`EntryPlan`] carries the size and the hash
//! [`encode()`] would derive — writes the header and the entry table at once,
//! and then takes the content in whatever pieces the caller has it in, emitting
//! ciphertext as it goes. The bytes are [`encode()`]'s exactly. It exists for
//! Packs, which the pack policy sizes around a target measured in gigabytes
//! (spec: PK-5), and [`ContainerFootprint`] is the measurement that target is
//! compared against (spec: PK-6).
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
//!     EntryPath::parse("photos/spring.jpg")?,
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

// The bound FM-19 puts on every unsigned integer the format carries, stated
// once for every serde-deserialized wire map that holds one — the meta
// section's and the entry map a control payload carries alike.
mod bounded_uint;

mod chunk_size;
pub use chunk_size::ChunkSize;

// Where this crate's tests turn a literal number into a ciphertext length
// claim, beside the modules that do the same for a generation, an Entry Path,
// and an extent and for the same reason.
#[cfg(test)]
mod ciphertext_len_claims;

mod container_footprint;
pub use container_footprint::ContainerFootprint;

mod container_id;
pub use container_id::generate_container_id;

mod container_key;
pub use container_key::generate_container_key;

mod container_reader;
pub use container_reader::{ChunkRun, ChunkRunReader, ContainerOutline};

mod container_writer;
pub use container_writer::ContainerWriter;

mod control;
pub use control::{
    decode_control_object, decode_index_snapshot, decode_journal_record, decode_keyring,
    encode_control_object, encode_index_snapshot, encode_journal_record, encode_keyring,
    keyring_set_digest, max_control_object_len, max_control_object_len_at, ControlEncodeRequest,
    ControlHeader, ControlPayload, DecodedControlObject, EncodedControlObject,
    IndexSnapshotPayload, SnapshotActivation,
};

mod decode;
pub use decode::decode;

mod decoded_container;
pub use decoded_container::DecodedContainer;

mod decoded_entry;
pub use decoded_entry::DecodedEntry;

mod encode;
pub use encode::encode;

mod encode_plan;
pub use encode_plan::EncodePlan;

mod encode_request;
pub use encode_request::EncodeRequest;

mod encoded_container;
pub use encoded_container::EncodedContainer;

mod entropy;

// Where this crate's tests turn a literal pair of numbers into an Entry's
// extent, beside the module that does the same for an Entry Path and for the
// same reason.
#[cfg(test)]
mod entry_extents;

// Where this crate's tests turn a literal into an Entry Path, in one place
// so that a mistyped fixture is reported as the fixture mistake it is.
#[cfg(test)]
mod entry_paths;

mod entry_plan;
pub use entry_plan::EntryPlan;

mod entry_source;
pub use entry_source::EntrySource;

mod error;
pub use error::{Error, Result};

// Where this crate's tests turn a literal number into a generation, beside the
// modules that do the same for a ciphertext length claim, an Entry Path, and an
// extent and for the same reason.
#[cfg(test)]
mod generations;

mod header;
pub use header::Header;

mod key_envelope;
pub use key_envelope::{unwrap_container_key, wrap_container_key};

mod layout;

mod library_id;
pub use library_id::generate_library_id;

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

mod recovery_code;
pub use recovery_code::RecoveryCode;

mod stored_master_key;
pub use stored_master_key::{Argon2Params, StoredMasterKey, UnlockedMasterKey};

mod stream;

// How an Entry's place in a plaintext stream is built, in one place: the model
// states the rule and this crate states the refusal, for a table being laid out
// and for one being read back alike.
mod stream_extent;

mod token_cache;
pub use token_cache::{decode_token_cache, encode_token_cache};
