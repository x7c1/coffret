use async_trait::async_trait;
use coffret_model::{ContainerId, ContainerSummary, EntryLocation, EntryPath, IndexCheckpoint};

use crate::committed_batch::CommittedBatch;
use crate::device_state::{DeviceTime, LocalEntry, LocalObservation, Mapping, PendingUpload};
use crate::index_error::IndexResult;
use crate::journal_record::JournalRecord;
use crate::snapshot_content::SnapshotContent;

/// The device-local catalog of one Library.
///
/// The Index answers, without asking Storage anything, which Container holds
/// the Entry at an Entry Path and where inside it — which is what lets a scan
/// detect changed files quickly and a viewer open a page without a lookup. It
/// catalogs the whole Library, not only what this device keeps on disk: a
/// laptop holding just `albums/` still knows which Container every page under
/// `books/` lives in, which is exactly what lets every device restore an
/// identical catalog from one Index Snapshot (spec: CK-7, EP-9).
///
/// It is a cache and never the source of truth. Losing it loses no Library
/// data, because it can be rebuilt exactly from Storage: the newest checkpoint
/// and the Journal records after it say which Containers are current and which
/// Entries each holds, so an exact rebuild opens no Container (spec: RV-1,
/// RV-5, CP-11).
///
/// # Two states, kept apart
///
/// Everything here divides in two, and the division is structural rather than
/// conventional:
///
/// - **Library-wide** — the checkpoint, the current Containers, and every
///   current Entry. This is precisely what an Index Snapshot carries, so
///   [`restore`](Self::restore) replaces it wholesale and
///   [`snapshot`](Self::snapshot) hands it back (spec: CK-7).
/// - **Device-local** — how this device maps the Library onto its folders
///   (spec: EP-9), which Entries it has actually put on disk (spec: EP-10), and
///   what it has spooled before committing (spec: OC-2). None of it is ever
///   uploaded, and no Library-wide operation touches it.
///
/// # Catching up
///
/// A stale device starts from whichever is newer, its own Index or the newest
/// valid checkpoint, and replays only the Journal records after that point
/// (spec: CK-9). That is [`restore`](Self::restore) then
/// [`apply`](Self::apply) when the checkpoint is newer, and
/// [`apply`](Self::apply) alone when its own Index is — the usual case between
/// Snapshots. Neither opens a Container, because a record carries what the
/// Containers it adds hold (spec: CP-11).
///
/// A device that committed a batch itself takes [`refresh`](Self::refresh)
/// instead, which is [`apply`](Self::apply) of that batch's record plus the
/// device-local bookkeeping only the committer has.
///
/// Every operation is atomic: an implementation applies the whole of one and
/// nothing of a failed one, so a catalog is never left half-caught-up.
#[async_trait]
pub trait Index: Send + Sync {
    /// Adopts an Index Snapshot's content, replacing the Library-wide state.
    ///
    /// The Snapshot is the whole Library at one committed state, so this is a
    /// replacement and not a merge: the previous Containers and Entries are
    /// gone, whatever the Snapshot does not mention included (spec: RV-1).
    ///
    /// Device state is left exactly as it was. That is what lets a Snapshot
    /// another device wrote be adopted unchanged: it carries no local root
    /// mappings, no local paths, and no record of what any device has
    /// materialized, so two devices laid out differently adopt the same content
    /// (spec: CK-7). What makes adopting another device's Snapshot *safe* is a
    /// separate matter and not this operation's: the Snapshot is authenticated
    /// under a purpose key derived from the Master Key, and its checkpoint names
    /// the committed Keyring tuple it depends on (spec: CK-9, CK-3, RV-3).
    ///
    /// The content's provenance comes with it:
    /// [`SnapshotContent::adopted_from`](crate::SnapshotContent::adopted_from)
    /// becomes the checkpoint object this catalog was last adopted from,
    /// replacing whatever it named before.
    async fn restore(&self, snapshot: SnapshotContent) -> IndexResult<()>;

    /// Replays one committed Journal record.
    ///
    /// The record's removals leave the current set first and its additions
    /// enter after, which is the order the commit's own uniqueness check runs
    /// in — a path may move from a replaced Container to its replacement in one
    /// record (spec: EP-6). The checkpoint then advances to the head this
    /// record became (spec: CP-1, CK-1).
    ///
    /// No Container is opened: the record carries each new Container's kind and
    /// entry table (spec: CP-11).
    ///
    /// Which checkpoint object this catalog was adopted from is left as it was.
    /// A replay carries the catalog past the Snapshot it started from without
    /// changing where it started, and one that has only ever replayed records
    /// has adopted none, so the provenance stays `None` there (spec: CK-9).
    async fn apply(&self, record: JournalRecord) -> IndexResult<()>;

    /// Applies this device's own batch, after its commit succeeded.
    ///
    /// The Library-wide half is exactly [`apply`](Self::apply) of the batch's
    /// record — a commit changes the current set the same way whoever made it
    /// (spec: CP-1). Beyond that it records what only the committer knows: the
    /// files it put on disk become `present` (spec: EP-10), and the spools of
    /// the Containers the batch uploaded stop being pending, because a
    /// committed Container is no longer a candidate for orphan cleanup
    /// (spec: OC-2).
    ///
    /// It is never called for a batch whose record was not created: before that
    /// the batch has changed nothing (spec: CP-1).
    async fn refresh(&self, batch: CommittedBatch) -> IndexResult<()>;

