---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1019-2129-complete-a-committed-batch-before-reading-the-catalog
created_at: 2026-10-19T12:29:31Z
updated_at: 2026-10-19T13:23:03Z
---

# fix(device): complete a batch that committed before the server died, without waiting for a sync

## Overview

When a server dies after a batch's Journal record is committed but before the Index reflects it, the next start catches the catalog up with the Library's head, but the pending rows of that batch stay incomplete until a sync settles them (OC-7). Crash tests against a real Library (S3 / MinIO, release build of `main`) showed what that costs: after a restart, the 300 files the batch carried in, still on disk and unchanged, were listed as `remote` (not on this device), and `fill` refused every one of them with `ForeignFile: a file this device did not put there stands where this Entry belongs`. One sync fixed it (`completed: 1`), but nothing tells the person to run one, and the refusal says something untrue.

OC-7's completion — a pending row whose Container is current in a caught-up Index becomes a materialization record for its Entries, and its spool is discarded — needs nothing a sync has that the start-up catch-up lacks.

### What to change

- Run OC-7's completion right after every catch-up that leaves the Index caught up: at server start, after unlock, and after the catch-up that precedes a fill, freeze or listing (find where `run_catch_up` is called; reuse the settle code in `coffret-usecase/src/sync/settle/` rather than duplicating it). Only completion runs there: reclaiming rows (OC-3) stays with settle, because it needs the batch's commit slots checked.
- Completion must respect the pending-work ownership rule in OC-2 (a producer or settlement owns this device's pending work for its whole execution). If a producer holds the rows, skip and leave them to it.
- After completion, the listing reports those Entries `present` and `fill` opens them (the files are the device's own).
- State in OC-7 (and where LA-12 describes what a server does at start) that completion follows any catch-up, not only a sync's.

### Concept docs to bring in line (found while settling interrupted uploads)

The previous change taught settle to trash a Container by name when its pending row never recorded the object, and taught sync to remove stale scratch files. Three Concept Docs no longer match that vocabulary; fix them in this change, since this one touches pending rows and the Index concept anyway:

- `docs/concepts/library/README.md`, the scratch Domain Rule (around "A scratch whose rename never comes is the writer's own leftover"): say who removes it — a sync removes a scratch no live writer holds and leaves alone a name that merely carries the prefix (spec: OC-8, EP-11).
- `docs/concepts/library/README.md`, Collocations: the second **discard** item lists "a scratch" among what is discarded, while scratch files are *removed* (unlinked) and `LocalTrash::discard` means the desktop's trash. Take "a scratch" out of that item and add a **remove** collocation for a stale scratch removed during a sync. Decide whether `Placement::discard` (which removes a fetch scratch) should be renamed to a remove verb to match, and do it if so.
- `docs/concepts/index/README.md`, the pending row description and the table row "Dispose of its spool and uploaded object": a pending row records its uploaded object only after the upload answers, so a row without one is not proof nothing is on Storage; settlement looks the Container up by its name before disposing of the row (spec: OC-2). Add an Index Collocation for removing a pending row (once its spool is discarded and nothing of its Container can be on Storage), so `remove`, not `delete`, is the row's verb.

### Tests

- A batch committed and not yet reflected (simulate by committing and failing the Index refresh, as `commit/run.rs` already can): after a restart's catch-up, the rows are completed, the listing says `present`, fill succeeds, the spool is gone.
- A row whose Container is not current is not completed by the catch-up path and stays for settle.
- A catch-up while a producer owns the rows does not touch them.

### Out of scope

Reclaiming abandoned rows outside sync, persisting a drop's Pack choice.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] After any catch-up that leaves the Index caught up, pending rows whose Containers are current are completed (tests).
- [x] Listing and fill treat the batch's files as this device's right after a restart (tests).
- [x] OC-7 and LA-12 state when completion runs, and the spec checks pass.
- [x] The Library and Index Concept Docs describe scratch removal and the limit of a pending row's object reference in the vocabulary the code and spec use.

### Before merge

- [ ] Against the agent's development Library, kill the server after `committed the batch` is logged but before the Index refresh (lock the Index), restart, and confirm the listing shows the files `present` and a page opens, before any sync.
