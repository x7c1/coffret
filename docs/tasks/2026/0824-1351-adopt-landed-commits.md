---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && make s3-store-it"
assignee: null
branch: task/0824-1351-adopt-landed-commits
created_at: 2026-08-24T13:51:02Z
updated_at: 2026-08-24T16:30:00Z
---

# fix(backend): complete the bookkeeping of a landed commit whose Index refresh failed

## Overview

`commit_batch` fails the call when `Index::refresh` fails after the Journal
record has landed (`commit/run.rs`), and the failure is currently
unrecoverable in a way that breaks EP-10's promise. `refresh` is atomic, so
the failed device's Index knows neither half of the commit: not the
Library-wide record, and not the device-local facts — which files this
device materialized (`mark_present`) and which pending-upload rows stop
being pending. The Library-wide half heals on the next catch-up, but
nothing ever replays the device-local half. The result, reproducible with
today's code:

1. Run N syncs `a.jpg`; the record lands; `refresh` fails; the run errors.
   The pending row (with its `object_ref`) and the spool survive.
2. Run N+1 scans with the stale Index (`entry_at` = `None`), re-spools and
   re-uploads the file, and its commit's catch-up then refuses the batch as
   an EP-6 collision with the device's own now-current Entry — a wasted
   duplicate upload and a failed run.
3. Run N+2's Index is current (N+1's catch-up applied the record), so the
   scan sees `entry_at` = `Some` but `local_entry_at` = `None` and treats
   the path as never materialized (EP-10, `sync/scan/examine.rs`). Every
   later modification or deletion of `a.jpg` is **permanently invisible to
   sync** — and `reconcile`'s "an earlier run's commit landed after all"
   branch even clears the pending row without `mark_present`, destroying
   the provenance that could have healed it.

The fix: make the committer's device-local bookkeeping recoverable by
**completing** a landed commit's interrupted bookkeeping from its surviving
pending row, and settle pending rows at the **start** of a run — before the
scan reads local state — so a run never scans while its own commit's
visibility is unresolved.

Naming note: this settlement is called **(bookkeeping) completion**,
mirroring OC-6's "removal completion" (work a committed record implies,
finished later and idempotently). The verb "adopt" stays reserved for its
established meaning — the Index taking in an Index Snapshot's content
(CK-9) — and is not used for this concept.

### Design

**Completion.** A pending-upload row whose Container is current in the
Index is proof the batch's record landed (a replayed record is never
unlearned). For such a row, complete the interrupted refresh instead of
cleaning up:
`mark_present` an observation for each of that Container's current Entries
(the record's entry table carries `path` / `size` / `mtime`, exactly what
`sync/run.rs::materialized` fed `refresh`; `at` = now), discard the spool,
and `clear_pending_upload`. No new Index port method or schema is needed —
derive the entries from the existing queries (e.g. filter
`entries_under(None)` by Container ID; adoption only runs when pending rows
exist, so the full listing is not a hot path).

**Settle first, then scan.** Move `sync/reconcile.rs` from the end of
`sync_folders` to the start, before `scan`:

- If no pending row exists (the overwhelmingly common case), do nothing —
  no head read, no change to today's behavior.
- If any pending row was uploaded (`object_ref` is `Some`), first catch the
  Index up via the commit flow's `catch_up` (expose it `pub(crate)`; adjust
  the `commit/mod.rs` doc sentence that says every step is private because
  none is a stopping point — catch-up alone is exactly the read-only state
  a settling run stops at). Spool-only rows are decidable without a head
  read and must not trigger one.
- Then settle every row: Container current in the Index → **complete**
  (above);
  uploaded and not current → the Index has just read the head, so the
  existing OC-2/OC-3 disposal applies (trash the object, discard the spool,
  clear the row); never uploaded → discard the spool and clear the row as
  today.
- If the catch-up itself fails, fail the run. Scanning with unresolved
  pending rows is what produces the duplicate upload in step 2 above, so
  "carry on without settling" is not a safe fallback.

This removes the end-of-run reconcile and its `caught_up` parameter: a run
that starts with an uploaded pending row reads the head itself instead of
waiting for a run that commits. The existing conformance case
`an_uploaded_container_waits_for_a_run_that_reads_the_head` asserts the old
waiting behavior and must be rewritten to assert the new one (the first run
settles the row). With settling at the start, the post-refresh-failure
sequence converges in one run with no error and no duplicate upload: the
next run completes the row's bookkeeping, the scan sees `local_entry_at` =
`Some`, and a modified file becomes an ordinary replacement.

