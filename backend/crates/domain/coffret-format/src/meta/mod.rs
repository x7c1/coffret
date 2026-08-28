//! The meta section: one CBOR map, encrypted as a single AEAD message.
//!
//! Container-level fields are `schema`, `kind`, `pad_len`, and `entries`; each
//! entry records `path`, `offset`, `size`, `mtime`, and `hash`, plus optional
//! `derived_from` and `mime`. The maps are forward-open — a reader ignores
//! fields it does not know, and adding a field only increments `schema`.
//!
//! The plaintext is that map followed by zero padding up to its Padmé bucket, so
//! the length the header records is not a proxy for how many Entries the
//! Container holds or how long their paths are. CBOR is self-delimiting, so
//! nothing records where the map ends: a reader takes one item and then checks
//! that the rest of the plaintext is zero.
//!
//! The CBOR shape lives in the `wire_*` modules, one per map, so that a schema
//! bump adds a field next to the map it belongs to rather than growing one
//! serialization module.

use coffret_model::{ContainerKind, EntryMetadata};

use crate::error::{Error, Result};

mod encode;
pub(crate) use encode::{encode, entry_len, envelope_len};

mod decode;
pub(crate) use decode::decode;

// How an Entry Path is read back out of any of those maps, in one place for
// the same reason the maps themselves are.
mod stored_path;

mod wire_derived_from;

// The entry map and the Container kind spelling are FM-9's, and the control
// payloads carry them verbatim: a Journal record's addition carries the entry
// table of the Container it adds (CP-11, FM-15), and an Index Snapshot lists
// every current Entry of the Library, each as that same map plus its
// `container` index (FM-16). They are shared rather than written a second time,
// so one reading of FM-9 serves every map that carries it.
mod wire_entry;
pub(crate) use wire_entry::WireEntry;

mod wire_kind;
pub(crate) use wire_kind::WireKind;

mod wire_meta;

#[cfg(test)]
mod testing;

/// The schema this crate writes.
const SCHEMA: u64 = 1;

/// What a decoded meta section says about the Container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Meta {
    pub(crate) kind: ContainerKind,
    pub(crate) pad_len: u64,
    pub(crate) entries: Vec<EntryMetadata>,
}

impl Meta {
    /// The length of the plaintext stream this meta section describes:
    /// every Entry back to back, then the padding tail.
    pub(crate) fn plaintext_len(&self) -> Result<u64> {
        let unpadded = match self.entries.last() {
            Some(last) => last
                .offset
                .checked_add(last.size)
                .ok_or(Error::StreamTooLong)?,
            None => 0,
        };
        unpadded
            .checked_add(self.pad_len)
            .ok_or(Error::StreamTooLong)
    }
}
