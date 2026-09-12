---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0912-0446-stop-calling-the-folder-a-descent-stopped-at-a-component.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "its `stopped_at` is a local folder" backend/crates/apps/coffret-server/src/api_error/redact/mod.rs && grep -q "A folder the descent would not pass through" backend/crates/apps/coffret-device/src/add/added_at.rs'
assignee: null
branch: task/0912-0612-follow-up-stop-calling-the-folder-a-descent-stopped-at-a-component
created_at: 2026-09-12T06:09:19Z
updated_at: 2026-09-12T06:22:34Z
---

# docs(backend): call the folder a descent stopped at a folder in two more comments

## Overview

Two comments still call a local folder a *component*, the word this repository
reserves for a piece of an Entry Path. One of them also names a struct field
that no longer exists. No behaviour changes: both are comments.

1. **`backend/crates/apps/coffret-server/src/api_error/redact/mod.rs`, line 24.**
   The module doc explains why redaction can be reasoned about per variant and
   names the field an unmaterializable path carries, in backticks, as
   `component`. That field is `stopped_at`. Replace

   ```
   //! knowing that its `component` is a local folder is the same knowledge that
   ```

   with

   ```
   //! knowing that its `stopped_at` is a local folder is the same knowledge that
   ```

2. **`backend/crates/apps/coffret-device/src/add/added_at.rs`, line 59.**
   The comment introduces the two things a look can run into, and calls the
   first a component although what it describes is the folder a descent would
   not pass through. Replace

   ```
   // A component the descent would not pass through, and a disk that
   ```

   with

   ```
   // A folder the descent would not pass through, and a disk that
   ```

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] The redaction module doc names the field by its own name:
      `grep -q "its \`stopped_at\` is a local folder" backend/crates/apps/coffret-server/src/api_error/redact/mod.rs`.
- [x] The comment above the look calls the folder a folder:
      `grep -q "A folder the descent would not pass through" backend/crates/apps/coffret-device/src/add/added_at.rs`.
