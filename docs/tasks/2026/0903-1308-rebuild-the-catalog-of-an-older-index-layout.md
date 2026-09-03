---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rq 'DEVICE_SCHEMA_VERSION' backend/crates/gateway/coffret-sqlite-index/src && grep -rq 'an_older_library_layout_is_discarded_and_the_device_state_kept' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'a_discarded_catalog_is_rebuilt_by_the_next_catch_up' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'a_layout_older_than_the_device_state_is_refused' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'a_layout_from_a_newer_build_is_refused' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'a_second_connection_finds_the_rebuilt_layout' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'the_discard_is_logged_without_a_path' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'RefusedIndex' backend/crates/gateway/coffret-sqlite-index/src && grep -rq 'mappings_are_read_from_a_refused_file' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'reading_a_refused_file_leaves_it_as_it_was' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'a_refused_file_needs_only_the_two_columns_every_layout_keeps' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'mappings_are_still_listed_when_the_index_is_refused' backend/crates/apps/coffret-device/src"
assignee: null
branch: task/0903-1308-rebuild-the-catalog-of-an-older-index-layout
created_at: 2026-09-03T13:08:49Z
updated_at: 2026-09-03T19:06:41Z
---

# fix(sqlite-index): discard an older catalog layout instead of refusing the file

## Overview

The Index file carries one layout stamp, `SCHEMA_VERSION` in
`backend/crates/gateway/coffret-sqlite-index/src/schema.rs`, and its doc
comment states the policy: there is no migration path, because the
Index is a cache that can be rebuilt exactly from Storage (spec: RV-5),
so "a file this build does not understand is discarded rather than
converted". `schema::prepare` does not do that. For any stamp other than
`0` or the current one it returns `IndexError::UnsupportedSchema`, and
`SqliteIndex::open` fails, so `coffret sync`, `coffret fetch`, and the
server exit with

```
the Index file is at schema version 4, this build reads 5
```

and nothing tells the owner what to do next. Every Library created
before the stamp moved from 4 to 5 (the `btime` column on `entries`) is
in this state today, and every future bump repeats it.

The policy cannot be applied to the whole file, and that is the actual
design problem. The DDL is two groups, and only the first is a cache:

- **Library-wide** — `checkpoint`, `containers`, `entries`. Identical on
  every device and carried by Index Snapshots and Journal records; a
  catalog with no checkpoint is rebuilt by the next catch-up exactly
  the way a freshly created file is (spec: CK-7, RV-5).
- **Device-local** — `mappings`, `local_entries`, `pending_uploads`.
  Never uploaded and not reconstructible from Storage (spec: EP-9,
  EP-10, OC-2). Deleting the file deletes the device's mappings, which
  is what happened when the file was removed by hand as a workaround
  (`coffret mappings` answered "Nothing is mapped yet" afterwards), and
  a `pending_uploads` row is the *only* record of a spool an interrupted
  run left behind (spec: OC-7).

So "discard" has to mean: discard the Library-wide group and keep the
device-local group, and it can only mean that when the device-local
group in the old file is a layout this build reads. Make that decision
explicit:

1. **Stamp the device-local group with its own floor.** Keep the single
   `user_version` stamp and `SCHEMA_VERSION` as it is (this task changes
   no DDL, so the stamp stays `5`). Add a second constant,
   `DEVICE_SCHEMA_VERSION`, the stamp at which the device-local group
   last changed — `4` today (`#39` → 2, `#40` → 3, `#45` → 4 each
   touched a device-local table; `#65` → 5 touched only `entries`).
   Its doc comment states the maintenance rule: every change to a
   device-local table, or to the vocabulary its values are spelled in,
   moves `DEVICE_SCHEMA_VERSION` to the new `SCHEMA_VERSION`; a change
   confined to the Library-wide group leaves it where it is. Split the
   `DDL` string into the two groups so that the Library-wide group can
   be recreated on its own and the divider comment becomes a real
   boundary.
