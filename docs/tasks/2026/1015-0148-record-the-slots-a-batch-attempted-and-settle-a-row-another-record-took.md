---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1015-0148-record-the-slots-a-batch-attempted-and-settle-a-row-another-record-took
created_at: 2026-10-14T16:48:23Z
updated_at: 2026-10-14T17:30:57Z
---

# fix(sync): record the slots a batch attempted, and settle a row whose slots another record took

## Overview

A pending row whose batch was marked `commit_attempted` and whose Container
never became current is retained by `settle`
(`coffret-usecase/src/sync/settle.rs`) for ever. It keeps its row, its spool
file and its Storage object, and it reports as needing attention, with exit
status 2, on every later run. OC-3 allows a second proof that would settle
it: "an authenticated different writer's record occupies the attempted commit
slot". The row does not record which slot its batch attempted, so the proof
can never be made. `settle.rs` says as much, and the conformance case
`another_writers_head_does_not_prove_an_uncertain_attempt` pins the retention.

Record the slots, and make the proof. Spec changes (behaviour changes, agreed):

- **OC-2:** before a Journal create can reach Storage, the batch durably
  records the slot it is about to consume, in the same all-or-nothing write
  that marks its rows attempted. A batch that rebases (CP-4) records each slot
  it attempts, so it can hold several.
- **OC-3:** define "an authenticated different writer's record". It is an
  authenticated control object at the attempted slot that does not name the
  row's Container among its additions. A Container ID is never reused (CP-14),
  so such a record cannot be this batch, whichever device wrote it, including
  a later run of this one. A record at the slot that does name the Container
  means the batch committed there, and OC-7 completes the row. An activation
  Index Snapshot at the slot is a different successor and qualifies. The proof
  needs **every** slot the batch recorded. A slot that is empty, unreadable,
  or holds a record that is gone (pruned, or its epoch rotated away) is not
  proof, and the row stays retained. A row with no recorded slot (written
  before this change) stays retained.
- **CP-18:** the sub-bullet says rows refused on a rebase after losing the
  slot "are retained". Reword it to point at OC-3: they are settled once
  every slot they recorded is proven taken by another record.

Implementation notes (the code decides the details):

- **Where the slot comes from.** The slot a commit consumes is the one its
  head carries (`journal::carried_slot`, x7c1/coffret#292). On a Storage that
  mints identifiers it is the head's `next_commit_slot` identifier; on one that
  names objects, `head-<g+1>`. Record what is needed to read that object back
  later (generation, plus the identifier where there is one). The first record
  (generation 0) has no carried slot. Record it by name, and leave its proof as
  the spec allows (an empty or ambiguous slot is not proof). Say in the PR
  what that means on Drive.
- **Order in the commit loop.** `commit/run.rs` calls
  `pending_provenance::mark_attempt` before `journal::commit`, and the slot is
  decided inside `journal::commit`. Decide the slot first, then mark the rows
  and record the slot in one Index write (`Index::mark_commit_attempted` from
  x7c1/coffret#290 is the natural place to extend), then create.
- **Index.** Pending rows already carry a `batch` identity, so the slots can
  belong to the batch rather than to each row. Add schema layout 12 in
  `coffret-sqlite-index` with a migration from 7..=11. Existing attempted rows
  get no slots and stay retained, which is OC-2's rule for missing evidence.
  Implement the change in every Index, including the in-memory index and the
  test decorators that wrap `mark_commit_attempted`.
- **Settle.** `settle` gets what it needs to read and authenticate a control
  object at a slot: keys, and the listing or the `CaughtUp` the run already
  has. `sync/run.rs` currently discards the `CaughtUp`. Reuse the existing
  reading pieces (`control_object::read`, `decode_journal_record`, the
  listing's handles, `ObjectStore::object_at`) rather than writing a second
  reader. A proven row is disposed of exactly like a row that was never
  attempted (trash the Storage object, remove the spool file and the row,
  OC-6 idempotence). Apply this on every path that settles pending rows, not
  only `sync`.
- **Reporting.** The text and JSON answers already distinguish settlements.
  Give a row settled by this proof its own settlement kind if the existing
  ones would misdescribe it, and keep the redaction rules.

Do not repair rows that exist today. The development Library's legacy rows are
cleared by a separate one-off script.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] OC-2, OC-3 and the CP-18 sub-bullet say what the Overview says; the
      spec checks pass
- [x] Marking a batch attempted and recording its slot are one all-or-nothing
      Index write, implemented by every Index, with a test that a failure
      partway leaves neither
- [x] Schema layout 12 migrates layouts 7..=11; an attempted row from before
      the migration has no recorded slot and is retained (test)
- [x] Conformance cases, run against the existing stores and the minting
      store:
      - a row whose only recorded slot holds another writer's Journal record
        that does not name its Container is disposed of (trash, spool, row);
      - a row whose batch rebased and recorded two slots is disposed of only
        when both are proven, and retained while one is empty;
      - a record at the slot that names the row's Container completes it
        (OC-7);
      - an empty slot and a missing (pruned) record both retain the row.
- [x] `another_writers_head_does_not_prove_an_uncertain_attempt` is replaced
      by cases that match the new OC-3
- [x] Existing settle, commit, sync and freeze conformance tests pass
      unchanged in meaning
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] On the agent Library (Google Drive), a sync killed right after its rows
      are marked attempted leaves rows with a recorded slot; after the other
      device commits on that head, the next sync on the first device disposes
      of them (trash, spool, rows) and exits without needing attention
- [x] On the agent Library, the 45 rows retained before this change (no
      recorded slot) are still retained, and reported as before
