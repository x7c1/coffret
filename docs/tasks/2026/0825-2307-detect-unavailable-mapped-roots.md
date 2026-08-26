---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && (cd backend && RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items) && git grep -q 'EP-12' -- docs/spec/entry-path/README.md && ! git grep -q 'A mapped root that is not there holds no files' -- backend && git grep -q 'SCHEMA_VERSION: i64 = 3' -- backend/crates/gateway/coffret-sqlite-index/src/schema.rs && git grep -q root_identity -- backend/crates/gateway/coffret-sqlite-index/src/schema.rs"
assignee: null
branch: task/0825-2307-detect-unavailable-mapped-roots
created_at: 2026-08-25T14:07:58Z
updated_at: 2026-08-25T16:25:10Z
---

# fix(backend): stop reading an unavailable mapped root as an emptied folder

## Overview

A mapped root that is not there is treated as a folder holding nothing, and the
sync then reports every Entry under it as deleted.

The walk is where it starts. `local_scan/walk_mappings.rs:66`–`:132` pushes
`(root, "")` onto one stack and loops, so the root directory and every
directory below it go through the same `fs::read_dir`
(`walk_mappings.rs:75`–`:82`). Its `NotFound` arm is a bare `continue`:

```rust
// A mapped root that is not there holds no files, and a directory
// that went away mid-walk holds no more: neither is a reason to
// fail a run over the folders that are there.
Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
```

The comment names both cases and the code cannot tell them apart — the stack
carries no marker for "this is the root", so the arm that is right for a
subdirectory that vanished mid-walk is also what answers for a mapped root that
was never opened at all.

The worse shape does not even reach that arm. An unmounted mount point is an
ordinary empty directory: `read_dir` succeeds, `next_entry`
(`walk_mappings.rs:84`) yields nothing, and `walk` returns an empty `Vec` — the
same value it returns for a folder the user really emptied.

`sync/scan/deletions.rs:17`–`:34` then draws the conclusion:

```rust
for mapping in mappings {
    for local in index.present_under(mapping.prefix.as_ref()).await? {
        if !found.contains_key(&local.observation.path) {
            gone.insert(local.observation.path);
        }
    }
}
```

Every Entry the Index holds `present` under that mapping is absent from `found`,
so every one of them becomes `Deferred::DeletedLocally`
(`sync/deferred.rs:35`–`:38`). Unplug an external drive, or lose a network
mount, run a sync, and the device reports its entire mapped subtree as locally
deleted.

Today the damage is capped, because a deletion is reported and not acted on:
`sync/deferred.rs:29`–`:34` says the Entry stays current, the row stays as it
is, and the finding is repeated by every later run.
`sync_conformance/scope.rs:44`–`:64` pins that. But the cap is the roadmap's,
not the design's — deletion propagation into Packs is PK-9..PK-12, named as
future work in `sync/mod.rs:72` and `freeze/mod.rs:73`–`:76` — and the
report is already wrong. A user who reads `SyncOutcome::deferred`
(`sync/sync_outcome.rs:40`–`:41`) after unplugging a disk is told that
thousands of files were deleted from their Library's mapped folders.

**Freeze is the silent half of the same bug.** `freeze/scan/mod.rs:52` calls the
same `walk_mappings`, so a freeze over an unmounted root walks zero files:
`considered` stays 0, `survey.selected` is empty, `segment::segment` returns no
segments, the `if !segments.is_empty()` block at `freeze/run.rs:91`–`:99` is
skipped, and the outcome is `packs: []`, `absorbed: []`, `packed_already: 0`,
`surfaced: []`, `commit: None` (`freeze/run.rs:118`–`:131`). A successful run
that packed nothing and surfaced nothing — which is exactly what the second
freeze of an already-packed folder looks like (`freeze/run.rs:59`–`:61`). The
user cannot tell "already packed" from "the disk is not plugged in".

**One further leak, which a naive fix does not close.** `deletions` asks
`present_under(mapping.prefix.as_ref())`, and for a Library-root mapping the
prefix is `None`, which `index.rs:195`–`:200` answers with *every* present row —
including the rows under a top-level mapping's prefix. Today the union of the
walks covers the union of those scopes, so nothing is misreported. Skipping an
unavailable mapping breaks that: with a root mapping available and an `albums/`
mapping unavailable, the root mapping's `present_under(None)` still names every
`albums/...` row, and none of them is in `found`. So the deletion step has to
apply the same partition the walk applies — a top-level mapping represents its
subtree and the root mapping represents the remainder (spec: EP-9,
`walk_mappings.rs:32`–`:45`) — rather than only skipping the unavailable
mapping's own pass.

### The decided fix: a root the device cannot vouch for is reported, not read

Two guards, one behind the other, both in front of the walk rather than inside
it.

1. **A missing root is not an empty root.** The root's own existence is checked
   once, before anything under it is walked, so it is a different answer from a
   subdirectory that vanished mid-walk. A mapping whose root is not there is
   *unavailable*: nothing under it is walked, no deletion is inferred for it,
   and the run reports the mapping with the reason. Every other mapping scans
   normally.

2. **An unmounted mount point is not an emptied folder.** Each mapping carries
   the filesystem identity its root stood on when a scan last saw it. A root
   that is empty *and* stands on a filesystem the mapping does not record is
   unavailable for the same reason and by the same route. A root that holds
   files and stands on a filesystem the mapping does not record is available,
   and its identity is re-stamped: a device number that moved across a reboot is
   not evidence that a folder went away.

