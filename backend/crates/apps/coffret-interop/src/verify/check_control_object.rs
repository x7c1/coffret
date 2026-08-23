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
    // Opening is where FM-11's payload padding is checked too: the plaintext
    // has to be the CBOR map carried to its Padmé bucket with zeros. A set
    // written by an implementation that skipped the padding stops the exchange
    // here rather than travelling on with the size it leaks.
    //
    // The manifest states no length for the payload, and could not usefully:
    // the two implementations spell and order a map's entries as they please
    // (`check_cbor_map`), so the map length a writer landed on is the writer's
    // own and not something this side derives from the fields the manifest
    // states.
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
