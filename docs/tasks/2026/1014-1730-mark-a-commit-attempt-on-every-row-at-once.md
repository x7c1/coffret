---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1014-1730-mark-a-commit-attempt-on-every-row-at-once
created_at: 2026-10-14T09:00:51Z
updated_at: 2026-10-14T09:12:22Z
---

# fix(commit): mark a commit attempt on every row of the batch at once

## Overview

Before a commit sends its Journal record, `mark_attempt`
(`coffret-usecase/src/commit/pending_provenance.rs`) marks each pending row of
the batch `commit_attempted = true`, so that a later settle can tell an
abandoned upload (no attempt: OC-3's first proof — nothing was ever sent for
the slot) from one whose outcome is unknown (kept). It writes the rows one at a
time (`index.record_pending_row(row)` in a loop), each its own write.

A process killed partway through that loop leaves the batch split: some rows
marked, some not. Measured on a development Library: a sync killed at the
moment the first row was marked left 45 of 300 rows marked and 255 unmarked;
the next run trashed the 255 and kept the 45 as "commit outcome unknown"
(needs attention), with their spool files and Storage objects, reported again
on every later run. Yet an unmarked row in the same batch proves the Journal
create was never sent — `mark_attempt` finishes before it — so the 45 were as
abandoned as the rest.

Make the mark atomic for the batch: add to the `Index` port a way to mark a
set of pending rows attempted in one transaction (or to record several rows in
one transaction), implement it in every Index implementation (the SQLite index
and any test or in-memory index), and have `mark_attempt` use it. Either every
row of the batch is marked or none is. Keep the existing per-row
`record_pending_row` for its other callers. Say in the doc of `mark_attempt`
why the mark must be all-or-nothing (OC-2, OC-3).

Rows already left split by the old code are not repaired by this change (the
rows carry no batch identity to group them by); say so in the PR, not in code.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The Index port can mark several pending rows attempted in one
      transaction, implemented by every Index; `mark_attempt` uses it
- [x] A test shows that a failure partway through marking leaves no row of the
      batch marked (for example a SQLite test where the transaction is made to
      fail after the first row, or an Index double that fails mid-way and
      checks nothing was applied)
- [x] The existing commit and settle tests pass unchanged in meaning
- [x] `make check` passes
