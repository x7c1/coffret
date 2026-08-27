---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && git grep -qF -- '-include local.mk' -- Makefile && git grep -qE '^local\\.mk$' -- .gitignore && ! git ls-files | grep -qxF local.mk"
assignee: null
branch: task/0827-0838-local-makefile-overrides
created_at: 2026-08-27T08:38:13Z
updated_at: 2026-08-27T09:14:00Z
---

# build: allow local Makefile overrides via local.mk

## Overview

The `Makefile` is the repo's single entry point, and it currently offers no
per-machine escape hatch: a developer whose host toolchain needs an override —
for example a `PATH` where `cc` resolves to a compiler that cannot build the
workspace's C dependencies, needing `CC=/usr/bin/cc` exported for every
`make check` — must either edit the tracked `Makefile` or remember to prefix
every invocation with environment variables. Both are error-prone, and the
first risks committing a machine-specific change.

Add the conventional optional include:

- In `Makefile`, directly after the `.DEFAULT_GOAL := help` line (so any
  variable assignments in the override file are in effect before the
  parameter defaults and every recipe, and cannot change the default goal
  by accident), add:

  ```make
  # Optional per-machine overrides (gitignored): toolchain pins like
  # `export CC := /usr/bin/cc`, parameter defaults, extra targets. Absent on
  # a fresh clone; `-include` skips it silently.
  -include local.mk
  ```

  The comment stays with the include so a reader learns what the file is for
  without leaving the Makefile.
- In `.gitignore`, add `local.mk` under the existing `# Local env` group
  (beside `.env` / `.env.local`), so the override file can never be committed.

No target's behavior changes when `local.mk` is absent: `-include` (unlike
`include`) neither warns nor fails on a missing file, and `.DEFAULT_GOAL` is
already set before the include.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `Makefile` contains the `-include local.mk` line (the check command
      greps for it), placed after `.DEFAULT_GOAL := help` and before the
      first parameter assignment, with the explanatory comment beside it.
- [x] `.gitignore` contains a `local.mk` line (the check command greps for
      the exact line), and no file named `local.mk` is tracked
      (`! git ls-files | grep -qxF local.mk` is part of the check command).
- [x] `make check` still passes with no `local.mk` present — the include is
      silently skipped and every existing target behaves as before.

## Out of scope

- **Committing any default `local.mk` or a `local.mk.example`.** The file is
  strictly per-machine; documenting its purpose is the Makefile comment's job.
- **Changing any existing target, variable default, or the frontend/backend
  command lines.** This task only adds the include seam and the ignore rule.
