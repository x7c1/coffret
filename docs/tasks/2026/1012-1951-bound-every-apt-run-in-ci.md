---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "install --with-deps" scripts/e2e-it.sh && grep -q "install-deps chromium" .github/workflows/ci.yml && [ "$(grep -l "Acquire::http::Timeout" .github/workflows/ci.yml .github/workflows/bundle.yml .github/workflows/toolchain-drift.yml | wc -l)" -eq 3 ]'
assignee: null
branch: task/1012-1951-bound-every-apt-run-in-ci
created_at: 2026-10-12T10:51:21Z
updated_at: 2026-10-12T10:58:27Z
---

# ci: give every apt run a stall timeout and retries, and install Chromium's system packages in a bounded step of their own

## Overview

A stalled Ubuntu mirror can hang `apt-get` indefinitely. The workflows'
own apt steps have a step timeout (`timeout-minutes: 15` on "Install system
packages" in `ci.yml` and `bundle.yml`, and the same in `toolchain-drift.yml`).
A stall there still waits out those 15 minutes before it fails. The e2e job has
no bound at all. `scripts/e2e-it.sh` runs `playwright install --with-deps
chromium` under `CI`, so the browser's system packages are installed by apt
inside the "Journeys against MinIO" step. On 2026-10-12 one such run waited 18
minutes on `azure.archive.ubuntu.com` after `Get:5 ... noble-security
InRelease`, and the job was cancelled at its 20-minute limit. The pull request
it ran on (x7c1/coffret#274) changes only documentation. An earlier run of the
same job took 11 minutes, against about 3 normally.

What to change:

- **Every apt run in the workflows fails or retries quickly on a stall rather
  than hanging.** Before the first apt command in each job that runs one,
  configure apt with a per-connection timeout and a retry count, for example
  `Acquire::http::Timeout`, `Acquire::https::Timeout` and `Acquire::Retries`
  in a file under `/etc/apt/apt.conf.d/`, or the same keys as `-o` options on
  every apt command. A stalled fetch is then abandoned within tens of seconds
  and tried again, and the step timeout stays as the outer bound. Apply this
  in `ci.yml` (the backend job, and the e2e job below), `bundle.yml` and
  `toolchain-drift.yml`, the same way in each.
- **The e2e job installs Chromium's system packages in a step of its own**,
  placed before "Journeys against MinIO". The step has its own
  `timeout-minutes`, sized from what a working run takes plus headroom, and
  runs `playwright install-deps chromium` with the apt configuration above in
  effect. `scripts/e2e-it.sh` then installs only the browser (`playwright
  install chromium`) in CI as it already does locally, so the target no longer
  installs system packages anywhere. Update the script's comment and the
  workflow comments to say where the packages come from and why the step is
  separate.

Keep the existing step timeouts and comments ("A stalled mirror can hang
`apt-get update`; fail fast …"), adjusting their wording where the new
configuration changes what they claim.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/e2e-it.sh` no longer runs `playwright install --with-deps` (grep gate)
- [x] `ci.yml`'s e2e job runs `playwright install-deps chromium` in its own step (grep gate)
- [x] `ci.yml`, `bundle.yml` and `toolchain-drift.yml` each configure an apt connection timeout (grep gate)

### Before merge (verified outside the check command)

- [x] On this pull request's own CI run, the e2e job passes, the new system-package step finishes within its timeout, and the backend job's apt step still installs everything it did before
