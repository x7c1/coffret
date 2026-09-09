---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0908-1915-confine-local-file-readers-to-mapped-roots.md
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -Fq ''This is how it finds'' backend/crates/apps/coffret-device/src/local_path.rs'
assignee: null
branch: task/0909-0722-follow-up-confine-local-file-readers-to-mapped-roots
created_at: 2026-09-09T07:22:23Z
updated_at: 2026-09-09T14:24:15Z
---

# docs: correct local file reader API guidance

## Overview

Correct the documentation of `OpenLibrary::local_path_of` in
`backend/crates/apps/coffret-device/src/local_path.rs`. Replace lines 13–17,
beginning `A shell that has fetched an Entry`, with:

```rust
    /// This joined path is for display and reporting. A shell asks this call
    /// rather than rederiving EP-9 from the mappings itself. Reading the file uses
    /// [`open_local_file`](Self::open_local_file), which keeps the mapped root and
    /// validated relative location separate during descriptor descent.
```

This is a documentation correction with no behavior change.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The obsolete reader guidance is absent, verified by
      `! grep -Fq 'This is how it finds' backend/crates/apps/coffret-device/src/local_path.rs`.
