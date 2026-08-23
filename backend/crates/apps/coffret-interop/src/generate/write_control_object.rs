use anyhow::{bail, Context, Result};
use coffret_format::{
    encode_control_object, ControlEncodeRequest, ControlPayload, Purpose, PurposeKey,
};
use coffret_model::{ControlObjectKind, ControlObjectName, MasterKey, MasterKeyEpoch};

use crate::fixture_set::{FixtureWriter, OBJECTS_DIR};
use crate::manifest::{to_cbor_map, BodyField, ControlObjectFixture, WireControlObjectKind};

use super::EPOCH;

/// Writes one control object whose payload body the manifest fields describe.
///
/// This is for a kind whose own schema is not written yet: the body is built
/// from the fields the manifest states, because there is nothing else to build
/// it from.
pub(super) fn write_control_object(
    writer: &FixtureWriter,
    fixture: &str,
    master_key: &MasterKey,
    name: &ControlObjectName,
    kind: ControlObjectKind,
    body: Vec<BodyField>,
) -> Result<ControlObjectFixture> {
    let payload = ControlPayload::new(MasterKeyEpoch::new(EPOCH)?, to_cbor_map(&body)?);
    write_payload_object(writer, fixture, master_key, name, kind, &payload, body)
}

/// Writes one control object whose payload a schema encoder produced.
///
/// The body and the fields the manifest states come from different places on
/// purpose: the body is what `coffret-format` wrote, and the fields are the
/// separate reading of FM-15 and FM-16 that `manifest::payload_fields` keeps.
/// An expectation taken back out of the encoded object would agree with any bug
/// the object carries.
pub(super) fn write_payload_object(
    writer: &FixtureWriter,
    fixture: &str,
    master_key: &MasterKey,
    name: &ControlObjectName,
    kind: ControlObjectKind,
    payload: &ControlPayload,
    body: Vec<BodyField>,
) -> Result<ControlObjectFixture> {
    let key = PurposeKey::derive(master_key, Purpose::of_control_object(kind));
    let epoch = MasterKeyEpoch::new(EPOCH)?;
    if payload.master_key_epoch != epoch {
        bail!(
            "the {fixture:?} payload names epoch {}, the set is written under {epoch:?}",
            payload.master_key_epoch.get()
        );
    }
    let encoded = encode_control_object(&ControlEncodeRequest::new(name, kind, &key, payload))
        .with_context(|| format!("encoding the {fixture:?} control object"))?;

    let file = writer.write(OBJECTS_DIR, encoded.object_name(), encoded.bytes())?;
    Ok(ControlObjectFixture {
        fixture: fixture.to_owned(),
        file,
        object_name: encoded.object_name().to_owned(),
        kind: WireControlObjectKind::from(kind),
        generation: name.generation().get(),
        replica_index: name.replica().index(),
        replica_count: name.replica().count(),
        master_key_epoch: epoch.get(),
        body,
    })
}
