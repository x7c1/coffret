use anyhow::{Context, Result};
use coffret_format::{wrap_container_key, Purpose, PurposeKey};
use coffret_model::MasterKey;

use crate::fixture_set::{FixtureWriter, BLOBS_DIR};
use crate::manifest::{ContainerFixture, KeyEnvelopeFixture};

/// Wraps a Container Key into the envelope the Keyring stores (FM-14).
pub(super) fn write_key_envelope(
    writer: &FixtureWriter,
    master_key: &MasterKey,
    container: &ContainerFixture,
) -> Result<KeyEnvelopeFixture> {
    let key = PurposeKey::derive(master_key, Purpose::ContainerWrap);
    let container_id = container.container_id()?;
    let container_key = container.container_key()?;
    let envelope = wrap_container_key(&key, &container_id, &container_key)
        .context("wrapping a Container Key")?;

    let file = writer.write(BLOBS_DIR, "key-envelope.bin", envelope.as_bytes())?;
    Ok(KeyEnvelopeFixture {
        fixture: "key-envelope".to_owned(),
        file,
        container_id: container.container_id.clone(),
        container_key: container.container_key.clone(),
    })
}