Neither guard ever infers *more* deletions than today's code. Every way the
comparison can go wrong ends in "skip deletion inference for this mapping" or
"re-stamp the identity", so the failure direction is always toward reporting
less and never toward reporting a file gone that is not.

#### Which filesystem identity, and why

Recorded form: the device number of the root, `st_dev`, read through
`std::os::unix::fs::MetadataExt::dev()`. Stored opaquely — see the port shape
below — and never interpreted, only compared with itself.

*When* it is recorded is what makes the design sound, and it is not at mapping
creation. **The scan stamps it.** A mapping is stored with no identity, and a
walk that finds the root stamps it with what it saw. That gives the guard its
one non-probabilistic property: once a scan has seen the mounted drive, the
recorded identity *is* the mounted drive's device number, and the root
filesystem that the same path resolves on after an unmount necessarily carries a
different one — two filesystems mounted at the same moment cannot share a device
number. So an unmount after any successful scan is a guaranteed mismatch, not a
likely one.

The re-stamp on a non-empty mismatch is what keeps that from becoming brittle.
Device numbers are not stable across reboots for every filesystem — network
mounts, LVM volumes, and btrfs subvolumes can all come back renumbered — and
without the re-stamp such a root would report unavailable on every run
thereafter, which is a folder silently stopping being backed up. With it, a
renumbered-but-present root costs one `set_mapping` and nothing else.

Three options were weighed, and the PR review is the place to overturn the
choice:

- **`st_dev` recorded at mapping creation, with no re-stamp.** Rejected on both
  ends. A mapping recorded while the drive is unmounted would stamp the *root
  filesystem's* number — the exact wrong value, permanently — and a reboot that
  renumbers a mounted volume would leave the mapping reporting unavailable
  forever. It also needs a mapping-creation surface that does filesystem I/O,
  which does not exist: mappings are created only through
  `Index::set_mapping` (`index.rs:163`–`:165`), a port operation that must not
  stat anything, and there is no use-case-level "record a mapping" function
  above it. Stamping from the scan needs no new surface at all.
- **Comparing the root's `st_dev` with its parent directory's** (a root is a
  mount point exactly when they differ), which persists nothing and is immune to
  renumbering. Rejected because it only says anything where the root is
  *expected* to be a mount point, and coffret has no way to know that: a mapping
  onto a plain subdirectory of the system disk legitimately shares its parent's
  device number, which is the shape every case in `sync_conformance` and
  `freeze_conformance` has (the fixtures map a temporary directory —
  `sync_conformance/fixtures.rs:84`–`:93`). The test would either flag every
  ordinary mapping or catch nothing.
- **The chosen combination**, stated as one rule: stamp what the walk saw
  whenever what it saw is evidence — no identity recorded yet, or a recorded
  identity that differs while the root holds files — and treat an empty root
  whose identity differs as unavailable.

The chosen design leaves one residual state that needs a person, and it is the
right one to leave: a folder the user *genuinely* emptied whose device number
*also* moved in the same interval reports unavailable rather than reporting the
deletions, and it will keep doing so, because an empty root is never re-stamped.
The gesture that resolves it is recording the mapping again — `set_mapping`
stores the mapping as given, identity included, so a mapping recorded afresh
carries no identity and the next scan stamps whatever is there and infers the
deletions. That is the same operation that created the mapping, so no new
confirmation flag is needed; `Index::set_mapping`'s rustdoc must say that this
is what it is for.

The other residual: an unmounted mount point whose underlying directory is *not*
empty (somebody wrote into the mount point while it was unmounted) re-stamps and
infers deletions. That is exactly today's behavior, so the guard regresses
nothing, and the shape it does not cover has to be stated where the rule is
written rather than left implied.

**Portability.** This repo targets Linux and macOS (both unix platforms) for the first release, and
`std::os::unix::fs::MetadataExt` is available there; the pattern for guarding it
is already in the tree (`coffret-logging/src/rotating_files/create_directory.rs:6,17`
and `start_file.rs:16,44` each pair a `#[cfg(unix)]` arm with a portable one).
The seam is that the *stored* identity is opaque: a platform that can say
nothing about the filesystem under a path records nothing, and a mapping with no
identity is simply guarded by the missing-root check alone. Nothing above the
one function that reads it knows what is inside it.

### What the walk returns

`walk_mappings` currently answers with `BTreeMap<EntryPath, SourceFile>`
(`walk_mappings.rs:29`–`:31`). It gains a per-mapping verdict alongside, in a
new `local_scan/walked.rs`:

```rust
/// What one walk of every mapping found, and what it made of each root.
pub(crate) struct Walked {
    /// Every regular file under every available mapping, by the Entry Path it
    /// stands at.
    pub(crate) found: BTreeMap<EntryPath, SourceFile>,
    /// One verdict per mapping, in the order the mappings were given.
    pub(crate) roots: Vec<WalkedRoot>,
}

/// One mapping, and what the walk found its root to be.
pub(crate) struct WalkedRoot {
    pub(crate) mapping: Mapping,
    pub(crate) state: RootState,
}

/// What one mapped root turned out to be, before anything under it was read
/// (spec: EP-12).
pub(crate) enum RootState {
    /// The root is there and stands on the filesystem the mapping records.
    Available,
    /// The root is there and holds files, and the mapping records either no
    /// filesystem or a different one: the identity to stamp it with.
    Stamp(RootIdentity),
    /// Nothing under the root is evidence about anything.
    Unavailable(RootUnavailable),
}
```

