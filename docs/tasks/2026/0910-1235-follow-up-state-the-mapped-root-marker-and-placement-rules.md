---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/mapped-root-marker
follow_up_of: docs/tasks/2026/0910-1206-state-the-mapped-root-marker-and-placement-rules.md
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "One temporary file inside a destination folder, open for writing" backend/crates/domain/coffret-usecase/src/scratch_file.rs && ! grep -q "One temporary file of ..InMemoryFs" backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_scratch_file.rs'
assignee: null
branch: task/0910-1235-follow-up-state-the-mapped-root-marker-and-placement-rules
created_at: 2026-09-10T12:35:03Z
updated_at: 2026-09-10T12:44:40Z
---

# docs: say scratch in the two doc comments that still say temporary file

## Overview

EP-11 now calls the file a fetch writes before the rename that publishes it a
*scratch*, and both types below are already named `ScratchFile`; only their
doc comments still say "temporary file". No behaviour changes; only comment
text moves.

- `backend/crates/domain/coffret-usecase/src/scratch_file.rs`, line 6:
  replace the doc-comment line with

  ```
  /// One scratch inside a destination folder, open for writing.
  ```

- `backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_scratch_file.rs`,
  line 13: replace the doc-comment line with

  ```
  /// One scratch of [`InMemoryFs`](super::InMemoryFs), open for writing.
  ```

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scratch_file.rs` no longer says temporary file:
      `! grep -q "One temporary file inside a destination folder, open for writing" backend/crates/domain/coffret-usecase/src/scratch_file.rs`.
- [x] `in_memory_scratch_file.rs` no longer says temporary file:
      `! grep -q "One temporary file of ..InMemoryFs" backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_scratch_file.rs`.
- [x] Existing backend, frontend, and interoperability checks continue to pass.
