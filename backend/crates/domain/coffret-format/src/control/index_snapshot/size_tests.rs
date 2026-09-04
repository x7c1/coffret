//! What a Snapshot of a real-sized Library costs (FM-16).
//!
//! The Snapshot payload is the one control object whose size grows with the
//! Library rather than with a batch, and a device whose own Index is older than
//! the newest checkpoint fetches one to start from (CK-9). The per-Entry cost
//! is therefore the number the schema was shaped around: naming a Container by
//! index instead of by ID, and carrying no device state, are both there to hold
//! it down (CK-7).
//!
//! Two things are measured here, because only one of them is the schema's. The
//! cost of a whole Entry includes its Entry Path, which is the Library's to
//! choose and not this format's — a Library of long paths costs more per Entry
//! whatever the schema does. So the design budget is checked against a Library
//! with the paths a photo and book collection actually carries, and the cost
//! *beyond* the path is pinned separately: that number is the schema's own, and
//! it does not move when the sample's paths do.

use coffret_model::{ContainerId, ContainerKind, EntryLocation, SnapshotContent};

use super::testing::{located, GENERATION};
use super::{encode, IndexSnapshotPayload};
use crate::control::testing::{checkpoint, summary};

/// Entries in the synthetic Library.
const ENTRIES: usize = 10_000;

/// Entries per Container, so the Library holds a realistic number of Packs
/// rather than one Container per Entry or one Container for everything.
const ENTRIES_PER_CONTAINER: usize = 50;

/// The ceiling this format was sized on: 120 bytes per Entry before padding.
const DESIGN_BUDGET: usize = 120;

/// What one Entry costs beyond its own Entry Path.
///
/// Everything the schema itself spends: the entry map's keys and their values —
/// `offset`, `size`, `mtime`, the 32-byte `hash`, and the `container` index —
/// plus this Entry's share of the `containers` array and the checkpoint. It is
/// pinned rather than bounded loosely, because it is the number a change to the
/// schema moves: a field added per Entry, or a Container named by ID again,
/// shows up here and nowhere else.
const PINNED_COST_BEYOND_THE_PATH: usize = 94;

// FM-16: a Snapshot of ten thousand Entries stays inside the per-Entry cost the
// schema was shaped around, for a Library whose paths look like a real one's.
#[test]
fn ten_thousand_entries_stay_inside_the_design_budget() {
    let payload = encode(&library()).expect("encoding a whole Library succeeds");
    let per_entry = payload.body.len() / ENTRIES;
    assert!(
        per_entry <= DESIGN_BUDGET,
        "a Snapshot of {ENTRIES} Entries costs {per_entry} bytes per Entry, \
         past the {DESIGN_BUDGET}-byte design budget"
    );
}

// The part of that cost the schema is answerable for: what is left of it once
// the Entry Paths are taken out.
#[test]
fn the_cost_beyond_the_entry_path_is_pinned() {
    let library = library();
    let paths: usize = library
        .content
        .entries
        .iter()
        .map(|location| location.path().as_str().len())
        .sum();
    let payload = encode(&library).expect("encoding a whole Library succeeds");

    let beyond_the_path = (payload.body.len() - paths) / ENTRIES;
    assert_eq!(
        beyond_the_path, PINNED_COST_BEYOND_THE_PATH,
        "one Entry costs {beyond_the_path} bytes beyond its path; the pinned figure is \
         {PINNED_COST_BEYOND_THE_PATH}. Moving it is a decision about what every catch-up \
         downloads, not a number to follow the code."
    );
}

/// A Library of [`ENTRIES`] Entries with the paths a real one carries.
fn library() -> IndexSnapshotPayload {
    let container_count = ENTRIES.div_ceil(ENTRIES_PER_CONTAINER);
    let containers = (0..container_count)
        .map(|index| {
            let mut container = summary(0, ContainerKind::Pack);
            container.id = synthetic_id(index);
            container
        })
        .collect();

    let entries = (0..ENTRIES)
        .map(|index| {
            let mut location = EntryLocation {
                container_id: synthetic_id(index / ENTRIES_PER_CONTAINER),
                entry: located(0, &path(index), 0, 0).entry,
            };
            location.entry.offset = (index % ENTRIES_PER_CONTAINER) as u64 * 240_000;
            location.entry.size = 240_000;
            location
        })
        .collect();

    IndexSnapshotPayload::ordinary(SnapshotContent {
        checkpoint: checkpoint(GENERATION),
        adopted_from: None,
        containers,
        entries,
    })
}

/// A Container ID that differs between Containers and orders by index.
fn synthetic_id(index: usize) -> ContainerId {
    let mut bytes = [0u8; ContainerId::BYTE_LEN];
    for (position, byte) in bytes.iter_mut().enumerate() {
        *byte = (index as u8)
            .wrapping_mul(31)
            .wrapping_add(position as u8 * 7);
    }
    bytes[..4].copy_from_slice(&(index as u32).to_be_bytes());
    ContainerId::from_bytes(bytes)
}

/// Half books and half photos, spelled the way a real Library spells them.
fn path(index: usize) -> String {
    if index.is_multiple_of(2) {
        format!("books/vol-{:04}/page-{:03}.png", index / 200, index % 200)
    } else {
        format!("albums/{}/IMG_{:04}.jpg", 2000 + index % 25, index % 10_000)
    }
}