`Walked` and `WalkedRoot` are `pub(crate)` like `SourceFile`
(`local_scan/mod.rs:17`), and re-exported from `local_scan/mod.rs` beside it.

The root check itself goes in a new `local_scan/root_state.rs`:

```rust
async fn root_state(mapping: &Mapping) -> Result<RootState, LocalError>
```

It states the root with `fs::metadata` — following links, which is what
`fs::read_dir` already does to the root, and unlike the `symlink_metadata` the
entries below it are stated with (`walk_mappings.rs:112`, spec: EP-8) — and
answers:

- `Err` whose kind is `NotFound` → `Unavailable(RootUnavailable::Missing)`.
- any other `Err` → `LocalError::io(LocalOperation::Stating, root, cause)`. A
  mapped root that is a regular file, or one whose parent is unreadable, still
  fails the run exactly as it does today; only absence is a verdict.
- `Ok(metadata)` → compute this platform's identity for it. Where the platform
  reports none, or where the mapping records none, or where the two are equal,
  `Available` — and where the mapping records none, `Stamp(current)` instead, so
  the first scan of a new mapping records what it saw. Where they differ, list
  the root: no directory entry at all is
  `Unavailable(RootUnavailable::AnotherFilesystem)`, and anything at all inside
  it is `Stamp(current)`.

"No directory entry at all" is deliberately the plainest possible test — not
"no regular file", and not "nothing a scan would back up". An unmounted mount
point is an empty directory, and widening the test would start guessing about
folders that hold something the walk passes over.

`walk_mappings` then calls `root_state` for each mapping, skips the walk for an
unavailable one, and — this is load-bearing — computes `claimed`
(`walk_mappings.rs:32`–`:36`) from **every** mapping's prefix, available or not.
A top-level mapping still represents its subtree while its drive is unplugged;
dropping its name from `claimed` would let the root mapping walk into the
folder that stands where the mapping's subtree belongs and commit Entry Paths
the other mapping owns (spec: EP-9). Say so where `claimed` is built.

Its rustdoc gains the reason the two guards are here at all, and the comment
quoted at the top of this task goes away with the arm it justified: the
`NotFound` arm inside the loop stays, but its comment is now only about a
directory that vanished mid-walk, because a missing root never reaches it. The
check command greps for the absence of the old sentence.

### What each flow reports

The report value is shared, because both flows have the same finding to make. A
new crate-root module `unavailable_root.rs`, `pub use`d from `lib.rs` and
re-exported from `sync` and `freeze` — exactly what `local_operation.rs` does
(`lib.rs:159`–`:160`, `sync/mod.rs:112`, `freeze/mod.rs:114`), and the paragraph
at `lib.rs:146`–`:151` that lists what those flows share and none owns gains it:

```rust
/// A mapping whose local root says nothing about the Library (spec: EP-12).
pub struct UnavailableRoot {
    /// The top-level component the mapping stands for, or `None` for the
    /// Library root.
    pub prefix: Option<EntryPath>,
    /// The folder on this device the mapping names.
    pub local_root: PathBuf,
    /// What made it unavailable.
    pub reason: RootUnavailable,
}

/// Why nothing under a mapped root may be read as evidence.
pub enum RootUnavailable {
    /// The root directory is not there.
    Missing,
    /// The root is there and empty, and stands on a filesystem the mapping does
    /// not record — what an unplugged disk or an unmounted share leaves behind.
    AnotherFilesystem,
}
```

`UnavailableRoot` derives `Debug, Clone, PartialEq, Eq` and `RootUnavailable`
derives `Debug, Clone, Copy, PartialEq, Eq` — the derives `Deferred`
(`sync/deferred.rs:13`) and `NotFrozen` (`freeze/not_frozen.rs:20`) carry, for
the same reason: a conformance case compares the whole finding. The rustdoc
carries the same warning those two do: `local_root` travels in the value because
the caller is what decides what to do about it, and it never travels into a log
line.

**It is a new field on each outcome and not a new variant of `Deferred`.** The
variant was considered and rejected: `Deferred` is documented as "a file the
sync found needing work it does not do" and each of its variants carries the
Entry Path of one file (`sync/deferred.rs:3`–`:12`). An unavailable root is a
finding about a *mapping*, and folding it in would make one list two granularities
— every existing `assert_eq!(outcome.deferred, vec![…])` would have to reason
about which kind each element is — and would put a local path inside a type whose
doc says the value it carries is an Entry Path. `NotFrozen` refuses it even more
plainly: "every one of these is update-eligible" (`freeze/not_frozen.rs:5`–`:9`),
which an unavailable root is not. So:

- `SyncOutcome` (`sync/sync_outcome.rs:21`–`:52`) gains
  `pub unavailable: Vec<UnavailableRoot>`, documented next to `deferred` as the
  other half of what a successful run has to be read for: a run that returns
  `Ok` with entries here has scanned less than the device's mappings cover, and
  has deliberately inferred no deletion under them (spec: EP-12, PK-14).
