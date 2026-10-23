---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, rust-module-structure, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "LocalOperation::Locking" backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_destination.rs && grep -q "classify" backend/crates/apps/coffret-desktop-core/src/door/submission.rs && grep -q "AlreadyApplied" backend/crates/domain/coffret-usecase/src/commit/run.rs'
assignee: null
branch: task/1023-1855-settle-four-edges-of-commit-refresh-locking-and-the-door
created_at: 2026-10-23T09:55:00Z
updated_at: 2026-10-24T02:20:00Z
---

# fix: settle four edges — a lost commit response, a refresh another process got to first, a lock the filesystem cannot give, and what the door logs

## Overview

Four decisions taken on review findings, each about an edge the code meets
rarely but wrongly. Implement them as written. Paths are relative to
`backend/crates/`.

### 1. A commit whose response was lost is recognised as committed

`domain/coffret-usecase/src/commit/journal.rs:207-237`: `put_if_absent` runs
under the retry policy. When the put succeeded but its response was lost, the
retry receives `AlreadyExists` for the run's own record and the run answers
`Attempted::Conflict`, rebasing onto what is its own head and then trying to
commit the same batch again.

- On `AlreadyExists`, read the slot back the way `commit/slot_occupant.rs:66-130`
  does (listing, authenticated read, decode). If the record there names
  exactly this batch's additions (Container IDs are never reused, CP-14; OC-3
  already treats "a record at the slot that names the Container" as proof the
  batch committed there), answer `Attempted::Committed` with the record read
  back; otherwise `Conflict` as today. A batch with no additions (removals or
  renames only) cannot be recognised this way and stays `Conflict`; say so in a
  comment with the reason.
- Spec: add to CP-3 (`docs/spec/commit-protocol/README.md`) that a writer
  refused at the slot reads it back and, where the record names its own
  additions, has committed. Keep the rule id; it is a sentence inside CP-3.
- Test: `commit_conformance/` — add a `Fault` for the head slot that writes the
  bytes and then answers `AlreadyExists` (the snapshot slot has `SiblingSnapshot`
  in `faulty_store.rs`), and a case that the run reports `Committed` once and
  does not commit the batch twice.

### 2. A refresh that finds its record already applied completes anyway

`commit/run.rs:235-241` (and `:165-172` after a first head) calls
`index.refresh(CommittedBatch { record, materialized, released })`, and the
SQLite `refresh` (`library_state/apply.rs:158-181`) applies the record, marks
the materialized rows present, clears the pending rows and releases the
reservations in one transaction. When another process has applied the same
record first, `apply` returns `AlreadyApplied` at the top and the other three
steps never run; the run fails with a `CommitError` and the next sync's OC-7
completion fills in the rows but not the reservations.

- When `refresh` answers `AlreadyApplied`, the run performs the device-side
  part — mark present, clear pending rows, release reservations — and
  completes as a success; the record is in the Index already. Add that as one
  Index operation (`refresh` taking the already-applied case into the same
  transaction, or a sibling that skips `apply`), implemented by the SQLite and
  the in-memory Index, so the device-side steps stay atomic.
- `index_conformance/commit.rs` and `index_conformance/refusals.rs:160-190`
  gain the case; `commit/catch_up_tests.rs:799` stays as it is (replay is
  unchanged).

### 3. A lock the filesystem cannot give leaves nothing behind and says why

`gateway/coffret-local-fs/src/unix_destinations/unix_destination.rs:30-66`
creates the scratch with `O_EXCL` and then takes a non-blocking exclusive
`flock`; when the lock fails it refuses, with the empty scratch left on disk,
as `LocalOperation::Creating`.

- On a failed `flock`, unlink the scratch just made (`unlinkat` on the folder
  handle) before refusing; a failure of that unlink is logged at `warn` and
  the refusal is still the lock's.
- Refuse as `LocalOperation::Locking` (it exists; `server_lock.rs:79-86` uses
  it), so the line reads "<folder> could not be locked".
- Where the device's `Error::Local` is displayed with the `Locking` operation
  (`coffret-device/src/error/display.rs:36`) and where the server maps it
  (`coffret-server/src/api_error/from_error.rs:76`, today a 500 "the server
  could not answer"), say that the folder's filesystem provides no file lock
  and that a mapped folder has to be on a local disk. The server answers it as
  a refusal the explorer can show, not a 500.
- Refusing is the decision; no path continues without a lock.
- Test: `destinations_conformance/` gains a fake whose `flock` fails
  (`in_memory_fs` `fail_on(LocalOperation::Locking, …)`), asserting the
  refusal's operation and that no scratch remains; a CLI answer test asserts
  the wording.

### 4. The door logs why a submission was not JSON

`apps/coffret-desktop-core/src/door/submission.rs:47-85` maps every body
failure to `(Refusal, &'static str)` and discards the cause (`:66`, `:75`), so
`Refusal::answer` (`refusal.rs:60-67`) logs only the fixed reason word. Log,
as fixed words, the `serde_json::Error::classify()` category (`io`, `syntax`,
`data`, `eof`) and, for `io`, the `io::ErrorKind`; for a broken frame, the
axum error's kind if it exposes one, else `broken`. Never the error's Display,
line or column, since Display may carry body text (EL-1, EL-3, EL-4). Pass
them as fields of the same `door_refusal` event, extending `answer` rather
than emitting a second event. `door/tests/events.rs` asserts the new fields.

### Out of scope

- Recognising a lost response for a batch without additions.
- Continuing without a lock on any filesystem.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Commit conformance: a head slot fault that stores the bytes and answers `AlreadyExists` makes the run report `Committed` once, with the record read back, and the Journal holds the batch once; a slot taken by a different writer's record still rebases. CP-3 carries the read-back sentence and `make spec-citations` passes.
- [x] Index conformance (SQLite and in-memory): a refresh after another process applied the record marks present, clears pending rows and releases reservations, and the commit run completes (`grep -q "AlreadyApplied"` on `commit/run.rs` is appended to `check_command`).
- [x] Destinations conformance: a failed lock leaves no scratch, refuses as `Locking`, and the device's line and the server's answer say the filesystem gives no lock and the folder must be on a local disk (`grep -q "LocalOperation::Locking"` on `unix_destination.rs` is appended).
- [x] Door events test: a non-JSON body logs `door_refusal` with the classification word and, for an I/O failure, the error kind, and no body text (`grep -q "classify"` on `submission.rs` is appended).
