---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && (cd backend && RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items) && ! git grep -q -e 'before anything is uploaded' -e 'dies before the row exists' -- backend && for f in backend/crates/domain/coffret-usecase/src/freeze/spool.rs backend/crates/domain/coffret-usecase/src/sync/spool.rs; do test $(grep -n record_pending_upload $f | tail -1 | cut -d: -f1) -lt $(grep -n SpoolFile::create $f | head -1 | cut -d: -f1) || exit 1; done"
assignee: null
branch: task/0825-2054-track-spools-before-writing
created_at: 2026-08-25T11:54:32Z
updated_at: 2026-08-25T13:53:31Z
---

# fix(backend): record the pending row before a spool file exists

## Overview

Both spool steps write the whole Container to disk, flush it, and digest it
**first**, and only then write the pending row that is the one thing able to
find that file again:

- `freeze/spool.rs:56` creates the spool file, `:71`–`:102` streams the Pack
  into it, `:103` flushes it, and `:106`–`:114` records the row.
- `sync/spool.rs:61` creates it, `:62` writes the whole Container, `:63`
  flushes it, and `:66`–`:74` records the row.

`SpoolFile` has no `Drop` guard (`spool_file.rs:24`–`:82`); the only thing that
removes a spool file is an explicit `spool_file::discard`
(`spool_file.rs:98`–`:104`). And nothing in production code lists the spool
directory: reconcile discovers spools **only** through `index.pending_uploads()`
(`sync/reconcile.rs:82`), which is the sole reader of those rows.

So every failure between file creation and the row is a permanent local leak.
The paths that reach it are ordinary, not exotic:

- `FreezeError::SourceChanged`, raised from `freeze/spool.rs:92`–`:96` (short
  read) and from `moved`/`closing` (`freeze/spool.rs:177`–`:198`) when a member
  file moves under the run;
- any I/O failure from `SpoolFile::create`, `SpoolFile::write`, or
  `SpoolFile::finish` (`spool_file.rs:34`–`:81`) — a full disk being the obvious
  one;
- a source open or read failure in the member loop (`freeze/spool.rs:76`–`:79`);
- a `wrap_container_key` failure (`freeze/spool.rs:104`, `sync/spool.rs:64`);
- a process crash or kill anywhere in the middle, which no error path covers at
  all.

Each leaves a `<container_id>.spool` file that no row names and that reconcile
can never reach. For a `freeze` oversized singleton (PK-3) that is a multi-GB
file silently eating the device's disk, with no operation anywhere in the
codebase that will ever reclaim it.

**The rustdoc already promises the opposite.** Four passages describe an
ordering the code does not implement:

- `freeze/spool.rs:20`–`:23` — "the Container is written first and the pending
  row is recorded before anything is uploaded, so a run that dies mid-upload
  leaves a row naming what it created". True of a run that dies mid-*upload*;
  the run that dies mid-*spool* leaves a row naming nothing.
- `sync/spool.rs:20`–`:26` — the same claim, and then the hole stated outright:
  "A run that dies before the row exists leaves a spool file nothing names,
  which is why the spool directory belongs to the sync and to nothing else."
  Belonging to the sync does not make the file reclaimable; nothing ever looks
  in that directory.
- `freeze/freeze_error.rs:79`–`:93` — `SourceChanged` says "the Pack is
  abandoned in the spool, where this device's own pending row accounts for it
  (spec: OC-2)". On that exact path there is no row: `SourceChanged` is raised
  strictly before `freeze/spool.rs:106`.
- `freeze/freeze_request.rs:24`–`:29` — `spool_dir` says "the sync flow deletes
  the ones an interrupted run left behind (spec: OC-2)". Only the row-tracked
  ones. `freeze/run.rs:63`–`:70` makes the same claim at flow level, and
  `sync/sync_request.rs:24`–`:28` makes it for the sync's own directory.

This task makes the code do what the documentation says.

### The decided fix: the row comes first

**Record the pending row before the spool file is created.** Not before it is
finished — before it exists. Then there is no window at all: from the instant a
spool file can be on disk, a row names it, and every interruption — error,
panic, or kill -9 — leaves state the next reconcile can settle.

The order in both spool steps becomes:

