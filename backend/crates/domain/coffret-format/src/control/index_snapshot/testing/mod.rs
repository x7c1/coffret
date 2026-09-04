//! Helpers shared by the Index Snapshot payload's tests.

use coffret_model::{
    Btime, ContainerKind, ContainerSummary, ControlObjectName, EntryLocation, Generation,
    SnapshotContent,
};

use super::{IndexSnapshotPayload, SnapshotActivation};
use crate::control::testing::{checkpoint, container_id, entry, summary};

/// The head this Snapshot's checkpoint stands at.
pub(super) const GENERATION: u64 = 7;

/// The birth time the one Entry that has one was created at.
pub(super) const BORN: Btime = Btime::from_unix_seconds(1_600_000_000);

/// Where that Entry sits once the encoder has put `entries` in Entry Path order
/// (EP-3): `albums/spring/a.jpg` sorts before every other path in the sample.
pub(super) const BORN_AT: usize = 0;

/// A Library of three Containers, whose Entries interleave across them.
///
/// Interleaving is the point: `entries` is in Entry Path order across the whole
/// Library (EP-3), not grouped by Container, so a case comparing the encoded
/// order to the order the content was handed over in has something to catch.
/// The Containers and the Entries are both handed over out of order for the
/// same reason. One Entry's file had a birth time when its Container was
/// written and the rest had none, so both spellings of the optional field
/// travel (FM-16).
pub(super) fn content() -> SnapshotContent {
    // Which checkpoint this Index adopted is device state, and no Snapshot
    // carries it (CK-7). It is set here so that the encoder has something to
    // leave out.
    content_of(Some(ControlObjectName::index_snapshot(Generation::new(4))))
}

/// The same Library as a decoded Snapshot reports it: no provenance, because
/// none of it was encoded (CK-7).
pub(super) fn decoded_content() -> SnapshotContent {
    content_of(None)
}

/// That Library's content, recorded as adopted from what the caller names.
fn content_of(adopted_from: Option<ControlObjectName>) -> SnapshotContent {
    let mut born = located(0x40, "albums/spring/a.jpg", 0, 100);
    born.entry.btime = Some(BORN);
    SnapshotContent::canonical(
        checkpoint(GENERATION),
        adopted_from,
        vec![
            summary(0x40, ContainerKind::Pack),
            summary(0x21, ContainerKind::OneFile),
            summary(0x33, ContainerKind::Pack),
        ],
        vec![
            located(0x33, "photos/2019/b.jpg", 0, 90),
            born,
            located(0x21, "books/atlas/page-001.png", 0, 200),
            located(0x40, "photos/2019/a.jpg", 100, 80),
        ],
    )
    .expect("a fixture holds a Library an Index could stand at")
}

/// The ordinary checkpoint of that head (CK-10).
pub(super) fn ordinary() -> IndexSnapshotPayload {
    IndexSnapshotPayload::ordinary(content())
}

/// The Snapshot that activated this epoch, at the head it took (MR-2).
pub(super) fn activating() -> IndexSnapshotPayload {
    IndexSnapshotPayload::activating(
        content(),
        SnapshotActivation {
            base_head_generation: Generation::new(GENERATION - 1),
            activation_slot: Some("minted-head-7".to_owned()),
        },
    )
}

/// One Entry of the Library, held by the Container with the given seed.
pub(super) fn located(seed: u8, path: &str, offset: u64, size: u64) -> EntryLocation {
    EntryLocation {
        container_id: container_id(seed),
        entry: entry(path, offset, size),
    }
}

/// A Library of exactly the Containers and Entries handed in, at the same head.
pub(super) fn content_holding(
    containers: Vec<ContainerSummary>,
    entries: Vec<EntryLocation>,
) -> SnapshotContent {
    SnapshotContent::canonical(checkpoint(GENERATION), None, containers, entries)
        .expect("a fixture holds a Library an Index could stand at")
}

/// The Containers of the sample content, in the order they are written.
pub(super) fn ordered_containers() -> Vec<ContainerSummary> {
    content().containers().to_vec()
}
