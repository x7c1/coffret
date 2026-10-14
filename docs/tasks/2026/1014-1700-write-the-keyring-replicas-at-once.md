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
branch: task/1014-1700-write-the-keyring-replicas-at-once
created_at: 2026-10-14T07:07:46Z
updated_at: 2026-10-14T08:02:16Z
---

# perf(commit): write and read back the Keyring replicas at once

## Overview

A commit writes the candidate Keyring's replicas one after another and then
reads them back one after another (`coffret-usecase/src/commit/keyring.rs`,
`replicate` around line 348): on Drive, three replicas take about seven
seconds before the head is written. No rule orders the replicas. CP-8 asks
only that a complete candidate set (KL-2) is written and verified by read-back
before the Journal commit; completeness is a matter of which positions are
present, not of order (KL-2); a replica's name fixes its content, so writing
one unconditionally, or twice, is harmless (KL-14); an interrupted write
leaves a partial candidate whatever subset landed, and nothing selects it
(KL-3, KL-5, CP-9), with the clean-up as it is today (KL-12, OC-2..OC-5). The
head must still be written only after every replica has been verified
(CP-1, CP-8, CP-10).

Write the replicas concurrently and read them back concurrently:

- In `replicate`, issue the replica writes together (for example
  `futures_util::future::join_all`; add the `alloc` feature to the crate's
  `futures-util` dependency if it needs it, and keep `make deps` passing),
  advancing `committing.replicas_stored(n)` as each one lands. Collect the
  results in position order and fail on the first failed position, so the
  error reported stays deterministic.
- Read the replicas back concurrently; `IncompleteKeyring` reports the lowest
  failing position, as now.
- Do the same for the repair writes and the examine reads in the same file
  (around lines 120 and 171) where the same reasoning holds; leave anything
  whose order a rule does fix.
- Say in the doc of `replicate` why the order is free (the rules above) and
  why the head still waits for all of them.

Tests:

- `commit_conformance/progress.rs`
  `a_commit_is_seen_part_way_in_the_objects_it_stores` asserts each replica
  is sent with the earlier ones counted. Change it to what still holds: every
  replica is sent before the head, the head goes out at `Committing(3)`, and
  the steps advance monotonically.
- Add a test that the writes overlap: a store that delays each put (paused
  tokio time) or holds every put until all are in flight, showing three
  replicas take about one put's time.
- Add or keep tests that a failure of any one position leaves no head written
  and reports a deterministic position (the existing faulty-store cases).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Replica writes and read-backs in a commit overlap (test with a delaying
      or barrier store)
- [x] The head is written only after every replica is verified; a failure at
      any position writes no head and reports the lowest failing position
      (tests)
- [x] The progress conformance test asserts what still holds under concurrent
      writes
- [x] `make check` passes

### Before merge (verified outside the check command)

- [ ] On the agent's Library (Drive), freeze one 300-page book and compare the
      time from the first replica write to the head write in the server or
      CLI log with the earlier measurement (about 7 s for three replicas)
