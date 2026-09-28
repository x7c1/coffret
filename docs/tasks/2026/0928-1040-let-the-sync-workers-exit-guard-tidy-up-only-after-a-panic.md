---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/0928-1040-let-the-sync-workers-exit-guard-tidy-up-only-after-a-panic
created_at: 2026-09-28T10:40:47Z
updated_at: 2026-09-28T10:47:26Z
---

# fix(server): let the sync worker's exit guard tidy up only after a panic

## Overview

The fill and freeze workers' exit guards now call `abandon` only when the worker is unwinding from a panic (`std::thread::panicking()` in `Drop`), because a guard that tidies up on a normal exit can stop a run that replaced it: between `take_next()` finding nothing armed and the guard's drop, another thread can arm a new run and spawn a new worker, and the old guard's `abandon` then takes that run for the dead worker's leftovers. The sync worker has the same guard and the same race (`coffret-server/src/sync/worker.rs`, `Leaving` and its `Drop`, calling `state.syncs.abandon()`); its comment even says the normal end "is the one ending that needs no putting back".

Make the sync worker's guard follow the fill and freeze workers' pattern — act only on a panic — and give it the same tests: a unit test that calls `take_next()` until it finds nothing armed, then arms a new sync, then drops the old worker's guard on the normal path and asserts the new sync stays armed and is not reported stopped; and a panic-path test that the guard still puts the running flag back and records the run as abandoned. Match the fill and freeze workers' structure and wording where the three can say the same thing (`coffret-server/src/fill/worker.rs`, `freeze/worker.rs`).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A sync worker's normal exit after a new sync was armed leaves that sync armed and not stopped, pinned in a unit test (`make check`)
- [x] A sync worker that panics still puts the running flag back and the run on record says it was abandoned, pinned in a unit test (`make check`)
- [x] `make check` passes
