---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make shell-lint && grep -qE "^    timeout-minutes: [0-9]+" .github/workflows/toolchain-drift.yml'
assignee: null
branch: task/1009-0150-bound-the-toolchain-drift-job-with-a-timeout
created_at: 2026-10-08T16:50:45Z
updated_at: 2026-10-08T16:52:53Z
---

# ci: bound the toolchain-drift job with a timeout

## Overview

Every job in `.github/workflows/ci.yml` now has a job-level
`timeout-minutes`, so a stalled step fails within minutes instead of holding
a run for GitHub's six-hour default. The weekly `latest-stable` job in
`.github/workflows/toolchain-drift.yml` bounds only its apt step; its build,
test and clippy steps can still hang for six hours.

Give the `latest-stable` job a job-level `timeout-minutes` sized from its
recent successful runs (about 5–6 minutes; look them up with
`gh run list --workflow toolchain-drift.yml`) with ample headroom for an
uncached apt install and a slower stable toolchain, for example 45, and a
one-line comment saying why, in the style of the bound on `ci.yml`'s
`backend` job. Nothing else changes.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] The `latest-stable` job in `toolchain-drift.yml` has a job-level `timeout-minutes`
- [x] `make shell-lint` passes
