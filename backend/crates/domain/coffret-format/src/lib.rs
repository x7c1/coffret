//! Container v1: the byte form of a coffret user-data Storage Object.
//!
//! A Container is a 32-byte plaintext header, an encrypted CBOR meta section,
//! and a sequence of separately encrypted chunks. All of it is
//! XChaCha20-Poly1305 under one Container Key, with deterministic nonces and
//! the header bound in as associated data, so reordering, truncating,
//! extending, or editing any part of the object fails authentication.
//!
//! The crate does no I/O of any kind: [`encode`] takes in-memory entry content
//! and returns bytes, [`decode`] takes bytes and returns entry content.
//! Internally both walk the plaintext stream one chunk at a time, and decoding
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
mod container_id;
mod decode;
mod decoded_container;
mod decoded_entry;
mod encode;
mod encode_request;
mod encoded_container;
mod entry_source;
mod error;
mod header;
mod meta;
mod nonce;
mod padme;
mod stream;

pub use chunk_size::ChunkSize;
pub use container_id::generate_container_id;
pub use decode::decode;
pub use decoded_container::DecodedContainer;
pub use decoded_entry::DecodedEntry;
pub use encode::encode;
pub use encode_request::EncodeRequest;
pub use encoded_container::EncodedContainer;
pub use entry_source::EntrySource;
pub use error::{Error, Result};
pub use header::Header;
pub use padme::padded_len;
