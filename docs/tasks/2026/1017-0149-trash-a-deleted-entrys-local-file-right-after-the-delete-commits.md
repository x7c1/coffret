---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1017-0149-trash-a-deleted-entrys-local-file-right-after-the-delete-commits
created_at: 2026-10-16T16:49:12Z
updated_at: 2026-10-16T17:36:01Z
---

# feat(delete): move the deleting device's own unedited copies to the trash as soon as the delete commits

## Overview

When the explorer deletes Entries, the device that ran the delete keeps its mapped files where they are until its next sync (`OpenLibrary::delete` in `backend/crates/apps/coffret-device/src/run_delete.rs`; the docs of `delete/mod.rs`, `delete_request.rs` and `run_delete.rs` say "the next sync's to decide (spec: EP-15)"). Until then the explorer lists them as "not in Library", which reads as a failure of the delete a person just confirmed. Apply EP-15's decision right after the delete's commit, for this device's own files of the Entries `DeleteOutcome.deleted` names:

- An unedited file (EP-15's `unedited` rule in `domain/coffret-usecase/src/sync/scan/examine.rs`: same size and mtime as the device's `LocalEntry` row, or the same plaintext hash) moves to the OS trash through `LocalTrash::move_to_trash`, its `LocalEntry` row is forgotten, and folders the moves emptied are removed (`EmptiedFolders`), exactly as `sync/run.rs`'s `move_departed_to_trash` does.
- An edited file stays and is reported as kept, as a sync would.
- The same root checks as a sync apply (EP-12 / EP-13: the mapping's root marker must match `expected_root_id`); a root that does not check out leaves its files to the next sync and says so.
- Factor the shared pieces (`unedited`, `move_departed_to_trash`, the `SourceFile` built from a mapped path) out of `sync` into a module both runs use, rather than copying them. Give the delete run access to `MappedRoots` and `LocalTrash` (`self.local_fs` in `OpenLibrary`).
- Report it: `DeleteOutcome` (and the server's delete answer / the explorer's finished-run line) carries what was moved to the trash and what was kept edited, the way a sync's `Surfaced` items are reported. A trash failure does not fail the delete — the commit already happened — it is reported, and the next sync tries again.
- Other devices are unchanged: they still trash their copies on their next sync.
- Update the docs that say the delete leaves local files alone, and add to EP-15 (as a sub-bullet, rule ID unchanged) that the deleting device applies it right after its commit.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A delete conformance case: after the delete commits, the deleting device's unedited mapped file is in the trash, its `LocalEntry` row is gone, and an emptied folder is removed.
- [x] An edited file is kept and reported, a missing file is a no-op, and a root whose marker does not match is left for the next sync and reported (tests for each).
- [x] A trash failure leaves the delete committed and reports the failure (test with a refusing `LocalTrash`).
- [x] `sync` still trashes departed files as before (existing tests pass unchanged), using the shared module.
- [x] EP-15 carries the new sub-bullet, rule IDs unchanged.

### Before merge (verified outside the check command)

- [x] On the agent's MinIO Library, deleting a book from the explorer moves its local folder to the macOS Trash immediately, and the explorer no longer lists it as "not in Library".
