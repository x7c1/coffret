//! The field numbers every stored CBOR map carries its fields under
//! (spec: FM-9, FM-15, FM-16, FM-17).
//!
//! A field's name is how the specification, this crate's readers and writers,
//! and every refusal speak of it, and never how it travels: on the wire each
//! field is carried as a CBOR map key equal to the field number the
//! field-number registry, `docs/spec/format/field-numbers.json`, assigns it
//! within its map. Keeping the two apart is what lets a field be renamed
//! without a format change, and keeps the Index Snapshot's per-Entry cost down
//! to one byte per field number.
//!
//! The registry is normative and this module is this crate's declaration of
//! it: one [`MapFields`] per map kind, each field with its name and field
//! number and, where the field holds a map (or an array of maps) of its own,
//! which kind that is. A test loads the registry and holds these declarations
//! against it map by map, in both directions, so neither can gain or lose a
//! field number without the other.
//!
//! The codecs above this module keep reading and writing fields by name. What
//! happens here is the one translation between the two spellings, at the edge
//! where a map becomes bytes or bytes become a map:
//!
//! - `to_wire` turns a map keyed by names into the map the wire carries. A
//!   name the map kind does not declare, or declares only as retired, is a
//!   writer bug and refused, so no retired field number is ever written.
//! - `from_wire` turns a wire map back into one keyed by names. The maps are
//!   forward-open, so a field number this build does not know is dropped, as
//!   is a retired one — it is never read again. A text map key is no field's
//!   spelling at all and is dropped too: a map that carries its required
//!   fields only under text keys reaches the reader without them, and is
//!   refused as missing whichever field it lacks first.
//! - [`named_view`] is the same translation for a person reading a payload
//!   rather than for a decoder: nothing is dropped, so a map key the registry
//!   does not name stays visible beside the names.
//!
//! [`named_view`]: fn@named_view

mod registered_field;

mod map_fields;
pub use map_fields::MapFields;

mod registry;
#[cfg(test)]
pub(crate) use registry::ALL;
pub(crate) use registry::MASTER_KEY_EPOCH_FIELD;
pub use registry::{
    ADDITION, ADDITION_ENTRY, DERIVED_FROM, INDEX_SNAPSHOT, JOURNAL_RECORD, KEYRING,
    KEYRING_ELEMENT, META, META_ENTRY, SNAPSHOT_CONTAINER, SNAPSHOT_ENTRY,
};

mod payload_map;
pub use payload_map::payload_map;

mod to_wire;
pub(crate) use to_wire::to_wire;

mod from_wire;
pub(crate) use from_wire::from_wire;

mod named_view;
pub use named_view::named_view;

mod field_number;
pub(crate) use field_number::field_number;

#[cfg(test)]
mod to_wire_lenient;
#[cfg(test)]
pub(crate) use to_wire_lenient::to_wire_lenient;

#[cfg(test)]
mod tests;