- `FreezeOutcome` (`freeze/freeze_outcome.rs:21`–`:43`) gains the same field,
  documented against the same obligation. A freeze does no deletion inference at
  all, so the only harm an unavailable root does it is silence — and silence is
  the one outcome PK-14 forbids, which is why freeze reports the mapping rather
  than refusing the run. Nothing else about a freeze changes: an unavailable
  root contributes no candidate, so it can neither absorb nor remove anything.
  Say that in `freeze/run.rs`'s rustdoc where the two reported-not-acted-on
  kinds are listed (`freeze/run.rs:48`–`:54`), and in `freeze/mod.rs`'s step 2
  (`freeze/mod.rs:20`–`:24`).
- `sync/survey.rs:13`–`:25` and `freeze/survey.rs:10`–`:21` each gain
  `unavailable: Vec<UnavailableRoot>`, which the run copies into the outcome
  (`sync/run.rs:87`–`:100`, `freeze/run.rs:118`–`:131`).
- Both `info!` lines gain `unavailable = outcome.unavailable.len()`
  (`sync/run.rs:106`–`:114`, `freeze/run.rs:132`–`:140`) and both `debug!` lines
  in the scans gain the same count (`sync/scan/mod.rs:54`–`:61`,
  `freeze/scan/mod.rs:92`–`:99`). A count and nothing else: the prefix is an
  Entry Path component and the root is a local path, and neither may reach a log
  line.

### What each scan does with the verdicts

`sync/scan/mod.rs:31`–`:63`, after the walk and before the deletion step:

1. For every `WalkedRoot` whose state is `RootState::Stamp(identity)`, call
   `index.set_mapping(Mapping { root_identity: Some(identity), ..mapping })`.
   The stamp is a write from a scan, which is already this step's habit — it
   refreshes the observation of every file that turned out unchanged
   (`sync/scan/mod.rs:22`–`:24`, `sync/run.rs:73`–`:75`) — and it is the flow
   that does it rather than the walk, because `LocalError`
   (`local_error.rs:17`–`:43`) has no vocabulary for a port failure and must not
   grow one for this. The two scans already return `SyncResult` / `FreezeResult`,
   both of which carry an `Index` variant.
2. Call the deletion step with the verdicts rather than the raw mappings.
3. Put one `UnavailableRoot` per `RootState::Unavailable` into the survey, in
   mapping order.

`freeze/scan/mod.rs:45`–`:101` does 1 and 3 and not 2: a freeze infers no
deletions.

`sync/scan/deletions.rs` becomes:

```rust
pub(super) async fn deletions(
    index: &dyn Index,
    roots: &[WalkedRoot],
    found: &BTreeMap<EntryPath, SourceFile>,
) -> SyncResult<Vec<Deferred>>
```

and applies the mapping partition the walk applies:

- Build `claimed: BTreeSet<&str>` from the prefix of every root, available or
  not — the same set, built the same way and for the same reason, as
  `walk_mappings.rs:32`–`:36`.
- Skip every root whose state is `Unavailable`.
- For a root with a prefix, `present_under(Some(prefix))` is already bounded to
  that subtree and every row it returns is considered.
- For the Library-root mapping, `present_under(None)` is the whole present set
  (`index.rs:195`–`:200`), so drop every row whose `path.top_level()`
  (`coffret_model::EntryPath::top_level`) is in `claimed`. That row belongs to
  the mapping that stands for that subtree, and if that mapping is unavailable
  the row is nobody's evidence.

Its rustdoc says both new things: that a mapping whose root the device cannot
vouch for produces no deletion, and that the root mapping accounts for the
remainder rather than for the whole namespace — the same partition the walk uses,
stated in the same words, so the two cannot drift.

### The port shape

`Mapping` (`device_state/mapping.rs:18`–`:25`) gains one field:

```rust
pub struct Mapping {
    pub prefix: Option<EntryPath>,
    pub local_root: PathBuf,
    /// What the filesystem under `local_root` was when a scan last saw it, or
    /// `None` where no scan has yet seen it (spec: EP-12).
    pub root_identity: Option<RootIdentity>,
}
```

Its type doc gains the paragraph the field needs: a mapping asserts nothing
about what is on disk (which it already says, `mapping.rs:14`–`:17`), and this is
the one thing it does assert — not that the root holds anything, but *which*
filesystem it stood on, which is what lets an empty root be told apart from a
folder that was emptied.

New `device_state/root_identity.rs`, a newtype in `BatchId`'s shape
(`device_state/batch_id.rs:14`–`:33`) and re-exported from `device_state/mod.rs`
in its alphabetical place:

```rust
pub struct RootIdentity(String);

impl RootIdentity {
    pub fn new(identity: impl Into<String>) -> Self;
    pub fn as_str(&self) -> &str;
}
```

with `#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]` and a
`Display`, as `BatchId` has. Its rustdoc is where the opacity is stated: what a
platform can say about "the filesystem this folder stands on" differs between
platforms, so this is compared only against another value this device recorded
and is never parsed, split, or ordered against meaning. It never leaves the
device — no Journal record or Snapshot carries it (spec: CK-7).

The one place that knows what is inside it is a private function beside the root
check, in `local_scan/root_state.rs`:

```rust
/// What this platform can say about the filesystem one folder stands on, or
/// `None` where it can say nothing (spec: EP-12).
#[cfg(unix)]
fn identity_of(metadata: &std::fs::Metadata) -> Option<RootIdentity> {
    use std::os::unix::fs::MetadataExt;
    Some(RootIdentity::new(format!("unix-dev:{}", metadata.dev())))
}

#[cfg(not(unix))]
fn identity_of(_metadata: &std::fs::Metadata) -> Option<RootIdentity> {
    None
}
```

