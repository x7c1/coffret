use anyhow::{Context, Result};
use coffret_format::{
    encode_control_object, ControlEncodeRequest, ControlPayload, Purpose, PurposeKey,
};
use coffret_model::{ControlObjectKind, ControlObjectName, MasterKey, MasterKeyEpoch};

use crate::fixture_set::{FixtureWriter, OBJECTS_DIR};
use crate::manifest::{to_cbor_map, BodyField, ControlObjectFixture, WireControlObjectKind};

use super::EPOCH;

pub(super) fn write_control_object(
    writer: &FixtureWriter,
    fixture: &str,
    master_key: &MasterKey,
    name: &ControlObjectName,
    kind: ControlObjectKind,
    body: Vec<BodyField>,
) -> Result<ControlObjectFixture> {
    let key = PurposeKey::derive(master_key, Purpose::of_control_object(kind));
    let epoch = MasterKeyEpoch::new(EPOCH)?;
    let payload = ControlPayload::new(epoch, to_cbor_map(&body)?);
    let encoded = encode_control_object(&ControlEncodeRequest::new(name, kind, &key, &payload))
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
