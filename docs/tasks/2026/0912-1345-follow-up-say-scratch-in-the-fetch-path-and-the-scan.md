---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0912-0501-say-scratch-in-the-fetch-path-and-the-scan.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "prefix reserved" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs'
assignee: null
branch: task/0912-1345-follow-up-say-scratch-in-the-fetch-path-and-the-scan
created_at: 2026-09-12T13:53:10Z
updated_at: 2026-09-12T14:13:43Z
---
# docs(backend): call the reserved prefix a prefix, not a scratch

## Overview

`FetchError`'s doc for a refused reserved name calls `.coffret-fetch-…` "the
scratch a half-written file is called by". EP-11 defines a **scratch** as the
file a fetch writes before the rename that publishes it, and names the prefix
separately: "The reserved prefix is `.coffret-fetch-`". So the line applies the
register's word to the thing the register does not give it — the name rather
than the file — in the one doc a reader consults to learn why a name was
refused. **No behaviour changes**; the edit is doc-comment text.

1. `backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`, line 131 —
   replace

   ```
       /// `.coffret-fetch-…` is the scratch a half-written file is called by, which
   ```

   with

   ```
       /// `.coffret-fetch-…` is the prefix reserved for a fetch's scratches, which
   ```

   The replacement is one column shorter than the line it replaces, so the
   following line (`/// a scan steps over (spec: EP-11). …`) needs no re-flow
   and no other line in the paragraph moves.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] the doc calls `.coffret-fetch-…` a reserved prefix rather than a scratch:
  `grep -q "prefix reserved" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`
