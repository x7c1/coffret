---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1101-0210-let-one-drop-or-import-carry-up-to-65536-files
created_at: 2026-10-31T20:17:01Z
updated_at: 2026-10-31T21:26:50Z
---

# feat(server): let one drop or import carry up to 65,536 files

## Overview

The server keeps three budgets on one upload request (spec LA-9,
`backend/crates/apps/coffret-server/src/allowance.rs`): 64 GiB in all, 1 GiB
per part, and 4096 parts. The desktop import borrows the file count (LA-28).
The explorer sends one drop as one request, so a folder of more than 4096
files is refused whole, although adding several volumes at once easily goes
past it.

Raise the part / file count to 65,536 for both. Keep the 64 GiB and 1 GiB
budgets: they are what guards the loopback socket. Rewrite the reasoning in
LA-9, LA-28 and the comments of `allowance.rs` so they no longer rest on "one
drop is one book"; say instead that the count guards against a request of
countless tiny parts and that the byte budgets carry the rest.

A drop of tens of thousands of files must stay usable in the explorer: the
drop confirmation dialog and the numbering preview must not freeze or grow
without bound. Make them render a large list without drawing every row at
once (virtualise, or show a bounded part with a count of the rest), and keep
any per-file work in the drop path linear.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it`
and `make e2e-electron`, and in CI) must keep passing.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Server tests cover a request of 65,536 parts passing the count and
  65,537 refused, with the byte budgets unchanged, for upload and import.
- [x] LA-9, LA-28 and `allowance.rs` state 65,536 and the new reasoning, and
  `make check`'s spec checks pass.
- [x] A web test renders the drop confirmation and the numbering preview for
  10,000 files and checks the number of rows drawn stays bounded while the
  totals are right.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent drops a generated folder of more than 10,000
  small images, records in the PR how long the confirmation took to appear
  and to commit, and that the tree shows every file.
