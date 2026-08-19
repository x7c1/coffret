use anyhow::{Context, Result};
use coffret_format::{decode_control_object, Purpose, PurposeKey};
use coffret_model::{ControlObjectKind, MasterKey};

use crate::fixture_set::FixtureReader;
use crate::manifest::{check_cbor_map, ControlObjectFixture};

use super::same;

pub(super) fn check_control_object(
    reader: &FixtureReader,
    master_key: &MasterKey,
    fixture: &ControlObjectFixture,
) -> Result<()> {
    let kind = ControlObjectKind::from(fixture.kind);
    let key = PurposeKey::derive(master_key, Purpose::of_control_object(kind));

    let bytes = reader.read(&fixture.file)?;
    let opened = decode_control_object(&bytes, &fixture.object_name, &key)
        .context("opening the control object")?;

    same("kind", &opened.kind, &kind)?;
    same("generation", &opened.generation, &fixture.generation())?;
    same("replica", &opened.replica, &fixture.replica()?)?;
    same(
        "master_key_epoch",
        &opened.payload.master_key_epoch,
        &fixture.master_key_epoch()?,
    )?;
    check_cbor_map(&opened.payload.body, &fixture.body).context("body")
}
