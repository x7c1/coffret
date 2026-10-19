use async_trait::async_trait;
use coffret_model::{
    ContainerId, ContainerSummary, EntryLocation, EntryPath, IndexCheckpoint, JournalRecord,
    SnapshotContent,
};

use crate::committed_batch::CommittedBatch;
use crate::device_state::{
    AttemptedSlot, BatchId, DeviceTime, HeldParcel, LocalEntry, LocalObservation, Mapping,
    PendingRow, Reservation, ReservationId,
};
use crate::index_error::IndexResult;

/// The cached Catalog and device-local records of one Library.
///
/// The Index answers, without asking Storage anything, which Container holds
/// the Entry at an Entry Path and where inside it — which is what lets a scan
/// detect changed files quickly and a reader open a page without a lookup. It
/// catalogs the whole Library, not only what this device keeps on disk: a
/// laptop holding just `albums/` still knows which Container every page under
/// `books/` lives in, which is exactly what lets every device restore an
/// identical catalog from one Index Snapshot (spec: CK-7, EP-9).
///
/// Its cached Catalog is reconstructed from intact control state without
/// opening Containers. Device-local records cannot be reconstructed from
/// Storage, and a cache rebuild preserves them (spec: RV-1, RV-5, CK-7).
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
///   (spec: EP-9), which Entries it has actually put on disk (spec: EP-10),
///   what it has spooled before committing (spec: OC-2), which parcels of
///   the Containers it read it still holds (spec: PK-21), and which dropped
///   files wait to be packed (spec: PK-17). None of it is ever
///   uploaded, and no Library-wide operation touches it — save that each
///   materialization record is kept about the Entry it names
///   ([`LocalObservation::entry`]): a rename, replayed or reached through an
///   adopted Snapshot, marks the file this device has of the renamed Entry to
///   follow it, since the file stays where it was placed; a replacement carries
///   the record over to the Entry that takes its Entry's place; removing the
///   Entry clears the mark again; and a record whose path another Entry comes
///   to hold is forgotten rather than read as that Entry's (spec: FM-15,
///   EP-10).
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
    /// Exclusively owns this device's pending rows until the returned guard is
    /// dropped. Sync, freeze and deletion take this before reading or creating
    /// spools, so settlement cannot reclaim another live run's data
    /// (spec: OC-2).
    /// Implementations must coordinate separate connections and processes.
    async fn own_pending_rows(&self) -> IndexResult<crate::PendingRowsGuard>;

    /// Adopts an Index Snapshot's content, replacing the Library-wide state.
    ///
    /// The Snapshot is the whole Library at one committed state, so this is a
    /// replacement and not a merge: the previous Containers and Entries are
    /// gone, whatever the Snapshot does not mention included (spec: RV-1).
    ///
    /// Device state is left as it was, save what each materialization record
    /// says about its Entry ([`LocalObservation::entry`]): a file whose Entry
    /// the Snapshot holds at another path than its own is marked to follow it
    /// there, as replaying the renames in between would have, and one whose
    /// Entry stands at its own path waits for nothing
    /// ([`LocalEntry::relocate_to`]). A record whose Entry the Snapshot no
    /// longer holds is carried over to the Entry standing where its Entry
    /// stood, where that Entry's Container is one the replaced catalog did not
    /// hold — the shape every replacement has, an update, a freeze absorbing a
    /// file or a rebuilt Pack, since each brings a new Container (spec: EP-6).
    /// Any other record of an Entry that left waits for nothing, and is
    /// forgotten where another Entry now holds the path it stands at: an Entry
    /// that was already current, a rename brought there, is never one the
    /// record's file is a copy of (spec: FM-15, EP-10, EP-15). Nothing else of
    /// this device's is touched. That is
    /// what lets a Snapshot another device wrote be adopted unchanged: it
    /// carries no local root mappings, no local paths, and no record of what
    /// any device has materialized, so two devices laid out differently adopt
    /// the same content (spec: CK-7). What makes adopting another device's Snapshot *safe* is a
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
    /// The record's removals leave the current set first, its renames then
    /// change the current paths of the Entries they name all at once, and its
    /// additions enter last, which is the order the commit's own uniqueness
    /// check runs in — a path may pass from a replaced Container to its
    /// replacement in one record (spec: EP-6), and two Entries may swap their
    /// names (spec: FM-15). The checkpoint then advances to the head this
    /// record became (spec: CP-1, CK-1).
    ///
    /// A rename changes an Entry's current path and nothing else: its
    /// Container, its entry number and the captured path its Container
    /// recorded stay as they were. Each one is held to FM-15's state rules
    /// against this catalog, and one that breaks any of them refuses the whole
    /// record with
    /// [`IndexError::RenameRefused`](crate::IndexError::RenameRefused), leaving
    /// the catalog and this device's rows exactly as they were. The row of a
    /// file this device has of a renamed Entry stays keyed by the path the
    /// file stands at, and records where the Entry stands now
    /// ([`LocalEntry::relocate_to`]) until the file is relocated or placed
    /// afresh.
    ///
    /// A record whose Entry this record replaces — removes its Container and
    /// adds, at the path the Entry stood at, an Entry of a Container the
    /// record adds — is carried over to the replacement (spec: EP-6). One whose
    /// Entry the record removes outright waits for nothing any more, and is
    /// forgotten where another Entry holds the path its file stands at; so is
    /// one whose Entry left earlier and whose path this record gives to another
    /// Entry, by an addition or a rename. Such a file is not a copy of that
    /// Entry, and a record kept there would claim it (spec: EP-10, EP-15).
    ///
    /// A record whose generation is at or below the head this Index stands at
    /// is refused before any of it is looked at, with
    /// [`IndexError::AlreadyApplied`](crate::IndexError::AlreadyApplied), and
    /// the catalog and this device's rows stay exactly as they were. The check
    /// is part of the same atomic operation as the apply it guards, so another
    /// process replaying into the same catalog cannot land the record between
    /// the two: a second application is refused by its generation rather than
    /// by whatever it would have collided on, and a record no collision would
    /// stop — one carrying only removals, or the two halves of a swap — never
    /// carries the catalog back (spec: CK-9, RV-5). With that refusal set
    /// apart, a duplicate or a refused rename always reports a record no
    /// commit could have produced from this catalog.
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
    /// The reservations the batch names in
    /// [`released`](CommittedBatch::released) are released in the same
    /// operation, so a drop's Pack is never committed while its reservation
    /// still says it waits for one, nor the reservation gone while the batch
    /// that packs it has not committed (spec: PK-17).
    ///
    /// It is never called for a batch whose record was not created: before that
    /// the batch has changed nothing (spec: CP-1). Being [`apply`](Self::apply)
    /// of the record first, it refuses a record the Index already stands at or
    /// past the same way, recording nothing of the batch's own.
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
    /// carries the Container and the Entry's extent inside it, which is what
    /// says which parcels of a Pack hold the Entry (spec: PK-16, PK-19).
    async fn entry_at(&self, path: &EntryPath) -> IndexResult<Option<EntryLocation>>;

    /// Every current Entry under a prefix, ordered by Entry Path bytes.
    ///
    /// `None` is the Library root and answers with all of it; `Some(prefix)` is
    /// the Entry at that path, if any, together with everything beneath it —
    /// the subtree a device maps a local root to (spec: EP-9).
    async fn entries_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<EntryLocation>>;

    /// Every current Entry one Container holds, ordered by entry number.
    ///
    /// The Container's side of the catalog: what a reader holding its entry
    /// table asks to learn which of those Entries are current and at which
    /// path each stands now, since a rename changes the path and leaves the
    /// `(Container ID, entry number)` alone (spec: FM-9, FM-15). A Container
    /// that is not current holds none.
    async fn entries_in(&self, container_id: ContainerId) -> IndexResult<Vec<EntryLocation>>;

    /// The distinct current Containers holding any Entry under a prefix,
    /// ordered by Container ID.
    ///
    /// Distinct because Packs do not partition the Library's path order: those
    /// built by different `freeze` invocations may overlap and interleave, so
    /// one Container can hold several Entries under a prefix and one prefix can
    /// span many Containers (spec: PK-8). This is the set to read from to
    /// materialize that subtree, a parcel at a time or every parcel at once
    /// (spec: PK-16).
    async fn containers_under(
        &self,
        prefix: Option<&EntryPath>,
    ) -> IndexResult<Vec<ContainerSummary>>;

    /// Records where one part of the Library lives on this device, replacing
    /// any mapping already held for that prefix (spec: EP-9).
    ///
    /// The mapping is stored exactly as given,
    /// [`root_identity`](Mapping::root_identity) included — which makes this
    /// operation two things at once. A scan uses it to stamp a root it has just
    /// seen, and a device uses it to say "this root is what I meant" after a run
    /// reported the mapping unavailable: recording the mapping again with no
    /// identity clears the stored one, so the next scan stamps whatever is there
    /// and infers the deletions under it (spec: EP-12). That is the same
    /// operation that created the mapping, so re-confirming a root needs no
    /// separate surface — and it is the one gesture that remedies a root
    /// reported unavailable because the user genuinely emptied it while its
    /// filesystem identity moved in the same interval.
    async fn set_mapping(&self, mapping: Mapping) -> IndexResult<()>;

    /// Every mapping this device holds, ordered by prefix with the Library root
    /// first.
    async fn mappings(&self) -> IndexResult<Vec<Mapping>>;

    /// Records that this device now has the file at an Entry Path on disk,
    /// having uploaded or fetched it there, as a copy of the Entry the
    /// observation names (spec: EP-10).
    ///
    /// Whatever row stood at the path is replaced. The new one waits to follow
    /// its Entry where the catalog holds that Entry at another path
    /// ([`LocalEntry::relocate_to`]) — the file of an update a rebase carried
    /// over a rename — and waits for nothing where it holds it at this one.
    /// One recorded for an Entry the catalog no longer holds is kept only
    /// where no other Entry holds the path, as every record of such an Entry
    /// is.
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

    /// Forgets this device's row for one Entry Path, so the path is outside its
    /// scope again (spec: EP-10, EP-15).
    ///
    /// What becomes of the row of a file whose Entry has left the Library once
    /// nothing of it is left on this device's disk either: the sync discarded
    /// it into the trash, or the person moved or removed it. Forgotten rather
    /// than marked [`Absent`](crate::device_state::LocalEntryState::Absent),
    /// because absence is a deletion this device witnessed of an Entry it
    /// materialized, and a fetch declines to put such an Entry back
    /// (spec: EP-11) — an Entry another device later adds at the same path is a
    /// different one, and this device has witnessed nothing about it.
    ///
    /// Forgetting a row that is not there succeeds, so an interrupted run's
    /// next pass simply forgets it again.
    async fn forget_local_entry(&self, path: &EntryPath) -> IndexResult<()>;

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
    /// only ones it may select for `update` or `freeze` (spec: EP-10). A file
    /// waiting to follow a rename of its Entry elsewhere
    /// ([`LocalEntry::relocate_to`]) is not the Entry at its own path, and is
    /// not among them.
    async fn present_under(&self, prefix: Option<&EntryPath>) -> IndexResult<Vec<LocalEntry>>;

    /// Every row whose file waits to follow its Entry to another path
    /// ([`LocalEntry::relocate_to`]), ordered by the path it stands at
    /// (spec: EP-10).
    ///
    /// What a run that relocates reads before it scans or places anything:
    /// absent rows among them, whose files are gone, as well as present ones.
    async fn awaiting_relocation(&self) -> IndexResult<Vec<LocalEntry>>;

    /// Re-keys this device's rows to the paths their files now stand at, all
    /// of them at once (spec: EP-10).
    ///
    /// Each `(from, to)` takes the row at `from` to `to`: the observation keeps
    /// what it said — the file a relocation renames keeps its length and modification
    /// time — and only its path changes, and the row then waits for whatever
    /// its Entry's current path still asks of it, nothing where the Entry
    /// stands at `to`. Every row a `from` names is taken out before any is put
    /// back, so two rows may exchange their paths, the way two files exchange
    /// theirs on disk. A row already at a `to` that no `from` takes out is
    /// replaced: the file the caller relocated is what stands there now. A
    /// `from` with no row is passed over, so an interrupted relocation is
    /// simply re-keyed again.
    ///
    /// Called only after the files have been relocated on disk: a record changes
    /// after its file, so an interruption between the two leaves the old
    /// record with the file at the new path, which the next run recognizes,
    /// and never a record pointing at a path with no file.
    async fn relocate_local_entries(&self, relocated: &[(EntryPath, EntryPath)])
        -> IndexResult<()>;

    /// The files this device has on disk at Entry Paths the Library no longer
    /// holds a current Entry for.
    ///
    /// Another device's commit can remove the Container an Entry lived in, and
    /// the local file stays where it is. The row survives the removal so that
    /// the file can be reported rather than silently left behind (spec: EP-10).
    /// A file waiting to follow a rename of its Entry is not one: its Entry has
    /// not left the Library (spec: FM-15).
    async fn present_without_entry(&self) -> IndexResult<Vec<LocalEntry>>;

    /// Records the pending row of a Container this device is about to spool, has
    /// spooled, or has uploaded before its batch committed, replacing any row
    /// already held for that Container.
    ///
    /// This is the local provenance that later makes cleaning the Container up
    /// possible at all, should the batch never commit (spec: OC-2, OC-3). The
    /// row's [`state`](PendingRow::state) says which of the three the
    /// Container is at, and the first of them is why the row is written before
    /// the spool file exists rather than after: the provenance has to cover the
    /// ciphertext from the moment there can be any.
    async fn record_pending_row(&self, pending: PendingRow) -> IndexResult<()>;

    /// Records that one Container's spool file is whole, moving its pending row
    /// from [`Spooling`](crate::device_state::SpoolState::Spooling) to
    /// [`Spooled`](crate::device_state::SpoolState::Spooled) (spec: OC-2). A
    /// row that is already `Spooled` keeps what it names, the object handle
    /// included.
    ///
    /// An update and not an upsert, for the reason
    /// [`mark_absent`](Self::mark_absent) is one: a Container with no row
    /// changes nothing rather than failing. The operation says that a row which
    /// exists names a whole file, and inventing one would record a spool the
    /// flow never announced.
    async fn mark_spooled(&self, container_id: ContainerId) -> IndexResult<()>;

    /// Records that a commit is about to send the Journal record naming these
    /// Containers into `slot`, marking each one's pending row
    /// [`commit_attempted`](PendingRow::commit_attempted) and recording `slot`
    /// against the batch of every row it marks (spec: OC-2, OC-3).
    ///
    /// All or nothing: either every named row is marked and the slot recorded
    /// or, on an error, none of it is. The rows of one batch are marked together
    /// because a later settle judges each row on its own, and a batch left
    /// half-marked would have it keep the marked rows as of unknown outcome
    /// while it discards the unmarked ones as never sent — although no create
    /// was sent for any of them. The slot is recorded in the same write because
    /// a marked row whose slot is missing could never be proven
    /// (spec: OC-3), and a slot recorded for rows left unmarked would say a
    /// create was about to be sent when none was.
    ///
    /// A slot already recorded for the batch is kept rather than recorded twice,
    /// and every other slot it holds stays: a batch that rebases (spec: CP-4)
    /// attempts one slot per attempt, and the proof needs every one of them.
    ///
    /// An update and not an upsert, for the reason
    /// [`mark_spooled`](Self::mark_spooled) is one: a Container with no row
    /// changes nothing rather than failing, and records no slot.
    async fn mark_commit_attempted(
        &self,
        container_ids: &[ContainerId],
        slot: &AttemptedSlot,
    ) -> IndexResult<()>;

    /// Every commit slot recorded against `batch`, ordered by generation and
    /// then by identifier (spec: OC-2, OC-3).
    ///
    /// Empty for a batch that never attempted a commit, and equally for one
    /// whose rows were marked before slots were recorded at all — which is the
    /// missing evidence OC-2 tells settlement to read as uncertain. What a batch
    /// recorded goes when the last of its pending rows does.
    async fn attempted_slots(&self, batch: &BatchId) -> IndexResult<Vec<AttemptedSlot>>;

    /// Drops the pending row for one Container, its batch having committed or
    /// been abandoned, and the slots its batch recorded with the batch's last
    /// row.
    ///
    /// Dropping one that is not there succeeds, so an interrupted cleanup is
    /// simply run again (spec: OC-8).
    async fn clear_pending_row(&self, container_id: ContainerId) -> IndexResult<()>;

    /// The pending row of every Container this device is about to spool, has
    /// spooled, or has uploaded whose batch has not committed, ordered by
    /// Container ID.
    ///
    /// The first of the three is why a row here is not evidence of a file: the
    /// row is written before the spool file it names exists, so one still
    /// [`Spooling`](crate::device_state::SpoolState::Spooling) can name a
    /// file that is half-written or that was never created at all (spec: OC-2).
    async fn pending_rows(&self) -> IndexResult<Vec<PendingRow>>;

    /// Records that this device holds one parcel of one Container, replacing
    /// any row already held for that parcel (spec: PK-21).
    ///
    /// Written before the parcel's file is created, the order a pending row
    /// keeps with its spool (spec: OC-2): the record has to cover the
    /// ciphertext from the moment there can be any, and a row whose file turns
    /// out short or missing is a parcel the fetch treats as not held.
    async fn hold_parcel(&self, parcel: HeldParcel) -> IndexResult<()>;

    /// Every parcel this device holds, ordered by Container ID and then by
    /// parcel index (spec: PK-21).
    async fn held_parcels(&self) -> IndexResult<Vec<HeldParcel>>;

    /// Forgets the row of one held parcel, the parcel having been let go.
    ///
    /// Forgetting one that is not there succeeds, so an interrupted letting go
    /// is simply run again (spec: OC-8, PK-21).
    async fn let_go_parcel(&self, container_id: ContainerId, index: u64) -> IndexResult<()>;

    /// Records a drop's choice to add its files to `folder` as a Pack, before
    /// any of them has landed, and answers with the reservation's identifier
    /// (spec: PK-17).
    ///
    /// The reservation names no path yet; each is added by
    /// [`extend_reservation`](Self::extend_reservation) before its file lands.
    /// Identifiers are drawn in the order reservations are made and never
    /// reused, so [`reservations`](Self::reservations) answers in that order.
    async fn reserve(&self, folder: Option<&EntryPath>) -> IndexResult<ReservationId>;

    /// Adds `path` to the reservation `id`, before the file the drop is
    /// writing there lands (spec: PK-17).
    ///
    /// An update and not an upsert, for the reason
    /// [`mark_spooled`](Self::mark_spooled) is one: a reservation that is no
    /// longer there changes nothing rather than failing, and none is made up
    /// for it.
    async fn extend_reservation(&self, id: ReservationId, path: &EntryPath) -> IndexResult<()>;

    /// Every reservation this device holds, in the order they were made
    /// (spec: PK-17).
    async fn reservations(&self) -> IndexResult<Vec<Reservation>>;

    /// Releases one reservation, its drop having been refused or its freeze
    /// having found none of its paths eligible any more (spec: PK-17).
    ///
    /// Releasing one that is not there succeeds, so an interrupted clean-up is
    /// simply run again (spec: OC-8). A reservation a commit packs goes with
    /// that commit's [`refresh`](Self::refresh) instead.
    async fn release_reservation(&self, id: ReservationId) -> IndexResult<()>;
}
