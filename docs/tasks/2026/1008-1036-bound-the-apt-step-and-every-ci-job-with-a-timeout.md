---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make shell-lint && for f in .github/workflows/ci.yml .github/workflows/bundle.yml .github/workflows/toolchain-drift.yml; do awk "/name: Install system packages/{getline; if (\$0 !~ /timeout-minutes/) bad=1} END{exit bad}" $f || { echo "no step timeout in $f"; exit 1; }; done && [ "$(grep -c "^    timeout-minutes:" .github/workflows/ci.yml)" -ge 7 ]'
assignee: null
branch: task/1008-1036-bound-the-apt-step-and-every-ci-job-with-a-timeout
created_at: 2026-10-08T01:36:16Z
updated_at: 2026-10-08T01:43:21Z
---

# ci: bound the apt step and every job with a timeout, so a stalled mirror fails fast

## Overview

A `backend` run in `.github/workflows/ci.yml` stalled in `apt-get update`
(fetching the Ubuntu indexes) and sat there until GitHub's default job limit
of six hours cancelled it; nothing about the change under test had run. The
jobs in `ci.yml` set no `timeout-minutes`, and neither do the system-package
steps in `ci.yml`, `bundle.yml` and `toolchain-drift.yml`.

1. Give each step named `Install system packages` (and the equivalent apt step
   in `toolchain-drift.yml`, renaming it to that name if it is named
   differently) a `timeout-minutes` placed directly under its `name:` line,
   sized from what the step takes when it works (a cached run installs in
   about a minute; pick a bound of a few times the uncached duration, for
   example 10).
2. Give every job in `ci.yml` a job-level `timeout-minutes` sized from its
   recent successful durations with ample headroom (for example 30 for
   `backend`, which takes about 7–8 minutes; less for the short jobs), so no
   job can again hold a run for hours.
3. Keep a short comment where the step bound sits saying why it exists.

Look up recent durations with `gh run list` / `gh run view` if needed rather
than guessing. Nothing else in the workflows changes.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] Every apt `Install system packages` step in `ci.yml`, `bundle.yml` and `toolchain-drift.yml` has a `timeout-minutes` directly after its name
- [x] Every job in `ci.yml` has a job-level `timeout-minutes`
- [x] `make shell-lint` passes

### Before merge (verified outside the check command)
- [x] The PR's own CI run passes, which shows the bounds leave room for a normal run