The `unix-dev:` tag is not decoration: it is what keeps a value one platform
recorded from ever comparing equal to a value another platform's form happened to
spell the same way, which matters because the comparison is what decides whether
deletion inference runs.

`Index::set_mapping` (`index.rs:163`–`:165`) keeps its signature. Its rustdoc
gains what the stored identity means for it: the mapping is stored exactly as
given, identity included, so recording a mapping again with no identity is how a
device says "this root is what I meant" after a run reported it unavailable, and
the next scan stamps whatever is there (spec: EP-12). No new port operation: the
scan's re-stamp is a `set_mapping` like any other.

**SQLite adapter.** The `mappings` table (`schema.rs:75`–`:82`) gains

```sql
    -- What the filesystem under `local_root` was when a scan last saw it, in
    -- whatever opaque form the platform could state (spec: EP-12). NULL until a
    -- scan has seen it, and NULL again whenever the mapping is recorded afresh
    -- — which is how a device re-confirms a root a run reported unavailable.
    root_identity TEXT
```

`device_state::set_mapping` (`device_state.rs:23`–`:38`) carries it in the
`INSERT`; the existing `DELETE` + `INSERT` pair already replaces the whole row,
so a mapping recorded with no identity clears the column with no extra
statement — note that in the function's rustdoc, since it is what makes the
re-confirmation gesture work. `rows::mapping` (`rows.rs:166`–`:172`) reads it
with `optional_text(row, "root_identity", OPERATION)?.map(RootIdentity::new)`,
the shape it already uses for `prefix`.

**Bump `SCHEMA_VERSION` to 3** (`schema.rs:18`). Required for the reason the
constant's own rustdoc gives (`schema.rs:14`–`:17`): `prepare` opens a file
already at the stamped version untouched (`schema.rs:138`), so leaving it at 2
would open an Index file whose `mappings` table has no `root_identity` column and
fail on the first read of it with a backend error saying nothing about why.
Bumping makes such a file `UnsupportedSchema`, which is the discard-and-rebuild
answer the module doc prescribes (`schema.rs:6`–`:12`). Update
`coffret-sqlite-index/tests/schema.rs:111` (`supported: 2` → `supported: 3`).

**In-memory adapter.** `in_memory_index/state.rs:30` currently keys local roots
by prefix (`BTreeMap<Option<EntryPath>, PathBuf>`), which cannot carry a third
value; make it `BTreeMap<Option<EntryPath>, Mapping>` keyed by
`mapping.prefix.clone()`. `set_mapping` (`state.rs:152`–`:154`) takes the whole
`Mapping`, `mappings` (`state.rs:156`–`:158`) yields `&Mapping`, and
`in_memory_index/mod.rs:93`–`:108` stops taking the value apart and putting it
back together. `sync_conformance/refusing_index.rs:88`–`:94` and
`sync_conformance/watching_index.rs:120`–`:126` pass `Mapping` through and need
no change.

### Spec register

Read `docs/spec/entry-path/README.md` first. This is a **new rule, EP-12**, and
not a sub-bullet under EP-10. EP-10 states which Entries a scan may say anything
about, and this task does not change that condition — it adds a second,
independent one, with its own persisted device state, its own reporting
obligation, and its own bearing on both flows. It also has to be *citable*: the
walk, the deletion step, `Mapping`, `RootIdentity`, `UnavailableRoot`, the two
outcomes, and the `mappings` DDL all cite a rule ID for it, and a sub-bullet has
no ID to cite. The register's own posture is that "every rule is a discrete
statement with a stable ID" (`docs/spec/README.md:48`–`:53`), and EP-10's
existing sub-bullet is an illustration of EP-10 rather than an additional
obligation.

Append to the rules list, after EP-11, following the register's citation and
*(Form: …)* conventions exactly — plain-text rule IDs, no links to other rules:

```markdown
- **EP-12.** Reporting an Entry as deleted locally (EP-10) requires the mapped
  root it stands under to be *available*: the root directory exists, and the
  filesystem it stands on is the one recorded for that mapping (EP-9). A device
  records that filesystem's identity per mapping, stamped by the scan that first
  sees the root and re-stamped whenever a root holding files stands on a
  different one; the identity is device state and is never uploaded (CK-7). A
  mapping whose root is missing, or whose root holds nothing and stands on a
  filesystem other than the recorded one, is unavailable: nothing under it is
  walked, no Entry under it is reported as deleted locally, no file under it is
  selected for `update` or `freeze`, and the run reports the mapping and the
  reason rather than returning silently — an unplugged disk or an unmounted
  share must never read as the user having emptied the folder. The device's
  other mappings scan normally, and a top-level mapping that is unavailable
  still represents its subtree, so the Library-root mapping neither walks it nor
  infers deletions under it (EP-9). *(Form: test)*
  - A root that holds files and stands on a filesystem other than the recorded
    one is available and is re-stamped: a device number that moved across a
    reboot or a remount is not evidence that a folder went away. The two
    conditions are asymmetric deliberately — every way the comparison can be
    wrong ends in skipping deletion inference or in re-stamping, never in
    reporting a deletion the recorded identity would not have. The state this
    leaves for a person is a folder genuinely emptied whose filesystem identity
    also moved, and the gesture is recording the mapping again, which clears the
    identity so the next scan stamps what is there.
  - What the rule does not cover: a mount point whose underlying directory is
    not empty reads as available and is re-stamped, because there is nothing to
    distinguish it from a folder holding files. That is the behavior of a device
    with no recorded identity at all, so the rule never makes such a root worse.
```