1. Draw the Container ID and key, and compute `spool_path`.
2. `index.record_pending_upload(...)` with the row in state `Writing` and
   `object_ref: None`.
3. `SpoolFile::create(&spool_path)`.
4. Write the content (whole-file for the sync, streamed for a Pack).
5. `SpoolFile::finish()`.
6. `index.complete_pending_spool(container_id)` — the row becomes `Written`.
7. `wrap_container_key(...)`, then build the `SpooledContainer`.

Step 7 is after step 6 deliberately: the row's state then tells the truth about
the file at the moment the file changed, and a wrap failure leaves a `Written`
row over a complete spool rather than a `Writing` row over one.

What happens *before* step 2 needs no row, because nothing is on disk yet:
`generate_container_id` / `generate_container_key` failures, and the sync's
whole-file read at `sync/spool.rs:39`, all leave the device exactly as they
found it. The `fs::create_dir_all(&spool_dir)` that precedes the loop
(`sync/run.rs:60`, `freeze/run.rs:90`) stays where it is, for the same reason.

Disposal must therefore tolerate a row whose spool file does not exist. It
already does: `spool_file::discard` treats `NotFound` as the same outcome as a
removal it performed (`spool_file.rs:101`). **Verify this and change nothing
there** — the tolerance is the reason row-first is safe, so say so in that
function's rustdoc rather than re-deriving it.

### The port shape

Note first what the row does *not* carry: `PendingUpload`
(`device_state/pending_upload.rs:25`–`:40`) holds `container_id`, `spool_path`,
`batch`, `created_at`, and `object_ref`. The digests `SpoolFile::finish`
produces go to `SpooledContainer`, never to the Index. So **every existing field
is already known before the file is created** and nothing has to become
optional. The only thing that changes at `finish` is whether the file is
complete, and that is what the row must be able to say.

Prescribed shape — an explicit state, mirroring `LocalEntryState`
(`device_state/local_entry_state.rs`) and the `mark_present` / `mark_absent`
pair it serves, which is the port's established idiom for exactly this
(a full upsert plus a narrow state flip):

1. **New** `backend/crates/domain/coffret-usecase/src/device_state/pending_spool_state.rs`:

   ```rust
   pub enum PendingSpoolState { Writing, Written }
   ```

   with `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]` — the same derives
   `LocalEntryState` carries (`local_entry_state.rs:13`). Re-export it from
   `device_state/mod.rs` beside `LocalEntryState`. Its rustdoc says what the two
   states are evidence of: `Writing` is a spool file this device announced and
   may or may not have finished writing, so its content is worth nothing and
   only its disposal matters; `Written` is a complete Container, the only kind
   that is ever uploaded or committed.

2. `PendingUpload` gains `pub state: PendingSpoolState`. Document on the field
   the invariant that ties it to `object_ref`: a Container is uploaded only
   after its spool is complete, so `Writing` always comes with
   `object_ref: None`, and an `object_ref` is only ever set on a `Written` row.

3. `Index` (`index.rs`) gains **one** operation, placed directly after
   `record_pending_upload` (`index.rs:215`):

   ```rust
   /// Records that one Container's spool file is complete (spec: OC-2).
   async fn complete_pending_spool(&self, container_id: ContainerId) -> IndexResult<()>;
   ```

   Its contract, following `mark_absent`'s precedent (`index.rs:175`–`:183`)
   exactly: it is an update and not an upsert, and a Container with no row
   changes nothing rather than failing — the operation says that a row which
   exists is complete, and inventing one would record a spool the flow never
   announced. No new `IndexError` variant.

   `record_pending_upload`'s rustdoc (`index.rs:211`–`:215`) must be reworded:
   it no longer records a Container "encrypted, and perhaps uploaded" but one
   this device is about to write, has written, or has uploaded — the row's state
   saying which.

