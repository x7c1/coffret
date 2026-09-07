use crate::commit::ControlListing;
use crate::destinations::Destinations;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// What every read of a Container in one run is made against.
///
/// The five things that do not change between Containers: where the objects
/// live, how hard to try for one, the keys that open them, where the files they
/// hold go, and the walk the catch-up already made — which is what answers for a
/// Container this device has never seen and so caches no handle for
/// (spec: FM-3).
///
/// They travel together because they are one run's arrangements rather than one
/// Container's, and gathering them says so: what a step is *about* is then the
/// summary, the envelope, and the Entries it was asked for, which is what
/// changes each time around the loop.
pub(super) struct Reading<'a> {
    /// Where the Library's objects live.
    pub(super) store: &'a dyn ObjectStore,
    /// How hard to try for one that does not come back.
    pub(super) retry: &'a RetryPolicy,
    /// The keys of the epoch the Library is in.
    pub(super) keys: &'a LibraryKeys,
    /// Where on this device the Entries a read produces are placed.
    pub(super) destinations: &'a dyn Destinations,
    /// The control objects the catch-up walked, for a Container the Index holds
    /// no handle for.
    pub(super) listing: &'a ControlListing,
}
