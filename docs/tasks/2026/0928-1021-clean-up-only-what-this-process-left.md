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
branch: task/0928-1021-clean-up-only-what-this-process-left
created_at: 2026-09-28T10:21:31Z
updated_at: 2026-09-28T10:35:27Z
---

# fix: clean up only what this process left, and never be tripped by what another left

## Overview

A piece of code that makes something temporary — a file next to the one it replaces, a guard that tidies up after a worker — has to clean up what it made on every path out, and must not act on something it did not make. Three places break that rule.

1. **A crashed process's temporary file blocks a later write.** Owner-only files are written to a temporary neighbour named `.<name>.<pid>-<seq>.tmp` and renamed over the target (`coffret-device/src/owner_only.rs`, `temporary_neighbour` around lines 139-150, `create` around lines 114-134; the same shape is duplicated in `google-drive-store/src/oauth/token_cache/store.rs`, around lines 63-81). The file is opened with `create_new`, so a neighbour a crashed run left behind under a pid the operating system has since reused makes the write fail, every time that pid comes round, until someone deletes it by hand. Add a few random bytes to the neighbour's name — both crates already depend on `getrandom` — so a new run never lands on an old name. Add a test that places a neighbour of the old shape first and asserts the write goes through.
2. **A failed write leaves its temporary file behind.** Both functions remove the neighbour only when the rename fails (`owner_only.rs`, around lines 50-63; `store.rs`, around lines 47-60). A failure in `write_all` or `sync_all` returns with the neighbour still on disk. Remove it on every failure after it was created, and test the write-failure path.
3. **A worker's exit guard can stop the run that replaced it.** The fill and freeze workers hold a `Leaving` guard whose `Drop` calls `abandon` (`coffret-server/src/fill/worker.rs`, around lines 8-31; `freeze/worker.rs`, around lines 14-30). Between `take_next()` returning `None` and setting `working = false` (`fill/progress.rs`, around lines 198-212) and the guard's drop, another thread's `arm()` or `queue()` can set `working = true` and spawn a new worker; the old guard's `abandon` (around lines 240-261) then takes the new run for the dead worker's leftovers, puts the armed folder in `discarded` and reports the run stopped. Make the guard tidy up only when the worker is unwinding from a panic (`std::thread::panicking()` in `Drop`); the workers are `tokio::spawn`ed and never aborted, so a panic is the only abnormal exit. Do fill and freeze together. Add a unit test that calls `take_next()` → `None`, then `arm()`, then the old worker's exit, and asserts the armed folder stays queued and not in `discarded`; the existing panic-path tests (for example `abandoning_a_fill_tells_whoever_is_waiting_on_it`) keep passing.

Also, in the same `token_cache` module: the type's doc (`google-drive-store/src/oauth/token_cache/mod.rs`, around lines 32-36) and `store()`'s doc (`token_cache/store.rs`, around lines 10-16) both explain that the cache is replaced by a rename so an interruption loses nothing. Keep the explanation in one place and point to it from the other.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Writing an owner-only file and the token cache succeeds when a temporary neighbour of the old `.<name>.<pid>-<seq>.tmp` shape is already present (`make check`)
- [x] A failure in writing or syncing the temporary neighbour leaves no neighbour behind, in both places (`make check`)
- [x] A fill worker's and a freeze worker's normal exit after a new arm leaves the armed folder queued and not discarded; the panic-path tests still pass (`make check`)
- [x] `make check` passes
