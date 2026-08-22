//! Writing the fixture set the TypeScript implementation has to open.
//!
//! The set is small on purpose: every fixture is here because it pins something
//! two implementations can disagree about — a multi-chunk stream, an empty one,
//! a Pack that an Entry count would misread as a one-file Container, each
//! control-object kind under its own purpose key — including both kinds the
//! control-head chain's one name form admits — a replica that is not the
//! only one of its set, an Entry with optional metadata, an Entry whose
//! `mtime` predates 1970, and the one Passphrase-derived form a device carries
//! between builds.
//!
//! This module states what the set contains; each object the set needs is
//! written by a `write_*` submodule of its own, so a fixture kind the exchange
//! gains later arrives as a new module rather than as more of this one.

use std::path::Path;

use anyhow::{Context, Result};
use coffret_format::{generate_master_key, ChunkSize};
use coffret_model::{
    ContainerKind, ControlObjectKind, ControlObjectName, Generation, ReplicaPosition,
};

use crate::fixture_set::FixtureWriter;
use crate::hex;
use crate::manifest::{BodyField, Manifest, SCHEMA};

mod entry_plan;
use entry_plan::EntryPlan;

mod write_container;
use write_container::write_container;

mod write_control_object;
use write_control_object::write_control_object;

mod write_key_envelope;
use write_key_envelope::write_key_envelope;

mod write_stored_master_key;
use write_stored_master_key::write_stored_master_key;

/// The Passphrase the generated stored Master Key form is protected under.
const PASSPHRASE: &str = "correct horse battery staple";

/// The epoch every control object in the generated set was written under.
const EPOCH: u64 = 3;

/// The digest the generated Keyring replica set carries in its name.
const KEYRING_SET_DIGEST: &str = "9f0c";

/// A chunk size small enough that the multi-Entry stream spans several chunks.
///
/// One chunk would leave the non-final chunk domain of FM-7 untested, and with
/// it the part of the nonce two implementations are most able to disagree on.
const SMALL_CHUNK_SIZE: u32 = 64;