4. **SQLite adapter.** `pending_uploads` gains `state TEXT NOT NULL`
   (`schema.rs:92`–`:100`), spelled the way `local_entries.state` is
   (`schema.rs:86`). Add a `rows::spool_state_text(PendingSpoolState)`
   alongside `rows::state_text`, mapping to `"writing"` / `"written"`, and read
   it back in `rows::pending_upload` (`rows.rs:182`–`:192`) with the same
   `match … found => return Err(unreadable(...))` shape `rows::local_entry` uses
   (`rows.rs:174`–`:178`). `device_state::record_pending_upload`
   (`device_state.rs:163`–`:189`) carries the column in both the `VALUES` list
   and the `ON CONFLICT DO UPDATE SET` list. New
   `device_state::complete_pending_spool` is
   `UPDATE pending_uploads SET state = ?2 WHERE container_id = ?1`, with
   `mark_absent`'s reasoning (`device_state.rs:78`–`:101`) restated for this
   row rather than copied.

   **Bump `SCHEMA_VERSION` to 2** (`schema.rs:13`). This is required, not
   cosmetic: `prepare` accepts a file already at the stamped version untouched
   (`schema.rs:129`), so leaving it at 1 would open an older Index file whose
   `pending_uploads` has no `state` column and fail later with a confusing
   backend error. Bumping makes such a file `UnsupportedSchema`, which is the
   discard-and-rebuild answer the module doc already prescribes
   (`schema.rs:6`–`:12`). Update anything that names the version.

5. **In-memory adapter.** `in_memory_index/state.rs:200`–`:210` gains
   `complete_pending_spool`, mutating the stored row's state in place where one
   is there and doing nothing where none is; `in_memory_index/mod.rs:132`–`:144`
   wires it. `sync_conformance/refusing_index.rs:116`–`:126` gains the
   pass-through.

6. **`upload/run.rs:57`–`:65`** re-records the row with the object handle; it
   now passes `state: PendingSpoolState::Written`. Its rustdoc already explains
   why the row is updated as soon as each upload lands (`upload/run.rs:28`–`:33`)
   and needs no change beyond the field.

### How the flow keeps provisional rows out of the batch

Upload, verification, and commit act on the `Vec<SpooledContainer>` a run built
(`sync/run.rs`, `freeze/run.rs:88`–`:97`, `upload/run.rs:40`,
`spooled_container.rs:106`), never on pending rows. A `SpooledContainer` is
returned only by a `spool` call that reached step 7, so it exists only for a
spool whose row says `Written`. Make that explicit rather than incidental:
state it in `SpooledContainer`'s rustdoc (`spooled_container.rs:16`) and pin it
with the conformance case below.

Across runs, the only reader of rows is reconcile, and it must treat a `Writing`
row as this device's own reclaimable leftovers:

- Replace the bare membership test at `sync/reconcile.rs:101` with a single
  predicate — e.g. `fn completes(row: &PendingUpload, current: &BTreeSet<ContainerId>) -> bool`
  that is `row.state == Written && current.contains(&row.container_id)` — and
  use that same function in `materialized` (`sync/reconcile.rs:132`–`:155`) so
  the two cannot drift. A `Writing` row is disposed of whatever the current set
  says: a Container whose spool never finished was never uploaded, so it cannot
  be current, and disposing of it is OC-2's posture over this device's own
  provenance.
- `dispose` (`sync/reconcile.rs:206`–`:252`) needs no logic change. A `Writing`
  row's `object_ref` is `None`, so nothing is trashed, and `discard` tolerates
  the missing or partial file. Its rustdoc gains the case.
- The head read stays keyed off `object_ref.is_some()`
  (`sync/reconcile.rs:87`). A `Writing` row never carries one, so the ordinary
  interrupted-mid-spool run still asks the Index one question and stops
  (spec: OC-6). Say so where that condition is.
- The module rustdoc's "**two** things a pending row can turn out to be"
  (`sync/reconcile.rs:20`–`:37`) becomes three: a spool that was never
  finished, a finished spool whose batch was abandoned, and a batch that
  committed while its refresh did not. The first two are disposed of
  identically; only the third is completed.

### Rustdoc alignment

Once the ordering is implemented, the promises above stop lying. Reword only
where the *mechanism* description changed — do not rewrite passages that were
already true:

- `freeze/spool.rs:20`–`:23` and `sync/spool.rs:20`–`:26`: the row is recorded
  before the spool file is created, so no window exists in which ciphertext sits
  on disk unnamed. **Delete** `sync/spool.rs`'s "A run that dies before the row
  exists leaves a spool file nothing names" sentence — the state it describes is
  no longer reachable. Keep the rule that nothing but the flow may write into
  the spool directory; drop the now-void justification for it.
