use coffret_model::EntryPath;

use crate::commit::CommitPolicy;
use crate::destinations::Destinations;
use crate::device_state::DeviceTime;
use crate::fetch::cancellation::Cancellation;
use crate::fetch::kept_parcels::KeptParcels;
use crate::fetch::never_cancelled::NEVER_CANCELLED;
use crate::fetch::publication::Publication;
use crate::fetch::unheeded::UNHEEDED;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::progress::{Progress, UNWATCHED};

/// Everything one run of [`fetch_entry`](super::fetch_entry) works from.
///
/// The same ports, capabilities, keys, clock, progress, and policy
/// [`FetchRequest`](super::FetchRequest) takes, and one Entry Path instead of a
/// prefix — and a [`Cancellation`] a caller that may stop wanting the Entry
/// hands over, and a [`Publication`] a caller with a reader waiting hands over.
/// Where the file goes is still not among them: that is the device's mappings,
/// which the [`Index`] holds (spec: EP-9), so a caller cannot fetch an Entry
/// into a folder the Library does not know this device has.
pub struct FetchEntryRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    pub keys: &'a LibraryKeys,
    /// The places on this device the Library's files are written into.
    pub destinations: &'a dyn Destinations,
    /// Where the parcels the run reads are kept, and how long one is
    /// (spec: PK-19, PK-21).
    pub parcels: KeptParcels<'a>,
    /// The Entry to make available on this device.
    pub path: EntryPath,
    /// What this device's clock says as the run starts.
    ///
    /// The observation the run writes down is stamped with it. Nothing about the
    /// Library's correctness rests on it (spec: CP-7).
    pub now: DeviceTime,
    /// Where the run says which phase it is in and whether its one Container
    /// has been read.
    ///
    /// A parcel read is the front of an object and the parcels covering one
    /// Entry, which out of a Pack are tens of megabytes, and a run that said
    /// nothing through the catch-up before it would be as silent as a folder
    /// fetch would be without one. [`UNWATCHED`] is the default and costs
    /// nothing.
    pub progress: &'a dyn Progress,
    /// The decisions Storage does not make.
    ///
    /// A partial fetch commits nothing, so what it takes from the policy is
    /// the [`RetryPolicy`](crate::RetryPolicy): the catch-up it starts with, the
    /// committed Keyring it opens, and every range read it makes run under it.
    pub policy: CommitPolicy,
    /// Whether the caller still wants the rest, asked between parcels and never
    /// inside one (spec: PK-21). [`NEVER_CANCELLED`] is the default.
    pub cancellation: &'a dyn Cancellation,
    /// Who hears the moment the Entry is published, before the parcels it came
    /// out of have finished arriving (spec: PK-16). [`UNHEEDED`] is the
    /// default.
    pub publication: &'a dyn Publication,
}

impl<'a> FetchEntryRequest<'a> {
    /// A run against `store` and `index` for the Entry at one path, under the
    /// default policy.
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        destinations: &'a dyn Destinations,
        parcels: KeptParcels<'a>,
        path: EntryPath,
        now: DeviceTime,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            destinations,
            parcels,
            path,
            now,
            progress: &UNWATCHED,
            policy: CommitPolicy::default(),
            cancellation: &NEVER_CANCELLED,
            publication: &UNHEEDED,
        }
    }

    /// The same request reporting its progress to `progress`.
    pub fn watched_by(mut self, progress: &'a dyn Progress) -> Self {
        self.progress = progress;
        self
    }

    /// The same request, stopping at the next parcel boundary once
    /// `cancellation` says so (spec: PK-21).
    pub fn cancelled_by(mut self, cancellation: &'a dyn Cancellation) -> Self {
        self.cancellation = cancellation;
        self
    }

    /// The same request, telling `publication` the moment the Entry is
    /// published (spec: PK-16).
    pub fn heard_by(mut self, publication: &'a dyn Publication) -> Self {
        self.publication = publication;
        self
    }

    /// The same request under a different policy.
    pub fn with_policy(mut self, policy: CommitPolicy) -> Self {
        self.policy = policy;
        self
    }
}