2. **Decide the file by its stamp.** `prepare` handles four cases:
   - `0` — a new file: create both groups and stamp, as now.
   - `SCHEMA_VERSION` — open untouched, as now.
   - `DEVICE_SCHEMA_VERSION..SCHEMA_VERSION` (an older layout whose
     device-local group this build reads) — **discard and rebuild**:
     in one `BEGIN IMMEDIATE` transaction, re-read the stamp (a second
     process racing to open the same old file must find the new stamp
     and do nothing, not rebuild twice), drop `entries`, then
     `containers`, then `checkpoint` (in that order, because
     `foreign_keys` is on and `entries` references `containers`),
     recreate the Library-wide group from its DDL, and stamp
     `SCHEMA_VERSION`. The device-local tables are not touched, not
     even read. A crash mid-way leaves the old file, which is rebuilt
     on the next open.
   - anything else — a stamp below `DEVICE_SCHEMA_VERSION` (a
     device-local layout this build cannot read) or above
     `SCHEMA_VERSION` (a file from a newer build) — **refuse**, as now,
     with `UnsupportedSchema`.
3. **Say what happened, and what to do.** The rebuild emits one
   `tracing` event (add `tracing` to the crate, as the Storage gateways
   have it) at `WARN`, with `operation`, `found`, and `supported` as
   fields and a message saying the catalog of an older layout was
   discarded and the next sync rebuilds it from Storage. No file path
   and no Entry Path in the event — the Index file lives under the
   state directory and the log's redaction rule is that nothing naming
   the owner's files goes in. The refusal's `Display` in
   `coffret-usecase/src/index_error.rs` gains the recovery: the file
   cannot be carried forward; record the device's mappings, delete the
   Index file, map again, and sync — stated in the domain's own terms
   (it must not name CLI subcommands; `coffret-device`'s
   `Error::Index` and the CLI already print the cause under their own
   head line). Add a variant field only if the message needs one — the
   `found` / `supported` pair already there is enough to say which of
   the two refusals it is, so prefer a message that distinguishes "older
   than this build can carry forward" from "newer than this build".
4. **Keep the doc comments true.** `SCHEMA_VERSION`'s comment,
   `SqliteIndex::open`'s comment ("refused ... rather than migrated"),
   the `tests/schema.rs` module doc, and the `UnsupportedSchema` /
   `UnreadableCatalog` docs in `index_error.rs` all describe the
   refuse-everything behaviour. Rewrite them to the two-group decision
   above. The `docs/concepts/index/` Domain Rule "The Index is a cache,
   never the source of truth" already carves the device's state out
   ("kept beside the catalog, not in it"); if the concept doc needs a
   sentence for "an older layout is discarded to the extent it is a
   cache", report it — do not edit the concept doc in this task.
5. **Tests that fix the cases**, in `tests/schema.rs` (or a sibling
   integration test file beside it — the check command greps the
   `tests/` directory). Build each old file the way the existing
   `a_file_from_another_layout_is_refused` case does: open with
   `rusqlite` directly, create the current DDL, write rows, restamp
   `user_version` to the stamp under test. Use these names:
   - `an_older_library_layout_is_discarded_and_the_device_state_kept`
     — a file at `DEVICE_SCHEMA_VERSION` holding a checkpoint, a
     container, an entry, a mapping, a `local_entries` row and a
     `pending_uploads` row opens; afterwards `checkpoint()` is `None`,
     `entries_under(None)` is empty, and the mapping, the local entry
     and the pending row read back unchanged through the port; the
     stamp is `SCHEMA_VERSION`.
   - `a_discarded_catalog_is_rebuilt_by_the_next_catch_up` — after the
     discard, `restore(snapshot)` + `apply(record)` leave the catalog
     at the same state a fresh file reaches by the same calls, while
     the mapping written before the discard is still there.
   - `a_layout_older_than_the_device_state_is_refused` — a file at
     `DEVICE_SCHEMA_VERSION - 1` is refused with `UnsupportedSchema`
     and **its rows are untouched** (reopen with `rusqlite` and count
     them): refusing must not half-discard.
   - `a_layout_from_a_newer_build_is_refused` — the existing `99` case,
     renamed to say which refusal it is.
   - `a_second_connection_finds_the_rebuilt_layout` — open an old
     file through `SqliteIndex::open` twice (two connections, the way
     `tests/sharing.rs` stages two processes); the second open finds
     the current stamp and the mapping is still exactly one row.
   - `the_discard_is_logged_without_a_path` — with
     `coffret_logging::testing::CapturedLogs` (dev-dependency with the
     `testing` feature, as `google-drive-store` does), the discard
     emits exactly one `WARN` event carrying `found` and `supported`,
     and the event text contains neither the temp directory's path nor
     `index.sqlite`.