- `freeze/freeze_error.rs:84`–`:86`: the `SourceChanged` claim becomes true.
  Make it precise — the row was recorded before the first byte was written, so
  the abandoned Pack is accounted for however far the write got.
- `freeze/freeze_request.rs:27`–`:28`, `freeze/run.rs:63`–`:70`, and
  `sync/sync_request.rs:26`–`:28`: verify each reads as true without
  qualification and sharpen only where it still implies the old order.
- `device_state/pending_upload.rs:8`–`:40`: the type doc says when the row is
  written (before the spool file exists) and what its two states are evidence
  of.
- `spool_file.rs:98`–`:104` (`discard`) and `spool_file.rs:34`–`:46`
  (`create`): note that absence is a valid outcome because a row may name a
  file whose creation never happened.
- `schema.rs:92`–`:100`: the table comment says the row precedes the file.

Two phrases must be gone from `backend/` afterwards, and the check command
greps for their absence: `before anything is uploaded` and
`dies before the row exists`.

### Spec register

Read `docs/spec/orphan-cleanup/README.md` first. OC-2 requires "positive local
provenance that identifies the creating batch" for cleanup but says nothing
about *when* that provenance is written, and row-first does not conflict with
any existing rule — so **do not renumber, reword, or add a rule ID**. What is
genuinely missing is the ordering statement this task establishes, and the
register's own convention for that is a sub-bullet under the rule it refines,
as OC-6 already carries one. Add exactly one sub-bullet under OC-2:

```markdown
  - The provenance is recorded before the artifact it accounts for exists: a
    device writes the row naming a Container it is about to spool before the
    spool file is created, so every local ciphertext it produces is named by a
    row from the moment it can exist, and an interruption at any point leaves
    nothing cleanup cannot reach.
```

Nothing else in `docs/spec/` and nothing in `docs/concepts/` changes.

### Tests

Every case below is a conformance case in the existing house style — a `pub
async fn` taking the suite's fixture, exported from the suite's `mod.rs` and
listed in its declaring macro so both the in-memory run under `make check` and
the MinIO run under `make s3-store-it` execute it.

**One shared fault-injecting Index wrapper.** Add
`sync_conformance/watching_index.rs` holding `WatchingIndex`, built on
`RefusingIndex`'s pattern (`sync_conformance/refusing_index.rs`): it wraps
whatever catalog the backend handed the suite and passes everything through,
except that

- `record_pending_upload` for a row whose state is `Writing` **asserts** that
  `pending.spool_path` does not yet exist and that `pending.object_ref` is
  `None`, panicking with a message naming the ordering invariant if either
  fails, and counts the row (an `AtomicUsize`, readable as
  `provisional_rows()`);
- `complete_pending_spool` returns `IndexError::Backend { operation: "completing a spool", .. }`
  when the wrapper was built to refuse it, which stops a run at the one point
  that leaves a spool file plus a row behind.

Declare the module `pub(crate) mod watching_index;` and let
`freeze_conformance` borrow the type rather than write a second copy — the same
cross-suite borrowing `freeze_conformance/fixtures.rs:260,268` already does for
`container_handle` and `lose_key`, and which `freeze_conformance/mod.rs:32`–`:33`
documents as the suite's policy. Also add a `pending(index)` helper to
`freeze_conformance/fixtures.rs`, mirroring
`sync_conformance/fixtures.rs:212`–`:216`; `spooled(spool)` already exists at
`freeze_conformance/fixtures.rs:287`.

**`sync_conformance/interruption.rs`** (registered in
`sync_conformance/mod.rs:59`–`:64` and the macro list at `:117`–`:120`):

- `a_row_precedes_the_first_byte_of_a_spool` — an ordinary successful sync of a
  folder driven through an unarmed `WatchingIndex`; asserts the run committed
  its files and that `provisional_rows()` equals the number of Containers it
  spooled, so every row really was announced before its file existed.
