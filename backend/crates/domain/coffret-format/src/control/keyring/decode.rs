use coffret_model::{ContainerId, KeyEnvelope, KeyringEntry, KeyringMapping};

use super::{ENVELOPE, ID, KEY_LOST, MAPPING, SCHEMA};
use crate::control::cbor::{read_body, Fields, SCHEMA_FIELD};
use crate::control::ControlPayload;
use crate::error::{Error, Result};

/// Parses a Keyring mapping out of the payload a replica carried (FM-17).
///
/// What the mapping says about the Library — that it covers every current
/// Container and no other (KL-7) — needs the Journal to check and is no part of
/// reading one replica. What is checked here is what makes these bytes a
/// mapping at all: every element maps its Container to exactly one thing, and
/// the elements are in the order that gives one mapping one `set_digest`.
///
/// The digest itself is not checked here either, because the payload does not
/// carry it: a caller compares [`set_digest()`](super::set_digest()) of what
/// this returns against the name it fetched the replica under (FM-12, KL-1).
pub fn decode(payload: &ControlPayload) -> Result<KeyringMapping> {
    let value = read_body(&payload.body, malformed)?;
    let fields = Fields::of(&value, malformed)?;

    let schema = fields.uint(SCHEMA_FIELD)?;
    if schema < SCHEMA {
        return Err(Error::UnsupportedKeyringSchema { schema });
    }

    let entries = fields
        .array(MAPPING)?
        .iter()
        .enumerate()
        .map(|(index, value)| entry(index, &fields.map(value)?))
        .collect::<Result<Vec<_>>>()?;

    // The order, and the one Container mapped twice that the same walk catches,
    // are the mapping's own rule: the entries are handed over as they were read
    // rather than sorted into shape (FM-17).
    KeyringMapping::new(entries).map_err(|error| match error {
        coffret_model::Error::CollectionOutOfCanonicalOrder { collection, index } => {
            Error::ControlPayloadOutOfOrder {
                array: collection,
                index,
            }
        }
        other => Error::Model(other),
    })
}

/// One element: a Container, and the one thing the Keyring holds for it.
fn entry(index: usize, fields: &Fields<'_>) -> Result<KeyringEntry> {
    let container_id = ContainerId::from_bytes(fields.byte_array::<{ ContainerId::BYTE_LEN }>(ID)?);

    let envelope = match fields.get(ENVELOPE) {
        Some(_) => Some(KeyEnvelope::from_bytes(
            fields.byte_array::<{ KeyEnvelope::BYTE_LEN }>(ENVELOPE)?,
        )),
        None => None,
    };
    let key_lost = match fields.optional_bool(KEY_LOST)? {
        // FM-17 spells the marker `true`. A `false` there is not "no marker":
        // it is a writer stating the field in a form the rule does not define,
        // and reading it as an absence would put two spellings of an envelope's
        // presence into circulation. It is refused as a rule violation of its
        // own rather than as a malformed payload: the field carried the type
        // the schema gives it, so what a caller learns from this is the same
        // kind of thing the two variants below report, not that the CBOR was
        // unreadable.
        Some(false) => return Err(Error::KeyringEntryMarkerNotTrue { index }),
        marker => marker.is_some(),
    };

    match (envelope, key_lost) {
        (Some(envelope), false) => Ok(KeyringEntry::envelope(container_id, envelope)),
        (None, true) => Ok(KeyringEntry::key_lost(container_id)),
        (Some(_), true) => Err(Error::KeyringEntryWithEnvelopeAndMarker { index }),
        (None, false) => Err(Error::KeyringEntryWithoutEnvelopeOrMarker { index }),
    }
}

/// What a field of the wrong shape in this schema is reported as.
fn malformed(detail: String) -> Error {
    Error::MalformedKeyringPayload { detail }
}
