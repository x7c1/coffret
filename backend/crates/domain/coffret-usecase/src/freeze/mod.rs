//! Packing a folder's eligible files into Packs.
//!
//! [`sync`](crate::sync) carries a folder into the Library one Container per
//! file, and that is the wrong shape at rest: a folder of ten thousand images
//! becomes ten thousand Storage Objects, ten thousand API round trips to open,
//! and an object count a Library-wide rebuild has to walk. This is the operation
//! that puts them in a band a provider and a rebuild can both live with — one
//! invocation, one Journal batch, and a set of Packs holding consecutive Entries
//! (spec: PK-1, PK-7).
//!
//! The sequence, and the rule each step answers to:
//!
//! 1. **Catch up, and read the committed Keyring** (spec: CK-9, KL-1, KL-7).
//!    Eligibility is a question about the current Library — which Container
//!    holds each Entry, and of what kind — so it is asked of an Index that has
//!    read the head. The Keyring is read for one reason: a Container whose key
//!    the Library records as lost is one whose stored content cannot be read
//!    back at all, and what a freeze does about that depends on the Container's
//!    kind (spec: PK-11, PK-13).
//! 2. **Scan** (spec: EP-9, EP-10). Walk the device's mappings, narrowed to the
//!    folder the request names, and translate the regular files under them into
//!    Entry Paths. A path with a current Entry this device never materialized is
//!    outside its scope and is left alone, mapping or no mapping — the same
//!    discipline the sync reads.
//! 3. **Select, and surface what is not selected** (spec: PK-1, PK-13, PK-14).
//!    Eligible: a local file not yet in the Library, and a local file whose
//!    current Entry is held by a one-file Container — including one whose
//!    content has changed and one whose Container's key is lost, because the
//!    replacement is built from the local bytes either way. Not eligible: an
//!    Entry an existing Pack holds. Existing Packs are never read, never
//!    rewritten, and never listed for removal (spec: PK-1, PK-2). A Pack-held
//!    Entry the local file no longer matches, and one whose Pack cannot be
//!    opened, are both update-eligible and are reported in
//!    [`FreezeOutcome::surfaced`] rather than passed over — silence there would
//!    tell the user that stale or unrecoverable content is safely backed up.
//! 4. **Segment** (spec: PK-3, PK-4, PK-5, PK-6). Sort the selection by Entry
//!    Path and append the next Entry while the resulting pre-padding Container
//!    footprint stays at or below the target, closing the current non-empty Pack
//!    when it would not. An Entry over the target on its own stays indivisible
//!    and forms an oversized singleton Pack. No empty Pack is created. The
//!    target is a pack-policy parameter carried in the request, not a format
//!    constant, and the footprint is measured by
//!    [`ContainerFootprint`](coffret_format::ContainerFootprint) so that the
//!    policy and the encoder cannot disagree about what was measured.
//! 5. **Spool each Pack, streaming** (spec: FM-1, FM-2, FM-3, FM-4, FM-5, FM-6,
//!    FM-7, FM-8, FM-9, FM-14, OC-2). A Container ID and a Container Key of its
//!    own, kind `Pack` (spec: PK-15), and the ciphertext written to the spool
//!    directory through [`ContainerWriter`](coffret_format::ContainerWriter):
//!    the scan settled the entry table, so the header and the table go down
//!    first and the member files stream past afterwards. Nothing buffers a
//!    Pack, or an Entry. The pending row is recorded before a byte goes out —
//!    the local provenance that makes cleaning up after an interrupted run
//!    possible at all.
//! 6. **Upload** (spec: FM-3). Put each spool file under the name its Container
//!    ID gives it, through the policy's [`RetryPolicy`](crate::RetryPolicy), and
//!    compare the digest the provider reports for what it stored against the one
//!    taken while writing the spool.
//! 7. **Commit** (spec: PK-7, CP-1). One Journal batch: additions are the new
//!    Packs with their entry tables (spec: CP-11), removals are exactly the
//!    one-file Containers those Packs absorbed. A newly imported file has no
//!    removal and an existing Pack never appears.
//!    [`commit_batch`](crate::commit::commit_batch) supplies the Keyring
//!    pre-replication, the Entry Path uniqueness check, the rebase, and the
//!    settle unchanged.
//!
//! [`freeze_folder`] is the whole of the public surface. The steps are private
//! because none of them is a state a caller may stop at: a spooled Pack that is
//! never committed is an orphan waiting to be cleaned up, not a result.
//!
//! What is deliberately not here. **Repack and compaction** (spec: PK-8): the
//! Packs one invocation builds are local to what it selected, and regrouping
//! across invocations — where path ranges overlap and interleave — is a separate
//! operation. **`update` and deletion** (spec: PK-9, PK-10, PK-11, PK-12):
//! carrying a change into a Pack, or taking an Entry out of one, is
//! read-modify-replace, which this flow surfaces and never performs. **Derived
//! Entries**: thumbnails and transcodes are packed by their own operation. And
//! **settling an interrupted run's pending rows**, which is
//! [`sync`](crate::sync)'s first step and settles a Pack exactly as it settles
//! a one-file Container (spec: OC-2, OC-3, OC-7).

mod freeze_error;
pub use freeze_error::{FreezeError, FreezeResult};

mod freeze_outcome;
pub use freeze_outcome::FreezeOutcome;

mod freeze_request;
pub use freeze_request::FreezeRequest;

mod frozen_pack;
pub use frozen_pack::FrozenPack;

mod not_frozen;
pub use not_frozen::NotFrozen;

mod run;
pub use run::freeze_folder;

mod scan;

mod segment;

mod selected;

mod spool;

mod survey;

// The keys one epoch's Containers are sealed with, and what the operating system
// refused, are shared with the two flows that go the other ways: neither is a
// fact about which operation is moving the bytes, so both are named once at the
// crate root and re-exported where their callers already reach.
pub use crate::library_keys::LibraryKeys;
pub use crate::local_operation::LocalOperation;
