use anyhow::{Context, Result};
use coffret_format::{
    decode_control_object, decode_index_snapshot, decode_journal_record, DecodedControlObject,
    Purpose, PurposeKey,
};
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
    check_cbor_map(&opened.payload.body, &fixture.body).context("body")?;
    check_payload_schema(&opened).context("payload schema")
}

/// Reads the payload again through the schema its kind owns.
///
/// The field-by-field check above proves the two implementations agree on the
/// map. This proves the map is one this side can actually make a Journal record
/// or an Index Snapshot out of: the canonical orders hold, every `container`
/// index names a Container the payload lists, and the activation fields agree
/// with the kind in the authenticated header (FM-15, FM-16). A body that
/// matched the manifest field for field but arrived in the wrong order would
/// pass the check above and fail here, which is exactly the disagreement those
/// orders exist to prevent.
///
/// The Keyring's payload has no schema yet, so its body stays opaque.
fn check_payload_schema(opened: &DecodedControlObject) -> Result<()> {
    match opened.kind {
        ControlObjectKind::Journal => {
            decode_journal_record(&opened.payload, opened.generation)
                .context("reading the Journal record")?;
        }
        ControlObjectKind::IndexSnapshot | ControlObjectKind::ActivationSnapshot => {
            decode_index_snapshot(&opened.payload, opened.kind)
                .context("reading the Index Snapshot")?;
        }
        ControlObjectKind::Keyring => {}
    }
    Ok(())
}
