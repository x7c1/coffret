//! The meta section: one CBOR map, encrypted as a single AEAD message.
//!
//! Container-level fields are `schema`, `kind`, `pad_len`, and `entries`; each
//! entry records `original_path`, `offset`, `size`, `original_mtime`, and
//! `hash`, plus optional `original_btime`, `derived_from`, and `mime`. The
//! `original_` prefix says what those values are: the Entry Path and the two
//! times as of the moment this Container was written, which is all an
//! immutable object can state about them. The maps are forward-open — a
//! reader ignores fields it does not know, and adding a field only increments
//! `schema`.
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

use coffret_model::{ContainerKind, EntryMetadata, MAX_FORMAT_INTEGER};

use crate::error::{Error, Result};

mod encode;
pub(crate) use encode::{encode, entry_len, envelope_len};

mod decode;
pub(crate) use decode::decode;

// How an Entry Path is read back out of any of those maps, in one place for
// the same reason the maps themselves are. The control payloads read one out of
// their own entry maps too, so this is the crate's single reading of EP-1 on
// the way back in.
mod stored_path;
pub(crate) use stored_path::stored_path;

// The parent reference is one map with one spelling, carried unchanged by the
// meta section and by the control payloads alike.
mod wire_derived_from;
pub(crate) use wire_derived_from::WireDerivedFrom;

// The entry table's own spelling. The control payloads carry the same values
// under the catalog's names — a Journal record's addition carries the entry
// table of the Container it adds (CP-11, FM-15), and an Index Snapshot lists
// every current Entry of the Library, each as that map plus its `container`
// index (FM-16) — so their map lives beside them, in `WireCatalogEntry`, and
// this one is the meta section's alone.
mod wire_meta_entry;

mod wire_kind;
pub(crate) use wire_kind::WireKind;

mod wire_meta;

#[cfg(test)]
mod rejection_tests;
#[cfg(test)]
mod round_trip_tests;

#[cfg(test)]
mod testing;

/// The schema this crate writes.
const SCHEMA: u64 = 1;

/// What a meta section that is not the CBOR FM-9 spells is reported as.
///
/// The maps this module reads are the meta section's own, so one variant covers
/// all of them. The same entry map read out of a control payload is that
/// payload's malformed variant instead, which is why every reading of it takes
/// the constructor rather than naming one.
fn malformed(detail: String) -> Error {
    Error::MalformedMeta { detail }
}

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
    ///
    /// The padded stream ends at a position the format admits, as every Entry's
    /// own end does (spec: FM-9, FM-19). The last extent and `pad_len` are each
    /// held to that bound where they are read, so their sum is the one place a
    /// meta section can still name a position past it.
    pub(crate) fn plaintext_len(&self) -> Result<u64> {
        let unpadded = match self.entries.last() {
            Some(last) => last.extent.end(),
            None => 0,
        };
        match unpadded.checked_add(self.pad_len) {
            Some(len) if len <= MAX_FORMAT_INTEGER => Ok(len),
            _ => Err(Error::StreamTooLong),
        }
    }
}
