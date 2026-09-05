//! Writing the fixture set the TypeScript implementation has to open.
//!
//! The set is small on purpose: every fixture is here because it pins something
//! two implementations can disagree about — a multi-chunk stream, an empty one,
//! a Pack that an Entry count would misread as a one-file Container, each
//! control-object kind under its own purpose key — including both kinds the
//! control-head chain's one name form admits — a replica that is not the
//! only one of its set, an Entry with optional metadata, an Entry whose
//! `mtime` predates 1970, the one Passphrase-derived form a device carries
//! between builds, and the two Recovery Codes a Master Key leaves a device in.
//!
//! Every control object carries the payload its own schema defines: a Journal
//! record with additions, their entry tables, and a removal (FM-15), both Index
//! Snapshot kinds (FM-16), and a Keyring replica whose mapping holds envelopes
//! and a key-lost marker (FM-17). Their arrays are built out of the canonical
//! order on purpose, so a set whose writer left the order alone fails the
//! exchange.
//!
//! This module states what the set contains; each object the set needs is
//! written by a `write_*` submodule of its own, so a fixture kind the exchange
//! gains later arrives as a new module rather than as more of this one, and the
//! domain values the payloads are made of live in `control_payloads`.

use std::path::Path;

use anyhow::{Context, Result};
use coffret_format::{
    encode_index_snapshot, encode_journal_record, encode_keyring, generate_master_key, ChunkSize,
};
use coffret_model::{
    ContainerKind, ControlObjectKind, ControlObjectName, MasterKeyEpoch, ReplicaPosition,
};

use crate::fixture_set::FixtureWriter;
use crate::hex;
use crate::manifest::{
    index_snapshot_fields, journal_record_fields, keyring_fields, Manifest, SCHEMA,
};

mod entry_paths;
use entry_paths::entry_path;

mod generations;
use generations::generation;

mod control_payloads;
use control_payloads::{
    activation_snapshot, journal_record, keyring_mapping, ordinary_snapshot, set_digest,
    ACTIVATION_GENERATION, JOURNAL_GENERATION, SNAPSHOT_GENERATION,
};

mod entry_plan;
use entry_plan::EntryPlan;

mod write_container;
use write_container::write_container;

mod write_control_object;
use write_control_object::write_control_object;

mod write_key_envelope;
use write_key_envelope::write_key_envelope;

mod write_recovery_codes;
use write_recovery_codes::write_recovery_codes;

mod write_stored_master_key;
use write_stored_master_key::write_stored_master_key;

/// The Passphrase the generated stored Master Key form is protected under.
const PASSPHRASE: &str = "correct horse battery staple";

/// The epoch every control object in the generated set was written under.
const EPOCH: u64 = 3;

/// An epoch past what 32 bits hold, which one Recovery Code in the set carries.
///
/// The epoch is 8 bytes wide (KD-11), and nothing else in the set has an epoch
/// large enough to tell a reader that took it for 4 apart from one that did not.
const LATE_EPOCH: u64 = 4_294_967_297;

/// The generation the generated Keyring replica set carries.
const KEYRING_REPLICA_GENERATION: u64 = 12;

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
                .btime(1_600_000_000)
                .mime("image/jpeg"),
        ],
    )?;
    let parent_id = one_file.container_id()?;

    // Everything optional an Entry can carry appears here, alongside an Entry
    // that carries none of it and one whose mtime predates 1970. The Entries
    // are in Entry Path order, as the segmentation that builds a Pack leaves
    // them (PK-3), and the derived one records the Entry it was produced from:
    // the photo in the Container above (FM-9). One Entry's birth time predates
    // 1970 too, so a reader that took the field as unsigned lands elsewhere.
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
            )
            .btime(-2_208_988_800),
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

    let key_envelope = write_key_envelope(&writer, &master_key, &one_file)?;

    // A link in the control-head chain, under the kind-neutral name the whole
    // chain shares (FM-12), carrying what a commit records: additions with
    // their entry tables, and a removal (FM-15).
    let record = journal_record();
    let journal = write_control_object(
        &writer,
        "journal",
        &master_key,
        &ControlObjectName::head(generation(JOURNAL_GENERATION)),
        ControlObjectKind::Journal,
        &encode_journal_record(&record)?,
        journal_record_fields(&record),
    )?;
    // The other kind the same name form admits: the Index Snapshot that
    // activated this set's epoch, at the head generation it took. An
    // implementation that read the kind off the name rather than off the
    // authenticated header would open this one as a Journal record, or refuse
    // it, so both chain kinds travel. It carries the two fields an ordinary
    // Snapshot may not (FM-16, MR-2).
    let activating = activation_snapshot();
    let activation_snapshot = write_control_object(
        &writer,
        "activation-snapshot",
        &master_key,
        &ControlObjectName::head(generation(ACTIVATION_GENERATION)),
        ControlObjectKind::ActivationSnapshot,
        &encode_index_snapshot(&activating)?,
        index_snapshot_fields(&activating),
    )?;
    // A replica that is not the only one of its set: the replica position rides
    // in the authenticated header as well as in the name. Its payload is the
    // generation's whole mapping (FM-17), and its name carries the digest of
    // exactly that mapping — so a reader that recomputes the digest from what it
    // opened has the name to hold it against (FM-12, KL-1).
    let mapping = keyring_mapping();
    let keyring_replica = write_control_object(
        &writer,
        "keyring-replica",
        &master_key,
        &ControlObjectName::keyring_replica(
            generation(KEYRING_REPLICA_GENERATION),
            &set_digest(),
            ReplicaPosition::new(1, 3)?,
        )?,
        ControlObjectKind::Keyring,
        &encode_keyring(&mapping, MasterKeyEpoch::new(EPOCH)?)?,
        keyring_fields(&mapping),
    )?;
    // The ordinary checkpoint of one head, carrying the whole Library's Index
    // (FM-16): several Containers, and Entries in Entry Path order across all of
    // them rather than grouped by Container. It carries that head's generation
    // under the `idx-` name only checkpoints take (CK-10, FM-12) — a name form
    // the chain above never uses.
    let ordinary = ordinary_snapshot();
    let index_snapshot = write_control_object(
        &writer,
        "index-snapshot",
        &master_key,
        &ControlObjectName::index_snapshot(generation(SNAPSHOT_GENERATION)),
        ControlObjectKind::IndexSnapshot,
        &encode_index_snapshot(&ordinary)?,
        index_snapshot_fields(&ordinary),
    )?;

    let stored_master_key = write_stored_master_key(&writer, &master_key)?;
    let recovery_codes = write_recovery_codes(&writer, &master_key)?;

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
        recovery_codes,
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