- `an_unfinished_spool_is_disposed_with_its_row` — the same sync driven through
  a `WatchingIndex` that refuses completion. The call fails; before the next
  run, the spool directory holds one file and `pending_uploads()` holds exactly
  one row for it, in state `Writing` with `object_ref: None`. A second
  `sync_folders` against the unwrapped catalog then reports
  `Reconciled::Disposed { trashed: false }` for that Container, leaves the spool
  directory empty and no pending rows, commits the source file exactly once
  under a *different* Container, and never names the abandoned Container in the
  record or puts it on Storage.
- `a_provisional_row_whose_spool_was_never_created_is_disposed` — a hand-planted
  `Writing` row (the `interrupted` helper at
  `sync_conformance/interruption.rs:228`–`:264`, extended to plant a
  state) whose `spool_path` was never created; one sync disposes of it with no
  error, and a second finds nothing to do (spec: OC-6). Keep the existing
  `a_stale_pending_row_is_dropped_with_its_spool`
  (`sync_conformance/interruption.rs:184`–`:224`) as it is — it covers the
  `Written` row whose file has since vanished, which is a different shape.

**`freeze_conformance/interruption.rs`** — a new module for the suite, exported
from `freeze_conformance/mod.rs` and added to the macro's case list
(`freeze_conformance/mod.rs:91`–`:103`). The suite's `request` fixture
(`freeze_conformance/fixtures.rs:74`) already takes an `&dyn Index`, so a case
can call `freeze_folder(request(fixture.store(), &watching, …))` directly and
match on the failure:

- `a_row_precedes_the_first_byte_of_a_pack_spool` — a freeze at `TARGET` over
  enough files to cut several Packs, through an unarmed `WatchingIndex`;
  `provisional_rows()` equals the number of Packs the outcome reports.
- `an_unfinished_pack_spool_is_disposed_with_its_row` — the same freeze through
  a `WatchingIndex` that refuses completion. The freeze fails with
  `FreezeError::Index(IndexError::Backend { .. })`, leaving one spool file and
  one `Writing` row. A freeze does not settle rows itself
  (`freeze/run.rs:63`–`:70`), so the next `sync_source` is what disposes of
  them: it reports `Reconciled::Disposed { trashed: false }` for the abandoned
  Pack, empties the spool directory, clears the rows, and never commits that
  Container.
- `a_provisional_pack_row_is_never_uploaded_or_committed` — from that same
  interrupted state, assert before reconciling that Storage holds no object for
  the abandoned Container, and after reconciling that no Journal record names
  it.

**`index_conformance`** (`index_conformance/mod.rs:32`–`:36` and the macro list
at `:94`):

- Extend `fixtures::pending` (`index_conformance/fixtures.rs:171`–`:183`) to
  produce a `Written` row — its even/odd `object_ref` rule is unchanged — and
  add a `provisional(seed, batch)` fixture producing a `Writing` row with
  `object_ref: None`.
- `a_spool_is_recorded_until_its_batch_settles`
  (`index_conformance/device_state.rs:246`–`:283`) keeps its meaning and now
  round-trips the state as part of the row equality it already asserts.
- New `a_provisional_spool_row_becomes_written_when_it_completes`: record a
  `Writing` row, read it back as `Writing`, `complete_pending_spool` it, read it
  back as `Written` with every other field untouched, complete it a second time
  and see no change (spec: OC-6), and complete a Container the catalog holds no
  row for and see the catalog unchanged rather than a new row or an error.
- The atomicity posture stays where it is:
  `a_refused_operation_leaves_the_whole_catalog_as_it_was`
  (`index_conformance/refusals.rs:113`) compares the whole seeded row through
  `seed_device_state` / `assert_device_state_intact`
  (`index_conformance/device_state.rs:11`–`:50`), so the new column is covered
  by it once the fixture carries one.

### Conventions

`CLAUDE.md` is authoritative: English documentation, comments, commit messages,
and PR text; Conventional Commits; no `PartialEq` on error types; a test per
variant that a caller matches on; `make check` as the gate, plus
`make s3-store-it` for the MinIO run of the sync, freeze, and index suites.
`coffret-logging`'s rule holds for anything logged here: counts, Container IDs,
object names, generations, byte totals — never Entry Paths, local paths,
plaintext, or key material. Commit and PR text must be self-contained.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Both spool steps record the pending row before the spool file is created:
      `sync_conformance::a_row_precedes_the_first_byte_of_a_spool` and
      `freeze_conformance::a_row_precedes_the_first_byte_of_a_pack_spool` pass,
      each asserting through `WatchingIndex` that every `Writing` row named a
      path that did not yet exist and carried no `object_ref`, and that one such
      row was announced per Container the run spooled. The check command's
      source-order gate independently requires `record_pending_upload` to
      precede `SpoolFile::create` in `freeze/spool.rs` and `sync/spool.rs`.
