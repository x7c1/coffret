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
branch: task/1007-1900-say-when-a-run-is-committing-and-how-far-the-commit-has-got
created_at: 2026-10-07T09:18:34Z
updated_at: 2026-10-07T09:34:57Z
---

# feat(explorer): say when a run is committing, and how far the commit has got

## Overview

A freeze of one book on Google Drive spends about as long after its Pack is
sent as sending it. In one run on a development Library, a 62 MB Pack was
stored about 12 seconds after the freeze began, and the batch was committed
about 17 seconds after that: three Keyring replicas written one after another
(about 2 to 5 seconds each) and then the head (about 7 seconds). Through all
of the commit the explorer still says "packing … — sending 1/1 — 62.0 MB of
62.0 MB…", which reads as a finished upload that has stalled.

**1. A phase of its own.** Add `Phase::Committing` to
`coffret-usecase/src/progress.rs`, in run order after `Uploading`: writing the
candidate Keyring's replicas, reading them back, and writing the head
(`commit/run.rs`, `commit_batch`, and what it calls). Count the objects the
commit writes as its units — each Keyring replica and the head — so the step
moves as each one is stored; if reading a replica back is a separate wait of
its own size, say how it is counted. Every flow that commits (sync, freeze,
the removals-only flow, prune if it reports progress) reports it the same way,
since the commit is shared. The doc comment on `Phase` says what the phase
covers.

**2. Through the work route to the explorer and the command line.** The
work answer (`step_dto.rs`, the contract and `work.json`) and the TypeScript
API (`work.ts`) carry the new phase. The explorer's `DOING` table in
`fill.ts` gives it a word a person can act on, for example
"committing to the Library 2/4". The command line's progress output gains a
line for it in its own style (`coffret-cli/src/progress.rs`).

Out of scope: making the commit faster (writing replicas in parallel is a
separate question that depends on the spec's ordering), and progress inside
one replica's write.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `Phase::Committing` exists, is documented, and every flow that commits reports it with a count of the objects written, with conformance or unit tests that a commit is seen part way
- [x] The work route, the contract, `work.json` and `work.ts` carry the phase, with route or DTO tests
- [x] The explorer and the command line each show a line for it, with tests for the wording
- [x] `make check` passes

### Before merge (verified outside the check command)
- [x] Needs a person: under `make desktop-dev`, dropping a book of tens of megabytes into a new folder of the development Library shows the megabytes rising while the Pack is sent, then a committing line that moves, then the packed notice