    /// The whole Library-wide state, in canonical order, ready to be encoded
    /// and uploaded when the checkpoint policy asks for a Snapshot
    /// (spec: CK-8).
    ///
    /// Containers come ordered by ID and Entries by the canonical bytes of
    /// their Entry Path (spec: EP-3), so that two devices at one committed
    /// state produce the same content rather than two orderings of it.
    ///
    /// Fails with [`IndexError::NoCheckpoint`](crate::IndexError::NoCheckpoint)
    /// on an Index that stands at no committed state yet: there is nothing to
    /// checkpoint.
    async fn snapshot(&self) -> IndexResult<SnapshotContent>;

    /// The committed Library state this Index stands at, or `None` on one that
    /// has never restored or applied anything.
    ///
    /// It is what a device catching up compares against the newest checkpoint
    /// on Storage to decide which of the two is the newer starting point
    /// (spec: CK-9).
    async fn checkpoint(&self) -> IndexResult<Option<IndexCheckpoint>>;

    /// Where the current Entry at one Entry Path lives, or `None` if the path
    /// holds none.
    ///
    /// One Entry Path identifies at most one current Entry, so this is the
    /// whole answer rather than one of several (spec: EP-5). The location
    /// carries the Container and the Entry's offset and size inside it, which
    /// is what a range read of a single Entry out of a Pack is aimed with
    /// (spec: PK-16).
    async fn entry_at(&self, path: &EntryPath) -> IndexResult<Option<EntryLocation>>;

    /// Every current Entry under a prefix, ordered by Entry Path bytes.
    ///
    /// `None` is the Library root and answers with all of it; `Some(prefix)` is
    /// the Entry at that path, if any, together with everything beneath it —
    /// the subtree a device maps a local root to (spec: EP-9).
    async fn entries_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<EntryLocation>>;

    /// The distinct current Containers holding any Entry under a prefix,
    /// ordered by Container ID.
    ///
    /// Distinct because Packs do not partition the Library's path order: those
    /// built by different `freeze` invocations may overlap and interleave, so
    /// one Container can hold several Entries under a prefix and one prefix can
    /// span many Containers (spec: PK-8). This is the set to fetch to
    /// materialize that subtree, the fetch unit being a whole Container
    /// (spec: PK-16).
    async fn containers_under(
        &self,
        prefix: Option<&EntryPath>,
    ) -> IndexResult<Vec<ContainerSummary>>;

    /// Records where one part of the Library lives on this device, replacing
    /// any mapping already held for that prefix (spec: EP-9).
    async fn set_mapping(&self, mapping: Mapping) -> IndexResult<()>;

    /// Every mapping this device holds, ordered by prefix with the Library root
    /// first.
    async fn mappings(&self) -> IndexResult<Vec<Mapping>>;

    /// Records that this device now has the file at an Entry Path on disk,
    /// having uploaded or fetched it there (spec: EP-10).
    async fn mark_present(&self, observation: LocalObservation) -> IndexResult<()>;

    /// Records that a file this device had is gone.
    ///
    /// A path this device never materialized has nothing to mark, and the call
    /// changes nothing there rather than failing: absence is a fact about a
    /// file this device put in place, and an Entry it never held is outside its
    /// scope rather than deleted (spec: EP-10). The last observation is kept —
    /// it is what the file looked like when the device still had it — and only
    /// the state and the time of looking change.
    async fn mark_absent(&self, path: &EntryPath, at: DeviceTime) -> IndexResult<()>;

    /// What this device knows about the local file at one Entry Path, or `None`
    /// if it has never had one there.
    ///
    /// The distinction is the whole of EP-10: a row means this device
    /// materialized the Entry at some point, so its absence now is a deletion
    /// this device witnessed, while no row at all means the path was never in
    /// this device's scope. A scan asks this before deciding whether a missing
    /// file is news.
    async fn local_entry_at(&self, path: &EntryPath) -> IndexResult<Option<LocalEntry>>;

    /// The Entries this device has on disk under a prefix, ordered by Entry
    /// Path bytes.
    ///
    /// These are the only ones a scan may report as deleted locally, and the
    /// only ones it may select for `update` or `freeze` (spec: EP-10).
    async fn present_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<LocalEntry>>;

    /// The files this device has on disk at Entry Paths the Library no longer
    /// holds a current Entry for.
    ///
    /// Another device's commit can remove the Container an Entry lived in, and
    /// the local file stays where it is. The row survives the removal so that
    /// the file can be reported rather than silently left behind (spec: EP-10).
    async fn present_without_entry(&self) -> IndexResult<Vec<LocalEntry>>;

    /// Records a Container encrypted, and perhaps uploaded, before its batch
    /// committed, replacing any row already held for that Container.
    ///
    /// This is the local provenance that later makes cleaning the Container up
    /// possible at all, should the batch never commit (spec: OC-2, OC-3).
    async fn record_pending_upload(&self, pending: PendingUpload) -> IndexResult<()>;

    /// Drops the pending row for one Container, its batch having committed or
    /// been abandoned.
    ///
    /// Dropping one that is not there succeeds, so an interrupted cleanup is
    /// simply run again (spec: OC-6).
    async fn clear_pending_upload(&self, container_id: ContainerId) -> IndexResult<()>;

    /// Every Container this device spooled or uploaded whose batch has not
    /// committed, ordered by Container ID.
    async fn pending_uploads(&self) -> IndexResult<Vec<PendingUpload>>;
}