`commit_batch` itself keeps failing the call on a refresh failure — the
caller is still stale — but update its rustdoc paragraph that says the
device-local half "is not something to report and carry on from": it now
survives in the pending rows and is completed by the next run.

**Spec.** Add one rule to `docs/spec/orphan-cleanup/README.md` — OC-7, the
mirror of OC-2/OC-3: local provenance whose Container is current in a
caught-up Index is proof the batch **did** commit, and cleanup's mirror
action is to complete the creating device's interrupted bookkeeping
(the record of what it materialized, the spool, and the row are settled)
rather than to reclaim anything. Cite EP-10 (materialization is device
state) and CP-1 (nothing after the record can un-commit it). *(Form:
test)*. Do not renumber existing rules. Leave `docs/concepts/` untouched —
concept-doc registration for the sync flow is tracked separately.

### Tests (extend `sync_conformance`, house pattern)

- **The regression this task exists for**: a run whose record lands and
  whose `refresh` fails (inject with a fault-wrapping `Index` in
  `sync_conformance`, precedent: `mangling_store.rs` / commit's
  `faulty_store.rs`) errors; the file is then modified on disk; the next
  run with the healthy Index succeeds in one pass — it reports the
  completed Container, commits exactly one replacement (old Container
  removed), ends
  with no pending rows and no spools, and the decoded committed bytes equal
  the modified file.
- The same interrupted state with the file left unchanged: the next run
  completes the bookkeeping (`local_entry_at` becomes `Some`, state
  present) and commits nothing.
- The rewritten head-reading case: a run with nothing to upload but an
  uploaded-and-abandoned pending row reads the head at the start, trashes
  the object, and clears the row in that same run.
- Existing interruption cases (spool-only convergence, uploaded-but-
  uncommitted convergence, stale row) keep passing with settling moved to
  the start.
- Completion is reported in `SyncOutcome`, not silent — extend `Reconciled`
  (or a sibling) so a caller can tell a completion from a disposal.

### Conventions

`CLAUDE.md` is authoritative: English docs/comments/commit/PR, Conventional
Commits, no `PartialEq` on error types, tests match variants, run
`make check` (and `make s3-store-it` for the MinIO half). Logging per
`coffret-logging`'s crate doc: counts, Container IDs, object names,
generations — never Entry Paths, local paths, plaintext, or key material.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] After a commit whose `Index::refresh` failed, the next sync run with
      a healthy Index converges in one pass: the surviving pending row's
      bookkeeping is completed, a subsequent modification of the same file
      is committed as a replacement, and no pending row or spool file
      remains (conformance case runs under `make check` in-memory and
      `make s3-store-it` on MinIO).
- [x] Completion marks the materialized Entry present: after it,
      `local_entry_at` answers `Some` with the committed Entry's size and
      mtime, and an unchanged file commits nothing.
- [x] Pending rows are settled at the start of a run: a run with nothing
      to upload but an uploaded-and-abandoned row reads the head, trashes
      the object, and clears the row in that same run (the former
      `an_uploaded_container_waits_for_a_run_that_reads_the_head` case is
      rewritten to this behavior).
- [x] A run with no pending rows performs no head read at the start
      (asserted via the store fixture's request/listing count or an
      equivalent observable).
- [x] `docs/spec/orphan-cleanup/README.md` gains OC-7 codifying bookkeeping
      completion, with existing rule IDs unchanged.
- [x] Existing sync and commit conformance suites pass under `make check`
      and `make s3-store-it`; error types derive no `PartialEq`.

### Manual / on-hardware (verified by a human before merge)

- [x] The rewritten rustdoc on `commit_batch` and the reconcile/completion
      module reads as one coherent story with the new OC-7 rule (judgement
      about prose intent, not mechanically checkable).

## Out of scope

- The EP-10 edge where a device's committed Entry is replaced by another
  device before adoption runs (the row is then not current and is disposed
  of; the local file's materialization record stays absent). Rare
  double-failure; revisit with the download path.
- Concept-doc registration of `materialize` / `spool` / the sync flow
  (tracked as a separate follow-up).
- Orphan reclamation beyond what reconcile already does, `prune`, epoch
  activation, Pack flows, and the download path.
