//! The map a control-object payload records one Container with.
//!
//! A Journal record's addition and an Index Snapshot's `containers` element are
//! the same five fields — `id`, `kind`, `ciphertext_hash`, `ciphertext_len`,
//! and optional `object_ref` (FM-15, FM-16) — because they say the same thing:
//! what the Library knows about a current Container without opening it (CP-11).
//! An addition carries the Container's entry table beside them, which the
//! Journal record's own module adds to the map this one builds.

use coffret_model::{ContainerId, ContainerSummary, ContentHash, ObjectRef};

use super::cbor::{Fields, MapBuilder};
use crate::error::{Error, Result};
use crate::meta::WireKind;

const ID: &str = "id";
const KIND: &str = "kind";
const CIPHERTEXT_HASH: &str = "ciphertext_hash";
const CIPHERTEXT_LEN: &str = "ciphertext_len";
const OBJECT_REF: &str = "object_ref";

/// The five fields, ready for a caller to add its own to.
pub(super) fn to_map(container: &ContainerSummary) -> MapBuilder {
    let mut map = MapBuilder::new();
    map.bytes(ID, container.id.as_bytes())
        .text(KIND, WireKind::from(container.kind).spelling())
        .bytes(CIPHERTEXT_HASH, container.ciphertext_hash.as_bytes())
        .uint(CIPHERTEXT_LEN, container.ciphertext_len)
        .optional_text(
            OBJECT_REF,
            container.object_ref.as_ref().map(ObjectRef::as_str),
        );
    map
}

/// Reads the five fields out of a map that may carry more.
pub(super) fn from_fields(
    fields: &Fields<'_>,
    malformed: fn(String) -> Error,
) -> Result<ContainerSummary> {
    let kind = fields.text(KIND)?;
    Ok(ContainerSummary {
        id: ContainerId::from_bytes(fields.byte_array::<{ ContainerId::BYTE_LEN }>(ID)?),
        kind: WireKind::parse(&kind)
            .ok_or_else(|| malformed(format!("{KIND} names no Container kind: {kind:?}")))?
            .into(),
        ciphertext_hash: ContentHash::from_bytes(
            fields.byte_array::<{ ContentHash::BYTE_LEN }>(CIPHERTEXT_HASH)?,
        ),
        ciphertext_len: fields.uint(CIPHERTEXT_LEN)?,
        object_ref: fields.optional_text(OBJECT_REF)?.map(ObjectRef::new),
    })
}
