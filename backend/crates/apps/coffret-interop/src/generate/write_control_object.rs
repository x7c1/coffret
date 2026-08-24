use anyhow::{bail, Context, Result};
use coffret_format::{
    encode_control_object, ControlEncodeRequest, ControlPayload, Purpose, PurposeKey,
};
use coffret_model::{ControlObjectKind, ControlObjectName, MasterKey, MasterKeyEpoch};

use crate::fixture_set::{FixtureWriter, OBJECTS_DIR};
use crate::manifest::{BodyField, ControlObjectFixture, WireControlObjectKind};

use super::EPOCH;

/// Writes one control object, sealing the payload its kind's encoder produced.
///
/// The body and the fields the manifest states come from different places on
/// purpose: the body is what `coffret-format` wrote, and the fields are the
/// separate reading of FM-15, FM-16, and FM-17 that `manifest::payload_fields`
/// keeps. An expectation taken back out of the encoded object would agree with
/// any bug the object carries.
pub(super) fn write_control_object(
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