- [x] An interruption between the row and a finished spool leaves nothing
      unreclaimable: `sync_conformance::an_unfinished_spool_is_disposed_with_its_row`
      and `freeze_conformance::an_unfinished_pack_spool_is_disposed_with_its_row`
      pass — the failed run leaves exactly one `Writing` row naming the file on
      disk, and the next run disposes of both, empties the spool directory,
      clears the rows, and commits the source content exactly once under a
      different Container.
- [x] A provisional row whose spool file was never created is disposed of
      without error and idempotently:
      `sync_conformance::a_provisional_row_whose_spool_was_never_created_is_disposed`
      passes, and the pre-existing
      `sync_conformance::a_stale_pending_row_is_dropped_with_its_spool` still
      passes unchanged in meaning.
- [x] A provisional row is never uploaded, verified, or committed:
      `freeze_conformance::a_provisional_pack_row_is_never_uploaded_or_committed`
      passes — Storage holds no object for the abandoned Container before
      reconciliation and no Journal record names it after.
- [x] The port carries the distinction and every adapter implements it
      identically: `Index::complete_pending_spool` exists,
      `PendingUpload::state` is a `PendingSpoolState`, and
      `index_conformance::a_provisional_spool_row_becomes_written_when_it_completes`
      passes against the in-memory catalog and against SQLite — including
      completing an already-complete row and completing one the catalog holds no
      row for, neither of which changes anything. The SQLite schema version is
      bumped so an Index file from an older build is refused rather than read
      with a missing column.
- [x] The Index port's atomicity posture is unweakened:
      `index_conformance::a_refused_operation_leaves_the_whole_catalog_as_it_was`
      passes with the new column inside the row it compares, and
      `index_conformance::a_spool_is_recorded_until_its_batch_settles` round-trips
      the state.
- [x] The rustdoc describes the ordering that is implemented: the passages at
      `freeze/spool.rs`, `sync/spool.rs`, `freeze/freeze_error.rs`
      (`SourceChanged`), `freeze/freeze_request.rs`, `freeze/run.rs`,
      `sync/sync_request.rs`, `device_state/pending_upload.rs`, `index.rs`
      (`record_pending_upload`), `spooled_container.rs`, and
      `sync/reconcile.rs` (three row shapes, not two) say the row precedes the
      first byte. Gated mechanically by the check command: neither
      `before anything is uploaded` nor `dies before the row exists` occurs
      anywhere under `backend/`, and
      `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items`
      is clean.
- [x] OC-2 in `docs/spec/orphan-cleanup/README.md` carries the ordering
      sub-bullet, with no rule renumbered, reworded, or added, and no other
      file under `docs/spec/` or `docs/concepts/` touched.
- [x] Every pre-existing suite still passes under `make check` and
      `make s3-store-it` — sync, freeze, commit, fetch, store, and index — and
      no error type gained `PartialEq`.

## Out of scope

- **Scanning the spool directory for orphaned files as a fallback.** Row-first
  makes a rowless spool file structurally impossible, so a directory scan would
  only ever find files left by a build from before this change; that migration
  question is not this task's.
- **The `ContainerWriter` output contract** — the streaming encoder's documented
  behavior is already correct and is not touched here.
- **Pack update and deletion propagation** (PK-9..PK-12), the read-modify-
  replace machinery over Entries already held by Packs.
- **Keyring repair** and key-loss recovery of any kind.
- **Any error-type restructuring** beyond the fields the new port operation
  needs: no new `IndexError` variant, no reshaping of `FreezeError`,
  `SyncError`, `UploadError`, or `LocalError`, and no change to how
  `SourceChanged` is raised or classified.
- **Resuming a spool an interrupted run left behind.** It stays impossible for
  the reason `sync/reconcile.rs:39`–`:54` gives: the Container Key lived only in
  the run that drew it.
