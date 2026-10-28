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
branch: task/1029-0423-cancel-superseded-pull-request-ci-runs
created_at: 2026-10-28T19:23:33Z
updated_at: 2026-10-28T19:31:39Z
---

# ci: cancel a pull request's CI run when a newer push supersedes it

## Overview

`.github/workflows/ci.yml` has no `concurrency` setting, so every push to a
pull request starts a full CI run while the previous one keeps going. The
older run's verdict is about a commit nobody will merge, and it holds runners
that the newer run waits for.

Add a `concurrency` setting to the CI workflow so that:

- **On a pull request**, a new run cancels the run still in progress for the
  same pull request. Key the group on the pull request number, not the head
  ref, so two pull requests from branches of the same name (one from a fork)
  never cancel each other.
- **On a push to `main`**, nothing is cancelled or dropped. `release.yml`
  listens for CI's `completed` event on `main` and only tags a release when
  that run's conclusion is `success`; a cancelled run on the release commit
  would silently skip the release. Note that a concurrency group drops a
  *pending* run when a newer one queues even with `cancel-in-progress: false`,
  so `main` runs must not share a group with each other at all — give each
  push its own group (e.g. keyed on the commit SHA), not just
  `cancel-in-progress: false`.

The workflow-level `concurrency` block gets a comment saying why `main` is
treated differently, pointing at `release.yml`, in the style of the comments
already in `ci.yml`. No other workflow changes.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] On this pull request, pushing a second commit while the first commit's
      CI run is still in progress cancels that run, and the second run
      completes
- [x] The `concurrency` group for a `push` event to `main` is unique per
      commit, so neither a running nor a pending `main` run can be cancelled
      by a later push (verified by reading the expression against the
      `push` event's context)
