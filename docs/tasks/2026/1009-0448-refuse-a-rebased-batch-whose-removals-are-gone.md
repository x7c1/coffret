---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1009-0448-refuse-a-rebased-batch-whose-removals-are-gone
created_at: 2026-10-08T19:48:31Z
updated_at: 2026-10-08T22:24:01Z
---

# fix(commit): refuse a rebased batch whose removals are no longer current

## Overview

A batch names Containers to remove (a one-file Container `sync` replaces, the
one-file Containers `freeze` absorbs, a Pack `delete` removes or rebuilds) and
Containers to add. When the head moved since the batch was prepared, the
commit rebases onto the new head (`coffret-usecase/src/commit/`, the candidate
check in `candidate.rs` and the rebase path). Today the rebase checks the
batch's additions against the new current state (Entry Paths, EP-5 / EP-6) but
does not require that every Container in the batch's removals is still
current. If another device removed one of them in between, the batch still
commits:

- `delete` rebuilding a Pack while another device deleted that whole Pack: the
  rebuilt Pack brings the kept Entries back into the Library after the other
  device deleted them;
- `sync` replacing a one-file Container another device already replaced or
  deleted: the replacement lands as if nothing happened;
- `freeze` absorbing one-file Containers another device removed: their
  content returns inside the new Pack.

Each is a later write silently undoing an earlier committed removal.

**1. The rule.** When a batch is committed against a head other than the one
it was prepared on, every Container in its removals must still be current in
the new state. If any is not, the commit is refused as a conflict, with the
Containers named; nothing is written to the head, and the prepared, uploaded
objects follow the existing cleanup rules for a batch whose commit did not
land (OC rules — check which applies: a refused-before-write commit is
`commit_attempted` false). State the rule in the commit protocol spec next to
the rebase rules (CP-*), and say what each caller does with the conflict.

**2. Callers.** `sync`, `freeze` and `delete` surface the conflict the way they
surface other conflicts today, so the next run re-plans from the new state
(the files are scanned again; a deleted Entry's local file is then new or
reported, per the existing rules). Do not retry the same batch automatically.

**3. Tests.** A conformance case per caller where another device removes a
Container between prepare and commit: the batch is refused, the head is
unchanged by it, and the removed Container stays removed. Plus the unchanged
case where removals are still current and the rebase commits as before.

Out of scope: changing what a run does after re-planning, and repack /
compaction.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] A rebased batch whose removals include a Container no longer current is refused as a conflict naming it, and the head does not change — conformance tests for `sync`, `freeze` and `delete`
- [x] A rebased batch whose removals are all still current commits as before (existing tests, plus one explicit case)
- [x] The refused batch's uploaded objects are handled by the existing cleanup rules (test)
- [x] The commit protocol spec states the rule, and `make spec-citations spec-rule-ids` passes
- [x] `make check` passes