Nothing else in `docs/spec/` changes.

### Concept documents

Two concept documents carry the user-visible reliance sentence this changes, and
they are deliberately two views of one rule — `docs/concepts/entry-path/README.md:58`–`:60`
says so outright. Both get one sentence, and nothing else in either file moves:

- `docs/concepts/entry-path/README.md`, appended to the deletion bullet at
  `:48`–`:52`: "Reporting one also requires the mapped root to be available —
  present, and standing on the filesystem the mapping recorded — so an unplugged
  disk or an unmounted share is reported as an unavailable root rather than read
  as an emptied folder (spec: EP-12)."
- `docs/concepts/library/README.md`, appended to the deletion bullet at
  `:60`–`:64`: the same sentence, so the two accounts stay one rule seen twice.

### Tests

Every case below is a conformance case in the existing house style — a
`pub async fn` taking the suite's fixture, exported from the suite's `mod.rs` and
listed in its declaring macro, so both the in-memory run under `make check` and
the MinIO run under `make s3-store-it` execute it.

**How a case makes an unavailable root.** The suite fixtures hand over one
folder each (`SyncUnderTest::folder`, `FreezeUnderTest::source_folder`), so a
case that needs to remove a mapped root maps a *subdirectory* of it and removes
that — the same move `a_top_level_mapping_takes_its_subtree_from_the_root_mapping`
already makes to get two local roots side by side
(`sync_conformance/import.rs:161`–`:172`). Add to
`sync_conformance/fixtures.rs`, beside `map` (`:84`–`:93`):

- `map_at(fixture, prefix, local_root) -> ()`, recording a mapping onto a
  directory the case names rather than onto the whole fixture folder, so the two
  new multi-mapping cases do not each hand-roll a `set_mapping`.
- `mappings(index) -> Vec<Mapping>`, beside `pending` (`:213`–`:218`), so a case
  can assert what a run stamped.

An identity mismatch is arranged without a mount by recording the mapping with
an identity no filesystem has: `Mapping { root_identity: Some(RootIdentity::new("unix-dev:0")), .. }`.
That is what an unmount looks like to the comparison, it needs no privileges, and
it runs the same in memory and against MinIO. Say so in a comment on the first
case that does it, and say what the real shape is that it stands for.

**`sync_conformance/roots.rs`** — a new module, exported from
`sync_conformance/mod.rs` and added to the macro's case list (`:114`–`:135`):

- `a_missing_mapped_root_is_reported_and_infers_no_deletion` — two mappings, one
  at the Library root over one subdirectory and one at `albums` over another,
  each holding a file. A first sync commits both. The `albums` local root is then
  removed, and a second sync: `outcome.unavailable` is exactly one
  `UnavailableRoot { prefix: Some("albums"), local_root, reason: Missing }`,
  `outcome.deferred` is empty, `outcome.commit` is `None`, the Entry under
  `albums/` is still current, and its local row is still `Present` — nothing
  inferred and nothing written down. The root mapping's own file is still
  reported `unchanged`, so the available mapping scanned normally.
- `an_empty_root_on_another_filesystem_is_reported_and_infers_no_deletion` — one
  mapping over a subdirectory holding a file, synced. The mapping is then
  re-recorded with the same local root, an identity of `unix-dev:0`, and the file
  removed so the root is empty. The next sync reports one `UnavailableRoot` with
  `reason: AnotherFilesystem` and no `Deferred::DeletedLocally`, and leaves the
  mapping's recorded identity as it found it — an empty root is never re-stamped.
- `an_emptied_folder_on_the_recorded_filesystem_still_reports_its_deletions` —
  the legitimate mass delete, and the case that keeps the guard from swallowing
  the behavior it is guarding. Sync a subdirectory holding two files, let the
  scan stamp the mapping, remove both files, and sync again: `unavailable` is
  empty and `deferred` is exactly the two `DeletedLocally` findings in Entry Path
  order. Then remove the root directory itself and sync a third time: now
  `deferred` is empty and `unavailable` holds the one `Missing` root — the same
  folder, the two answers the rule distinguishes.
- `a_renumbered_root_that_holds_files_is_restamped_and_scans_normally` — a
  mapping recorded with an identity of `unix-dev:0` over a subdirectory holding
  a file. One sync: `unavailable` is empty, the file is committed, and
  `mappings(index)` shows the mapping carrying an identity that is neither
  `None` nor `unix-dev:0`. A second sync reports nothing unavailable and finds
  the file unchanged, so the stamp stuck.
- `an_unavailable_top_level_mapping_holds_its_subtree_back_from_the_root_mapping`
  — the leak the naive fix leaves. Both mappings as in the first case, both
  synced, then the `albums` local root removed. The second sync reports the
  `albums` mapping unavailable and **no** `DeletedLocally` for
  `albums/spring.jpg`, even though the root mapping's `present_under(None)` names
  that row. Assert positively that the root mapping still reports its own
  deletion when its own file goes: remove the root-mapped file too and check
  that exactly one `DeletedLocally` is reported, for that path and not for the
  one under `albums/`.
