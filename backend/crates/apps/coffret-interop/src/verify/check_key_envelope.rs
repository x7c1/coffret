use coffret_format::{unwrap_container_key, Purpose, PurposeKey};
use coffret_model::{KeyEnvelope, MasterKey};

use crate::fixture_set::FixtureReader;
use crate::manifest::KeyEnvelopeFixture;
use crate::{hex, Error, Result, Step};

use super::same;

pub(super) fn check_key_envelope(
    reader: &FixtureReader,
    master_key: &MasterKey,
    fixture: &KeyEnvelopeFixture,
) -> Result<()> {
    let key = PurposeKey::derive(master_key, Purpose::ContainerWrap);
    let bytes = reader.read(&fixture.file)?;
    let envelope =
        KeyEnvelope::from_slice(&bytes).map_err(Error::model(Step::ReadingKeyEnvelope))?;

    let unwrapped = unwrap_container_key(&key, &fixture.container_id()?, &envelope)
        .map_err(Error::format(Step::UnwrappingContainerKey))?;
    same(
        "container_key",
        &hex::encode(unwrapped.as_bytes()),
        &hex::encode(fixture.container_key()?.as_bytes()),
    )
}
