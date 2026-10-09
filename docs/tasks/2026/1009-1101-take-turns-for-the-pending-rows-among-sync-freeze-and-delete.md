---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq until_pending_rows_are_free backend/crates/apps/coffret-server/src/'
assignee: null
branch: task/1009-1101-take-turns-for-the-pending-rows-among-sync-freeze-and-delete
created_at: 2026-10-09T11:01:00Z
updated_at: 2026-10-09T12:52:00Z
---

# fix(server): let sync, freeze and delete take turns on the pending rows instead of stopping the second one as a server failure

## Overview

A sync, a freeze and a deletion each own this device's pending rows for the
whole of their run: `own_pending_rows` is the first thing
`coffret-usecase/src/sync/run.rs`, `freeze/run.rs` and `delete/run.rs` do,
and the Index refuses a second owner at once with
`IndexError::PendingRowsBusy` rather than making it wait (spec: OC-2; the
SQLite adapter's `pending_ownership.rs` uses a non-blocking file lock). That
is right for the use case — another *process* may be the owner, and it
cannot be waited on blindly — but the server runs the three flows on three
independent workers (`coffret-server/src/{sync,freeze,delete}/worker.rs`),
so it competes with itself. A file dropped while a book is being packed
starts a sync that fails on its first line; the explorer's work answer then
shows `could not back up what was added — the server could not answer`
(`PendingRowsBusy` is mapped through `catalog_unusable` → `ApiError::server`,
500, in `api_error/from_error.rs`), and the person is offered a retry for
something that never had a chance.

`delete/run.rs` already patches one direction: `until_pending_rows_are_free`
waits for the freeze and the sync trackers to go idle before the run takes
the keys. Its own doc comment names what it does not cover — a sync armed
between the wait returning and the run taking the rows, a freeze armed while
a deletion holds them, and a sync and a freeze meeting each other at all.
Replace that patch with one order for all three.

**1. One queue on the server.** Give `ServerState` a single turn the three
flows take in the order they were armed — a fair `tokio::sync::Mutex<()>`
(FIFO under contention) or an equivalent of your choice, held by each run
across its whole use-case call. Take the turn *before* `state.unlocked()`,
as `delete/run.rs` takes its wait, so that time spent waiting is not
counted as somebody being here (spec: DK-4), and let a run armed after a
lock still stop on the locked refusal once its turn comes. Remove
`until_pending_rows_are_free` (its doc comment's two holes are what this
closes; a grep gate in `check_command` pins its removal). The use case keeps
refusing a second owner — that is the cross-process guard OC-2 asks for and
this task does not touch `coffret-usecase`'s ownership contract — the server
simply stops being that second owner. The explorer's "backup", "packing"
and "deletion" lines keep the same status and run-number semantics while a
run waits: it is `syncing` / `freezing` / `deleting` with `step: null`, as
a deletion already is while it waits today.

**2. Say that a run is waiting, on every line.** `deletingLine` in
`frontend/packages/apps/web/src/deleteEntries.ts` already says `waiting for
the packing under way to finish` when `App.tsx` passes it what else is
running. Do the same for `syncLine` and `freezeLine` in `fill.ts`: a run
with `step === null` while another of the three flows is running says what
it waits for, with one helper deciding the word (`packing` / `backup` /
`deletion`) so the three lines cannot disagree. Extend the delete
confirmation (`DeleteConfirm.tsx`) with one sentence when a sync or a freeze
is running at the time it is shown — "It starts after the packing under way
finishes." — since today the person learns that only from the progress line
after confirming.

**3. A refusal that names the real cause for the case that remains.** When
the owner is another process — `coffret` on the command line syncing the same
Library while the desktop app serves it — the use case still answers
`PendingRowsBusy`, and that must not arrive as "the server could not answer".
In `from_sync`, `from_freeze` and `from_delete` map `*Error::Index(
IndexError::PendingRowsBusy { .. })` to a refusal of its own: a status that is
not 500, a message along the lines of "another run on this device owns its
pending work — a sync, a freeze or a deletion started elsewhere; try again
when it finishes", and a kind the explorer's retry button is offered from
(the existing `conflict` kind introduced for commits the Library moved
underneath is a reasonable fit; choose, and say why in the doc comment).
Bring the Index's own words up to date with the third flow: the `Display`
text in `coffret-usecase/src/index_error/display.rs` says "another import";
the doc comments on `IndexError::PendingRowsBusy` and `Index::own_pending_rows`
name sync and freeze only.

**Operation × state coverage.** For each of the three operations, the other
two can be idle, running (inside Storage), or queued behind a running one.
Every combination must end with every armed run completing in arming order
and none stopped on `PendingRowsBusy`. Route tests already cover "a second
deletion waits for the first" and "a deletion confirmed while a book is
packed waits for it" (`tests/routes/delete.rs`); add the missing pairs, using
the same `hold_storage` / `held_reads` pattern so no test sleeps:

- a drop that arms a sync while a freeze is packing (freeze → sync);
- a freeze armed while a sync is in Storage (sync → freeze);
- a sync and a freeze armed while a deletion is running (delete → sync,
  delete → freeze);
- a freeze armed while a deletion is itself waiting behind a sync
  (queued → the freeze runs after both, in arming order).

Out of scope: making the use case's `own_pending_rows` wait across
processes; stopping a run that is waiting its turn (the "cancel a drop" work
is a separate roadmap item); the retry button for a deletion refused as a
conflict.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A sync armed while a freeze is packing, a freeze armed while a sync is in Storage, and a sync and a freeze armed while a deletion is running each wait for the running flow and then complete, in arming order, with the work answer reporting every run as `done` and none as `stopped` (route tests)
- [x] A freeze armed while a deletion is waiting behind a sync runs after both (route test)
- [x] The wait is taken before the keys: a run armed before an idle lock, whose turn comes after it, stops on the locked refusal exactly as an un-queued run does (route test)
- [x] `until_pending_rows_are_free` no longer exists in `coffret-server/src/` (grep gate appended to `check_command`)
- [x] `syncLine` and `freezeLine` say what the run is waiting for while `step` is null and another of the three flows is running, through the same helper `deletingLine` uses, and the delete confirmation says it will start after the running flow finishes (frontend unit tests)
- [x] `IndexError::PendingRowsBusy` reaching the server through a sync, a freeze or a deletion is answered with a non-500 refusal whose message names another run on this device as the owner (`api_error/tests.rs`), and the Index's `Display` text and doc comments name all three flows
- [x] `make check` passes

### Before merge (verified outside the check command)

- [ ] Needs a person: under `make desktop-dev` on the development Library, dropping a file into a mapped folder while a dropped book is still packing shows the backup line as waiting for the packing, and both finish without a "could not back up" notice
