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
branch: task/1019-1200-clean-up-what-an-interrupted-upload-leaves
created_at: 2026-10-19T11:10:00Z
updated_at: 2026-10-19T12:06:52Z
---

# fix(settle): clean up what an interrupted upload leaves in Storage and in mapped folders

## Overview

A server killed in the middle of an import must leave nothing that cleanup cannot account for (spec: OC-2: "an interruption at any point leaves nothing whose creating batch cleanup cannot identify"). Crash tests against a real Library (S3 / MinIO, release build of `main` at `144409d3`) found two places where it does not.

### 1. A Container put but not yet recorded becomes an orphan

`coffret-usecase/src/upload/run.rs` puts a Container and only then records the Storage object reference on its pending row. A process that dies between the two leaves a row in `spooled` state with `object_ref` NULL while the object exists in Storage. The next settle (`coffret-usecase/src/sync/settle/dispose.rs`) reads `object_ref() == None` as "never left the device", discards the spool, deletes the row, and never trashes the object. In the test, a 125 MB Container stayed in Storage named by no record and no finding.

Fix: a row without an object reference may still have a Container in Storage. The object name a put uses is derivable from the Container ID (check how each Storage gateway names objects: S3 by key, Drive by name under the Library folder). Before disposing such a row, settle trashes the object by that name idempotently: an object that is not there counts as success, the way OC-6 / OC-8 removals do. Only if the absence or the trash is confirmed is the row deleted; a refused or uncertain trash retains the row, as the `Some(object)` branch already does. Keep the `never left the device` path only where it is provably true (e.g. the row never reached the upload step). State the rule in OC-2 / OC-3 (or the settle rules they point at) so the spec says that a missing object reference is not proof the object is absent.

### 2. A receive scratch file stays in the mapped folder forever

`coffret-device/src/add/incoming_file.rs` removes its `.coffret-fetch-incoming-*.part` scratch file only in `Drop`. A process killed while a browser upload is still being received leaves the `.part` in the mapped folder. The scan skips it (`is_scratch` in `local_scan/walk_mappings.rs`), so it is never listed, never uploaded, and never removed, however many syncs run. Fetch scratch files have the same lifetime assumption.

Fix: a sync's walk of a mapped root (and of the scratch locations it already knows) removes stale coffret scratch files it meets: files under coffret's own scratch prefix that no live writer in this process holds. Removing them is OC-8's business (what this device wrote for its own purposes); state it there and in EP-11 where the scratch prefix is defined. Never remove anything outside the scratch prefix. A scratch file currently being written by this process (a concurrent upload or fetch) must not be removed: track live scratch files in the process, or only remove those older than the process start, and say which in the spec.

### Tests

- A row in `spooled` state with no object reference whose Container exists in Storage: settle trashes the object and deletes the row (both Index implementations, both Storage fakes where the settle suite runs).
- The same row whose Container does not exist: settle deletes the row, with no error.
- A trash that is refused retains the row, and a later settle retries.
- A stale `.part` under a mapped root is removed by the next sync; a `.part` being written concurrently by a live upload is not; a file that merely resembles the prefix but is not coffret's is not.
- Existing settle and scan tests keep passing.

### Out of scope

Persisting a drop's Pack choice, completing a committed batch outside sync, signal handling.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Settle never deletes a pending row whose Container may be in Storage without first trashing it by name or confirming it is absent (tests).
- [x] A refused or uncertain trash retains the row (tests).
- [x] A sync removes stale coffret scratch files in mapped roots and leaves live ones and non-scratch files alone (tests).
- [x] OC-2 / OC-3 / OC-8 and EP-11 state the new rules, and the spec checks pass.

### Before merge

- [ ] Against the agent's development Library, kill the server between the put and the row update (lock the Index with `BEGIN IMMEDIATE` during the put, then SIGKILL), restart, sync, and confirm the object is trashed and no unrecorded object remains.
- [ ] Kill the server during a throttled multipart upload, restart, sync, and confirm the `.part` is gone.