- `a_mapping_recorded_afresh_clears_its_identity_and_reports_the_deletions` — the
  recovery gesture. From the state
  `an_empty_root_on_another_filesystem_is_reported_and_infers_no_deletion`
  leaves, record the same mapping again with `root_identity: None` and sync: the
  root is now available, `unavailable` is empty, and the file that really is gone
  is reported `DeletedLocally`.

**`freeze_conformance/roots.rs`** — a new module, exported from
`freeze_conformance/mod.rs` and added to its macro's case list (`:101`–`:117`):

- `a_missing_mapped_root_is_surfaced_by_a_freeze` — map a subdirectory of the
  source folder, write enough files to cut more than one Pack at `TARGET`, freeze
  once and check the Packs landed. Remove the local root and freeze again:
  `outcome.unavailable` holds the one `Missing` root, and every other field is
  the empty answer a freeze with nothing to do gives — `packs` empty, `absorbed`
  empty, `packed_already: 0`, `surfaced` empty, `commit: None`. The point of the
  case is that the two are told apart: assert in the same case that a *second*
  freeze over the intact folder gives that same empty answer with `unavailable`
  empty too, which is what makes the new field the only thing distinguishing
  "already packed" from "the disk is not there".
- `an_empty_root_on_another_filesystem_is_surfaced_by_a_freeze` — the same with
  the identity arranged as in the sync suite: one `AnotherFilesystem` root
  surfaced, nothing packed, nothing absorbed, and the Packs the first run built
  untouched on Storage (the suite's `counting_store` is what already asserts
  "left alone", `freeze_conformance/mod.rs:27`–`:31`).

**`index_conformance`** (`index_conformance/mod.rs:31`–`:37` and the macro list
at `:82`–`:106`):

- `fixtures::mapping` (`index_conformance/fixtures.rs:161`–`:166`) keeps its
  signature and produces `root_identity: None`. Add
  `stamped(prefix, local_root, identity)` beside it, in the shape `provisional`
  stands to `pending` (`fixtures.rs:188`–`:199`).
- `seed_device_state` (`index_conformance/device_state.rs:12`–`:25`) seeds the
  *stamped* mapping, and `assert_device_state_intact` (`:28`–`:53`) compares it,
  so `a_refused_operation_leaves_the_whole_catalog_as_it_was`
  (`index_conformance/refusals.rs:113`) covers the new column with no change of
  its own — the same way it already covers `pending_uploads.state`.
- New `a_mapping_round_trips_its_root_identity`: record a stamped mapping and
  read it back whole; record the same prefix again with a different identity and
  see the identity moved with the rest of the row; record it once more with
  `None` and see the identity cleared rather than kept — which is the
  re-confirmation gesture the port promises (spec: EP-12). Keep
  `a_mapping_is_kept_once_per_prefix` (`device_state.rs:218`–`:245`) as it is:
  one mapping per prefix is a different claim.
- The schema refusal is not a conformance case and does not become one: it is
  `coffret-sqlite-index/tests/schema.rs::a_file_from_another_layout_is_refused`,
  and it only needs its `supported: 2` moved to `3`. That test drives
  `SqliteIndex::open` directly, which the port-level suite cannot.

**Unit test in the walk.** `local_scan/walk_mappings.rs`'s `tests` module
(`:143`–`:197`) is the one place the distinction between the root and a
subdirectory can be checked without a Library, so add one there beside the
existing case: `a_missing_root_is_not_an_empty_root` — one mapping over a
directory that was never created, asserting the walk returns no files and one
`RootState::Unavailable(RootUnavailable::Missing)`, and a second mapping over a
directory that exists and holds one file, asserting it comes back `Stamp(_)` on
unix with its file found. The comment carries EP-12 and the reason: the walk
used to answer both with `continue`, and the two answers are what the whole rule
rests on.

### Conventions

`CLAUDE.md` is authoritative: English documentation, comments, commit messages,
and PR text; Conventional Commits; no `PartialEq` on error types; a test per
variant that a caller matches on; `make check` as the gate, plus
`make s3-store-it` for the MinIO run of the sync, freeze, commit, fetch, store,
and index suites. `coffret-logging`'s rule holds for anything logged here:
counts, Container IDs, object names, generations, byte totals — never Entry
Paths, local paths, plaintext, or key material, which is why the new outcome
field is logged as a length and never as its contents. Commit and PR text must be
self-contained.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A mapped root that is not there is a verdict and not an empty folder:
      `sync_conformance::a_missing_mapped_root_is_reported_and_infers_no_deletion`
      passes — the run reports one `UnavailableRoot` with `reason: Missing`,
      reports no `Deferred::DeletedLocally`, commits nothing, leaves every Entry
      under that mapping current with its local row still `Present`, and scans
      the device's other mapping normally. The unit case
      `local_scan::walk_mappings::tests::a_missing_root_is_not_an_empty_root`
      passes, pinning the distinction between a missing root and a subdirectory
      that vanished mid-walk.
- [x] An empty root standing on a filesystem the mapping does not record gets the
      same treatment:
      `sync_conformance::an_empty_root_on_another_filesystem_is_reported_and_infers_no_deletion`
      passes, reporting one `AnotherFilesystem` root, no deletions, and leaving
      the recorded identity untouched.
