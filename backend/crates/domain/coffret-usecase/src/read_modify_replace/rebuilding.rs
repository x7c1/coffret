use std::path::Path;

use crate::commit::ControlListing;
use crate::device_state::{BatchId, DeviceTime};
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;
use crate::spool::Spool;

/// What every rebuild in one run is made against.
///
/// The things that do not change between Containers: where the objects live
/// and how hard to try for one, the keys that open the old ones and seal the
/// new ones, the walk the catch-up made, and where on this device the
/// replacements wait for their commit, under which batch and clock.
pub(crate) struct Rebuilding<'a> {
    /// Where the Library's objects live.
    pub(crate) store: &'a dyn ObjectStore,
    /// The catalog the pending rows are written into (spec: OC-2).
    pub(crate) index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    pub(crate) keys: &'a LibraryKeys,
    /// Where replacements are spooled.
    pub(crate) spool: &'a dyn Spool,
    /// The directory inside it they go in.
    pub(crate) spool_dir: &'a Path,
    /// How hard to try for an old object that does not come back.
    pub(crate) retry: &'a RetryPolicy,
    /// The control objects the catch-up walked, for a Container the Index
    /// holds no handle for (spec: FM-3).
    pub(crate) listing: &'a ControlListing,
    /// What this device calls the batch the replacements go into (spec: OC-2).
    pub(crate) batch: &'a BatchId,
    /// What this device's clock said as the run started.
    pub(crate) now: DeviceTime,
}