/// Writes a complete fixture set into `out`.
pub fn generate(out: &Path) -> Result<()> {
    let writer = FixtureWriter::create(out)?;
    let master_key = generate_master_key().context("drawing the Master Key")?;

    let one_file = write_container(
        &writer,
        "one-file",
        ContainerKind::OneFile,
        ChunkSize::DEFAULT,
        &[
            EntryPlan::new("photos/spring.jpg", 1_700_000_000, filler(4096, 0x11))
                .mime("image/jpeg"),
        ],
    )?;
    let parent_id = one_file.container_id()?;

    // Everything optional an Entry can carry appears here, alongside an Entry
    // that carries none of it and one whose mtime predates 1970. The Entries
    // are in Entry Path order, as the segmentation that builds a Pack leaves
    // them (PK-3), and the derived one records the Entry it was produced from:
    // the photo in the Container above (FM-9).
    let multi_entry = write_container(
        &writer,
        "multi-entry",
        ContainerKind::Pack,
        ChunkSize::new(SMALL_CHUNK_SIZE)?,
        &[
            EntryPlan::new("album/2019/party.jpg", 1_500_000_000, filler(100, 0x37))
                .mime("image/jpeg"),
            EntryPlan::new(
                "notes/ancient.txt",
                -2_208_988_800,
                b"written in 1900".to_vec(),
            ),
            EntryPlan::new("photos/.thumbs/spring.jpg", 1_500_000_100, filler(30, 0x5b))
                .derived_from(parent_id, "photos/spring.jpg")
                .mime("image/webp"),
        ],
    )?;

    // A Pack holding exactly one Entry, which is what deletion leaves behind:
    // the replacement keeps the old Container's kind (PK-15). Without it every
    // Pack in the set would hold several Entries and every one-file Container
    // exactly one, so an implementation that inferred the kind from the Entry
    // count instead of reading the field would still pass the exchange.
    let singleton_pack = write_container(
        &writer,
        "singleton-pack",
        ContainerKind::Pack,
        ChunkSize::DEFAULT,
        &[
            EntryPlan::new("books/atlas.pdf", 1_600_000_000, filler(200, 0x2d))
                .mime("application/pdf"),
        ],
    )?;

    // Entries that are all empty leave a zero-length stream, which both
    // implementations still write as one final chunk.
    let empty_entries = write_container(
        &writer,
        "empty-entries",
        ContainerKind::Pack,
        ChunkSize::DEFAULT,
        &[
            EntryPlan::new("empty/first", 0, Vec::new()),
            EntryPlan::new("empty/second", 1, Vec::new()),
        ],
    )?;

    let (key_envelope, envelope_bytes) = write_key_envelope(&writer, &master_key, &one_file)?;

    // A link in the control-head chain, under the kind-neutral name the whole
    // chain shares (FM-12).
    let journal = write_control_object(
        &writer,
        "journal",
        &master_key,
        &ControlObjectName::head(Generation::new(7)),
        ControlObjectKind::Journal,
        vec![
            BodyField::uint("records", 2),
            BodyField::text("note", "the kind's own fields are opaque to the framing"),
        ],
    )?;
    // The other kind the same name form admits: the Index Snapshot that
    // activated this set's epoch, at the head generation it took. An
    // implementation that read the kind off the name rather than off the
    // authenticated header would open this one as a Journal record, or refuse
    // it, so both chain kinds travel.
    let activation_snapshot = write_control_object(
        &writer,
        "activation-snapshot",
        &master_key,
        &ControlObjectName::head(Generation::new(2)),
        ControlObjectKind::ActivationSnapshot,
        vec![BodyField::uint("activated_epoch", EPOCH)],
    )?;
    // A replica that is not the only one of its set: the replica position rides
    // in the authenticated header as well as in the name.
    let keyring_replica = write_control_object(
        &writer,
        "keyring-replica",
        &master_key,
        &ControlObjectName::keyring_replica(
            Generation::new(12),
            KEYRING_SET_DIGEST,
            ReplicaPosition::new(1, 3)?,
        )?,
        ControlObjectKind::Keyring,
        vec![
            BodyField::uint("containers", 1),
            BodyField::bytes("envelope", &envelope_bytes),
        ],
    )?;
    // A kind with no fields of its own yet, so a payload of nothing but the
    // epoch travels too. This one is the ordinary checkpoint of head 4, so it
    // carries that head's generation under the `idx-` name only checkpoints
    // take (CK-10, FM-12) — a name form the chain above never uses.
    let index_snapshot = write_control_object(
        &writer,
        "index-snapshot",
        &master_key,
        &ControlObjectName::index_snapshot(Generation::new(4)),
        ControlObjectKind::IndexSnapshot,
        Vec::new(),
    )?;

    let stored_master_key = write_stored_master_key(&writer, &master_key)?;

    writer.write_manifest(&Manifest {
        schema: SCHEMA,
        producer: "rust".to_owned(),
        master_key: hex::encode(master_key.as_bytes()),
        passphrase: PASSPHRASE.to_owned(),
        containers: vec![one_file, multi_entry, singleton_pack, empty_entries],
        control_objects: vec![
            journal,
            activation_snapshot,
            keyring_replica,
            index_snapshot,
        ],
        key_envelopes: vec![key_envelope],
        stored_master_keys: vec![stored_master_key],
    })
}

/// Content that differs in every byte, so a reader that dropped or reordered
/// bytes lands on a different hash rather than on the same one.
fn filler(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture_set::FixtureReader;

    #[test]
    fn a_generated_set_covers_every_required_fixture() {
        let directory = tempfile::tempdir().expect("a temporary directory is available");
        generate(directory.path()).expect("the set is generated");
        FixtureReader::open(directory.path()).expect("the set covers every fixture");
    }

    // FM-7: the multi-Entry stream must span more than one chunk, or the
    // non-final chunk domain never appears in the exchange.
    #[test]
    fn the_multi_entry_container_spans_several_chunks() {
        let directory = tempfile::tempdir().expect("a temporary directory is available");
        generate(directory.path()).expect("the set is generated");
        let reader = FixtureReader::open(directory.path()).expect("the set opens");
        let fixture = reader
            .manifest()
            .containers
            .iter()
            .find(|fixture| fixture.fixture == "multi-entry")
            .expect("the multi-Entry Container is listed");

        let stream: usize = fixture
            .entries
            .iter()
            .map(|entry| entry.content.len() / 2)
            .sum();
        assert!(stream > fixture.chunk_size as usize, "{stream} bytes");
    }
}