7. **Make the recovery of a refused file possible.** The recovery in 3
   begins with "note where this device maps the Library onto its
   folders", but the only way to list mappings is through the same
   `prepare` that refused the file, so the first step is a dead end
   (found by walking the refusal as the owner). The mappings are the one
   piece of device state the owner cannot recreate from memory, and the
   two columns that carry them — `mappings.prefix` and
   `mappings.local_root` — have existed unchanged since layout 1. State beside
   `DEVICE_SCHEMA_VERSION` that every layout keeps a `mappings` table
   with those two columns readable by name, whatever else changes — **the
   two columns every layout keeps**. (Not "salvage": that word is the
   spec's name for presenting Container contents without control state,
   RV-4, and this is unrelated to it.) Then:
   - **Gateway.** Add a small public type `RefusedIndex` in its own
     module of `coffret-sqlite-index`: `RefusedIndex::open(path)` opens
     the file read-only (rusqlite
     `OpenFlags::SQLITE_OPEN_READ_ONLY`; if a WAL-mode file cannot be
     opened read-only because its `-shm` is absent, fall back to a plain
     open that still runs no `prepare`), and `.mappings()` runs `SELECT
     prefix, local_root FROM mappings ORDER BY prefix IS NOT NULL,
     prefix` reading the two columns by name and returns them with
     `root_identity: None` (a mapping read this way is recorded afresh, so the next scan stamps it —
     the same as `set_mapping`). No `prepare`, no journal-mode change,
     no stamp check: the file is left byte-for-byte as it was, which
     the refusal promised.
   - **Device.** `coffret_device::mappings` currently returns
     `Vec<Mapping>` and fails on a refused file. Make it return a listing
     type (one public type, its own module) that says which of the two
     happened: the mappings as recorded, or the mappings read from a
     file `open` refused with `IndexError::UnsupportedSchema`, carrying
     that refusal so the caller can show why. Any other open error is
     still an error. Adapt every caller (the CLI; check whether the
     server or its tests call it).
   - **CLI.** `coffret mappings` prints the mappings on stdout as it does
     now in both cases; in the refused case it also prints, on stderr,
     that the Library's Index cannot be opened by this build (the
     refusal's own text) and that these mappings were read from it
     directly, followed by the recovery as commands: delete the Index
     file, `coffret map` each mapping again, `coffret sync`. The CLI is
     the layer that may name subcommands.
   - **Domain text.** Change `RECOVERY` in `index_error.rs` so its first
     step no longer implies a command that cannot run: the mappings can
     still be listed from the refused file; then delete the Index file,
     record them again, and catch up from Storage.
   - **Tests.** `coffret-sqlite-index/tests/` (a sibling file beside
     `schema.rs` is fine): `mappings_are_read_from_a_refused_file`
     (a file at `DEVICE_SCHEMA_VERSION - 1` holding two mappings yields
     both, root first, `root_identity` `None`);
     `reading_a_refused_file_leaves_it_as_it_was` (stamp and row counts
     unchanged afterwards; a following `SqliteIndex::open` is still
     refused); `a_refused_file_needs_only_the_two_columns_every_layout_keeps`
     (a `mappings` table with only `prefix` and `local_root`, the
     layout-1 shape, and one with an extra unknown column both read).
     In `coffret-device` (`mapping/tests.rs`, using the existing
     `state_dir` / `create_s3` scaffolding):
     `mappings_are_still_listed_when_the_index_is_refused` (map
     two folders, restamp the Index file below `DEVICE_SCHEMA_VERSION`
     through rusqlite, list: the refused-file variant with both mappings and
     the `UnsupportedSchema` refusal).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `schema.rs` carries `DEVICE_SCHEMA_VERSION` beside
      `SCHEMA_VERSION`, with the DDL split into the Library-wide and
      device-local groups (grep gate on the constant name).