- [x] The legitimate mass delete is unaffected:
      `sync_conformance::an_emptied_folder_on_the_recorded_filesystem_still_reports_its_deletions`
      passes — an emptied folder on the recorded filesystem reports every
      `DeletedLocally` it did before, and removing the root itself turns the same
      folder into an unavailable root with no deletions.
- [x] A device number that moved does not cost a folder its backup:
      `sync_conformance::a_renumbered_root_that_holds_files_is_restamped_and_scans_normally`
      passes — the run reports nothing unavailable, commits the file, and leaves
      the mapping stamped with what it saw, and the next run finds the file
      unchanged.
- [x] The mapping partition holds on the deletion side:
      `sync_conformance::an_unavailable_top_level_mapping_holds_its_subtree_back_from_the_root_mapping`
      passes — the Library-root mapping infers no deletion under an unavailable
      top-level mapping's prefix, and still reports its own.
- [x] The recovery gesture works and is the only one needed:
      `sync_conformance::a_mapping_recorded_afresh_clears_its_identity_and_reports_the_deletions`
      passes, and `index_conformance::a_mapping_round_trips_its_root_identity`
      passes against the in-memory catalog and against SQLite — including
      recording a mapping with no identity over one that had one and seeing the
      identity cleared.
- [x] A freeze over an unavailable root is not silent:
      `freeze_conformance::a_missing_mapped_root_is_surfaced_by_a_freeze` and
      `freeze_conformance::an_empty_root_on_another_filesystem_is_surfaced_by_a_freeze`
      pass — each reports the mapping in `FreezeOutcome::unavailable`, packs
      nothing, absorbs nothing, surfaces nothing else, commits nothing, and
      leaves the Packs an earlier run built untouched on Storage; and a second
      freeze over the *intact* folder gives the same empty answer with
      `unavailable` empty, so the field is what distinguishes the two.
- [x] The port carries the identity and every adapter stores it identically:
      `Mapping::root_identity` is an `Option<RootIdentity>`, the SQLite
      `mappings` table has a `root_identity` column, the in-memory catalog keeps
      it, and `index_conformance::a_refused_operation_leaves_the_whole_catalog_as_it_was`
      passes with a stamped mapping inside the row it compares. The check
      command independently requires `root_identity` to appear in the DDL and
      `SCHEMA_VERSION` to be 3, and
      `coffret-sqlite-index/tests/schema.rs::a_file_from_another_layout_is_refused`
      passes against `supported: 3`, so an Index file from an older build is
      refused rather than read with a missing column.
- [x] The rustdoc describes the guards that exist: `walk_mappings`,
      `local_scan/root_state.rs`, `sync/scan/deletions.rs`,
      `device_state/mapping.rs`, `device_state/root_identity.rs`,
      `unavailable_root.rs`, `index.rs` (`set_mapping`),
      `coffret-sqlite-index/src/device_state.rs` (`set_mapping`),
      `sync/sync_outcome.rs`, `freeze/freeze_outcome.rs`, `sync/mod.rs` (step 2),
      `freeze/mod.rs` (step 2), `sync/run.rs`, and `freeze/run.rs` each say what
      an unavailable root is and what it stops. Gated mechanically by the check
      command: the sentence `A mapped root that is not there holds no files` does
      not occur anywhere under `backend/`, and
      `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items`
      is clean.
- [x] `docs/spec/entry-path/README.md` carries EP-12 with its two sub-bullets, no
      existing rule is renumbered or reworded, and the check command requires the
      ID to be present. `docs/concepts/entry-path/README.md` and
      `docs/concepts/library/README.md` each carry the one appended sentence, and
      no other file under `docs/spec/` or `docs/concepts/` is touched.
- [x] Every pre-existing suite still passes under `make check` and
      `make s3-store-it` — sync, freeze, commit, fetch, store, and index —
      including `sync_conformance::a_file_deleted_locally_is_surfaced_and_untouched`
      and `sync_conformance::a_top_level_mapping_takes_its_subtree_from_the_root_mapping`
      unchanged in meaning, and no error type gained `PartialEq`.

## Out of scope

- **Acting on a deletion.** Propagating one into a Pack is read-modify-replace
  (PK-9..PK-12), which neither flow does; this task only stops a deletion being
  *inferred* where the device cannot vouch for the folder.
- **Any Storage-side behavior.** No object is trashed, purged, listed, or
  re-uploaded differently because a root is unavailable, and nothing about
  orphan cleanup changes.
- **NFC normalization of Entry Paths** (EP-1), which `EntryPath` still does not
  perform and which is not made worse or better here.
- **Renaming `Deferred` to `Surfaced`** to match `FreezeOutcome::surfaced`. The
  asymmetry is real and predates this task; renaming a public type is its own
  change with its own diff.
- **Identity sources beyond `st_dev`.** A platform that cannot report one
  records nothing and is guarded by the missing-root check alone. The stored form
  stays opaque so a later platform can record something else, and choosing what
  that something else is belongs to whichever release targets it.
- **A fetch's placement into an unavailable root.** `fetch/translate.rs:35`
  reads the same mappings and creates the directories it needs, so a fetch onto
  an unmounted mount point writes onto the underlying disk instead. That is the
  same family of bug with a different remedy — a fetch has no deletion to infer
  and a partly-written folder to worry about instead — and it is not this task's
  to design.
- **A command-line or UI surface for recording mappings.** The recovery gesture
  this task defines is `Index::set_mapping` with no identity; giving a person a
  way to invoke it is the first release's product work, not this fix.
