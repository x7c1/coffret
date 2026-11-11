---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness]
max_refine_rounds: 2
retries_remaining: 0
check_command: 'make check'
assignee: null
branch: task/1111-1815-bump-rustix-past-rustsec-2026-0486-retry-1
created_at: 2026-11-11T09:15:00Z
updated_at: 2026-11-11T09:50:00Z
---

# fix(deps): bump rustix to 1.1.6 or later for RUSTSEC-2026-0486

## Overview

cargo-deny now refuses the dependency tree: RUSTSEC-2026-0486 marks
`rustix` 1.1.4, the version `backend/Cargo.lock` holds, as unsound. Its
extended-attribute functions (`getxattr` and the like) mark unwritten bytes
as initialized when given a zero-length buffer. The fix is in 1.1.6.

Update `rustix` in `backend/Cargo.lock` to the newest 1.1.x at 1.1.6 or
later (`cargo update -p rustix`), without widening the update to other
crates beyond what that pulls in. Do not add an exemption for the advisory
to `backend/deny.toml`. If the update needs a code change, make it; say in
the PR whether coffret calls any of the affected functions.

Do not change `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `backend/Cargo.lock` holds `rustix` 1.1.6 or later and no 1.1.x
      below it, and `backend/deny.toml` has no entry for RUSTSEC-2026-0486.
- [x] `make check` passes.