- [x] Opening a file stamped in `DEVICE_SCHEMA_VERSION..SCHEMA_VERSION`
      discards the Library-wide tables, keeps every device-local row
      byte-for-byte, and restamps the file (test
      `an_older_library_layout_is_discarded_and_the_device_state_kept`).
- [x] The discarded catalog rebuilds through the ordinary
      `restore` / `apply` path to the same state a fresh file reaches
      (test `a_discarded_catalog_is_rebuilt_by_the_next_catch_up`).
- [x] A stamp below `DEVICE_SCHEMA_VERSION` and a stamp above
      `SCHEMA_VERSION` are each refused with `UnsupportedSchema`, and a
      refused file is left exactly as it was (tests
      `a_layout_older_than_the_device_state_is_refused`,
      `a_layout_from_a_newer_build_is_refused`).
- [x] Two connections opening the same old file rebuild it once (test
      `a_second_connection_finds_the_rebuilt_layout`).
- [x] The discard is logged as one `WARN` event with `found` and
      `supported` fields and no path in it (test
      `the_discard_is_logged_without_a_path`).
- [x] `UnsupportedSchema`'s `Display` tells the owner the recovery
      (record mappings, delete the Index file, map again, sync) without
      naming a CLI subcommand, and `make check` passes.

- [x] `RefusedIndex` reads `prefix` / `local_root` from a refused file
      without `prepare`, returns the mappings root first
      with no `root_identity`, leaves the file unchanged, and needs only
      the two columns every layout keeps (tests
      `mappings_are_read_from_a_refused_file`,
      `reading_a_refused_file_leaves_it_as_it_was`,
      `a_refused_file_needs_only_the_two_columns_every_layout_keeps`; grep
      gates).
- [x] `coffret_device::mappings` returns the mappings read from the file together
      with the `UnsupportedSchema` refusal when the Index is refused,
      and the recorded mappings otherwise (test
      `mappings_are_still_listed_when_the_index_is_refused`; grep
      gate).
- [x] The two `mappings` columns every layout keeps are stated as a rule beside `DEVICE_SCHEMA_VERSION`, and `RECOVERY`
      no longer asks the owner for anything the refused file cannot
      give (reading the diff; covered by `make check` only for
      compilation).

### Manual / on-hardware (verified by a human before merge)

- [ ] On this device, `coffret sync` against one of the Libraries
      created before the 4 → 5 bump (the state directory's `main`,
      `second`, or `test`) opens the file, logs the discard, catches up
      from Drive, and `coffret mappings` still lists the mappings that
      were recorded before — no `map` was needed.
- [ ] `coffret mappings` against a Library whose Index file has been
      restamped below `DEVICE_SCHEMA_VERSION` (a copy of one of the above
      with `PRAGMA user_version = 3`) lists the mappings on stdout and
      prints the refusal and the recovery commands on stderr; the file's
      stamp is unchanged afterwards.

## Out of scope

- A migration of any table, in either group. The policy stays "no
  conversion code"; this task only makes the discard as narrow as the
  cache actually is.
- Carrying a device-local group forward across a change to *its*
  layout. When `DEVICE_SCHEMA_VERSION` moves, files below it are
  refused with the recovery message; deciding whether some such change
  deserves better is a decision for that change.
- Bumping `SCHEMA_VERSION`. Nothing in the DDL changes here.
- Reading `local_entries` or `pending_uploads` out of a refused file.
  Materialized-file records are rebuilt by the next scan; a pending row
  lost with the file leaves at most a spool file on disk and an
  uncommitted object in Storage, which the existing orphan rules bound.
  Only the mappings are unrecoverable by any other means, so only they
  are read out.
- Rewriting a refused file in place (reading the mappings out and
  rebuilding without deleting the file). Deleting and re-mapping is a short, explicit
  recovery; an automatic rewrite of a file this build cannot read is a
  decision for a change that needs it.
