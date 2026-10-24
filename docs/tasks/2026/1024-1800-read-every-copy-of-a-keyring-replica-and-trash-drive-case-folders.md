---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "KL-14" backend/crates/domain/coffret-usecase/src/commit_conformance/'
assignee: null
branch: task/1024-1800-read-every-copy-of-a-keyring-replica-and-trash-drive-case-folders
created_at: 2026-10-24T09:00:00Z
updated_at: 2026-10-24T09:52:08Z
---

# fix: read every copy of a Keyring replica a listing names twice, and trash the Drive test case folders

## Overview

The commit conformance suite was run against real Google Drive for the first
time, and it found two defects that no in-memory run can show. They land
together because they come from the same run.

### 1. A Keyring replica written twice is read through one copy only (KL-14)

On a Storage whose names are not unique (Google Drive), one Keyring replica
name can carry two objects: a repair writes a valid copy beside the one that
did not read back valid. KL-14 (`docs/spec/keyring-lifecycle/README.md`)
says either copy that reads back valid is the replica. The code does not do
that. `ControlListing::record` in
`backend/crates/domain/coffret-usecase/src/commit/control_listing.rs` keeps
one handle per name, the one listed last, and the Keyring read goes through
that handle alone. The in-memory minting double lists objects in creation
order, so the repaired copy always comes last there and every run passes.
Drive lists them in another order. The corrupt copy is then the one read, so
the next commit finds the position unusable again and repairs it again,
adding one more duplicate each time. The real-Drive run's
`an_unreadable_replica_is_replaced` case fails this way: after the repair,
the replica it reads back is the 28-byte corrupt one.

Fix: keep every handle a listing gives for a Keyring replica name, and have
every reader of Keyring replicas try the copies of a position until one reads
back valid. The commit path's catch-up and repair, the fetch path, and any
other reader of `key-*` objects (grep for where replica handles are looked
up) must all do this. A position counts as unusable only when no copy of it
reads back valid. Heads and Index Snapshots listed twice stay refused as they
are. Hold it in the commit conformance suite with a case that does not depend
on listing order: for example, a wrapper store that lists objects newest
first, or that lists a chosen object last, around the store under test. The
case is "a position whose name carries a corrupt copy and a valid copy reads
as the valid one, whichever the listing gives last" (cite KL-14). It must
fail against today's code on the in-memory runs. Keep the `debug!` note when
a name is listed twice. If KL-14's text needs a sentence to say a reader
tries every copy, add it.

### 2. The Drive test case folders are never trashed

`CaseFolder`'s `Drop` in
`backend/crates/gateway/google-drive-store/tests/support/mod.rs` trashes the
case's subfolder from a new current-thread runtime on a scoped thread. The
`GoogleDrive` it calls was built on the case's own runtime, and that runtime
is shutting down by then. Its HTTP client's connections belong to the dead
runtime, so the call waits out the whole-call deadline. Every case of the
last real-Drive run printed `could not trash the case folder …: the call
timed out`, so 35 subfolders were left in the test folder. Make the removal
work: for example, build a fresh client for the removal inside the removal's
runtime from the same settings, or trash the folder explicitly at the end of
each case while its runtime is alive and keep `Drop` as the fallback. A
failed removal is still printed, not panicked over.

### Out of scope

- Removing duplicate Keyring copies from Storage. Reading the right one is
  the fix; orphan cleanup has its own rules.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A commit conformance case holds KL-14 independently of listing order:
      a position with a corrupt and a valid copy reads as valid whichever is
      listed last, and is not repaired again. It fails on the in-memory runs
      against the code before this change (the work phase demonstrates
      that), and it cites KL-14 (gate on `KL-14` in `commit_conformance/`
      appended).
- [x] Every reader of Keyring replicas tries every copy a listing gives for
      a position, and the existing commit, fetch and sync suites still pass
      on the in-memory, minting and MinIO runs that `make check` covers.
- [x] `CaseFolder`'s removal no longer goes through a client bound to the
      case's runtime (reading the diff shows the new path), and the support
      module still compiles with the Drive variables unset.

### Before merge (verified outside the check command)

- [ ] On this macOS machine, `make drive-store-it` passes, and no line of
      its output says `could not trash the case folder` (an agent may run
      it: the person granted the test client for this). The merging session
      records the counts and timings in the PR.
