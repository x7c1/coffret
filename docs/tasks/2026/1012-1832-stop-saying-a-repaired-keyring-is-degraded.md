---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1012-1832-stop-saying-a-repaired-keyring-is-degraded
created_at: 2026-10-12T09:32:05Z
updated_at: 2026-10-12T09:54:32Z
---

# fix(fetch): stop a served Entry fetch from saying a repaired Keyring is degraded

## Overview

A server keeps the committed key table between Entry fetches
(`FetchMemory::key_table_for` / `remember_key_table` in
`backend/crates/domain/coffret-usecase/src/fetch/fetch_memory/mod.rs`, used by
`key_table` in `fetch/entry_run.rs`). It keeps the degraded-set finding of the
read that produced the table alongside it, and it looks both up by
`KeyringCommitment` alone. A repair (KL-13, KL-14) rewrites replicas under the
same commitment, so the commitment does not change. As a result, once the
first fetch after unlock has found the set degraded, every later served fetch
keeps reporting the set as degraded. That goes on after this process or another
device has repaired it, and after a refresh, until the Library locks or the
catalog names another commitment. The explorer then shows a health warning for
a condition that no longer holds.

The Library concept's findings table says an unresolved condition is "reported
again while a later run still observes the condition". KL-15 says a run reports
as loss only what a read established. A finding carried past a repair breaks
both.

What to change:

- A catch-up stops the kept finding from reaching later fetches. Catch-ups
  happen at startup, on `POST /api/refresh`, on reconnect, and on the one
  catch-up a served fetch makes for a stale catalog; each already updates the
  kept listing through `FetchMemory::remember_listing`. A catch-up is the point
  where this process asks Storage what the Library is now, so it is where a
  repair made elsewhere can be learned. The simplest form forgets the kept key
  table on every catch-up, so the next Entry fetch reads the Keyring again.
  Keeping the table and dropping only a degraded finding is also acceptable,
  as long as the next fetch then reads the Keyring to learn the set's current
  health.
- A write run in this process (sync, freeze, delete), which is where repair
  happens on this device, forgets the kept key table once it has committed or
  repaired.
- A kept key table without a degraded finding may still be reused across a
  catch-up that left the commitment unchanged. That keeps the round-trip saving
  in the healthy case, which is the case that matters for reading.

Update the doc comments on `FetchMemory`, `key_table` in `entry_run.rs`, and the
KL-15 fill test comment in `backend/crates/apps/coffret-server/tests/routes/fill.rs`
so they say when a kept finding stops being reported. Do not change any spec rule.
The spec wording for this is being written in a separate change.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test: a served fetch finds the set degraded. The set is then repaired on Storage and the server refreshes (`POST /api/refresh`). The next served fetch no longer reports the set as degraded, and it read the Keyring again to learn that
- [x] A test: in a server whose kept key table carries a degraded finding, a sync in the same process that repairs the set leaves the next served fetch without the finding
- [x] A test: with a healthy set, a refresh that does not advance the head leaves the next served fetch reading no Keyring. The saving from the kept key table holds in the healthy case
- [x] The existing KL-15 fill test, where one fill's fetches report a lost replica once, still passes
