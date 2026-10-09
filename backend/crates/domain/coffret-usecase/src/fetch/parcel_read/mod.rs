//! Reading the Entry a caller asked for out of its Container, a parcel at a
//! time, and keeping the parcels on the device.
//!
//! The fetch unit is the parcel (spec: PK-16, PK-19): a Container's chunk
//! sequence divided from its first chunk into runs of a fixed number of chunks,
//! with boundaries that have nothing to do with where Entries begin or end. So a
//! read aimed at one page asks Storage for the front of the object — the same
//! read whichever Entry is wanted — and for every parcel the page overlaps, and
//! the provider learns which parcels of which object were read and nothing
//! about the page (spec: PK-20).
//!
//! What arrives is more than the page. Every Entry inside the parcels read is
//! written out as its bytes pass, where this device would place it anyway —
//! mapped, and nothing standing in the way (spec: EP-10, EP-11) — so the rest of
//! a volume is on disk by the time the reader turns to it. And the parcels
//! themselves are kept, because an Entry that runs on into a parcel not yet read
//! needs them again: a fetched parcel stays on the device until every Entry it
//! covers that the device maps is on disk or witnessed absent, or its Container
//! leaves the current set, and a parcel the device holds is never requested
//! from Storage again (spec: PK-21).
//!
//! What a read of parcels verifies is what holds over parcels: every chunk
//! authenticates on its own, bound to its position and to the header
//! (spec: FM-5, FM-7, FM-8), and every placed Entry's plaintext hash is held
//! against the catalog before the file becomes visible (spec: EP-11). The
//! Container's ciphertext hash is not, because it is a claim about every byte
//! of the object and this did not read every byte (spec: PK-22).

use std::ops::Range;

use coffret_model::{EntryMetadata, EntryPath};

use crate::commit::DegradedKeyring;
use crate::device_state::DeviceTime;
use crate::fetch::cancellation::Cancellation;
use crate::fetch::fetch_error::FetchError;
use crate::fetch::kept_parcels::KeptParcels;
use crate::fetch::publication::Publication;
use crate::fetch::reading::Reading;
use crate::fetch::target::Target;
use crate::fetch::unheld_parcel::UnheldParcel;
use crate::index::Index;

mod companions;

mod front;

mod herald;

pub(super) mod let_go;

mod placements;

mod read_entry;

mod source;

mod spread;

/// What reading one Entry by its parcels came to.
pub(super) struct Stroke {
    /// Whether the Entry asked for was placed. `false` only where the caller
    /// cancelled before a parcel it needed was asked for (spec: PK-21).
    pub(super) placed: bool,
    /// The other Entries placed out of the same parcels, in stream order.
    pub(super) alongside: Vec<EntryPath>,
    /// The kept parcels that turned out not to be held, and were read again.
    pub(super) unheld: Vec<UnheldParcel>,
}

/// One Entry wanted out of a Container, the Entries that can come with it, and
/// the parcels each of them needs.
struct Member {
    target: Target,
    entry: EntryMetadata,
    parcels: Range<u64>,
}

/// Everything a parcel read is made against besides the Container itself.
pub(super) struct ParcelRead<'r> {
    pub(super) reading: &'r Reading<'r>,
    pub(super) kept: &'r KeptParcels<'r>,
    pub(super) index: &'r dyn Index,
    pub(super) now: DeviceTime,
    pub(super) cancellation: &'r dyn Cancellation,
    /// Who hears the moment the Entry asked for is published (spec: PK-16).
    pub(super) publication: &'r dyn Publication,
    /// What the run read of the committed Keyring before the parcels, which
    /// goes with that news.
    pub(super) degraded: Option<DegradedKeyring>,
}

/// Whether [`translate::target_of`] refused an Entry for a reason no fetch
/// ever places it under: unmapped, not current, a path no local file can stand
/// for, or one colliding with another Entry's local path. Such an Entry is
/// neither a companion nor waited for by a held parcel.
fn never_placed(error: &FetchError) -> bool {
    matches!(
        error,
        FetchError::UnmappedEntryPath { .. }
            | FetchError::EntryNotCurrent { .. }
            | FetchError::UnmaterializablePath { .. }
            | FetchError::LocalPathCollision { .. }
    )
}
