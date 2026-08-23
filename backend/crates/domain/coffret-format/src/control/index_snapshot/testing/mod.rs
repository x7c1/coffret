//! Helpers shared by the Index Snapshot payload's tests.

use coffret_model::{
    ContainerKind, ContainerSummary, ControlObjectName, EntryLocation, Generation, SnapshotContent,
};

use super::{IndexSnapshotPayload, SnapshotActivation};
use crate::control::testing::{checkpoint, container_id, entry, summary};

/// The head this Snapshot's checkpoint stands at.
pub(super) const GENERATION: u64 = 7;

/// A Library of three Containers, whose Entries interleave across them.
///
/// Interleaving is the point: `entries` is in Entry Path order across the whole
/// Library (EP-3), not grouped by Container, so a case comparing the encoded
/// order to the order the content was handed over in has something to catch.
/// The Containers and the Entries are both handed over out of order for the
/// same reason.
pub(super) fn content() -> SnapshotContent {
    SnapshotContent {
        checkpoint: checkpoint(GENERATION),
        // Which checkpoint this Index adopted is device state, and no Snapshot
        // carries it (CK-7). It is set here so that the encoder has something
        // to leave out.
        adopted_from: Some(ControlObjectName::index_snapshot(Generation::new(4))),
        containers: vec![
            summary(0x40, ContainerKind::Pack),
            summary(0x21, ContainerKind::OneFile),
            summary(0x33, ContainerKind::Pack),
        ],
        entries: vec![
            located(0x33, "photos/2019/b.jpg", 0, 90),
            located(0x40, "albums/spring/a.jpg", 0, 100),
            located(0x21, "books/atlas/page-001.png", 0, 200),
            located(0x40, "photos/2019/a.jpg", 100, 80),
        ],
    }
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

/// The content as the encoder puts it on the wire: Containers by ID, Entries by
/// Entry Path, and no record of what this Index adopted (CK-7, FM-16).
pub(super) fn canonical(mut content: SnapshotContent) -> SnapshotContent {
    content.adopted_from = None;
    content.containers.sort_by_key(|container| container.id);
    content
        .entries
        .sort_by(|left, right| left.path().as_str().cmp(right.path().as_str()));
    content
}

/// The Containers of the sample content, in the order the encoder writes them.
pub(super) fn ordered_containers() -> Vec<ContainerSummary> {
    canonical(content()).containers
}
