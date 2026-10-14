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
branch: task/1015-0043-consume-the-commit-slot-the-head-carries
created_at: 2026-10-14T15:43:46Z
updated_at: 2026-10-14T16:18:14Z
---

# fix(commit): consume the commit slot the head carries, so two writers on Drive cannot both succeed one head

## Overview

CP-2 says each authenticated head determines exactly one next commit slot,
and on a Storage that mints identifiers the head carries it as a pre-minted
identifier in `next_commit_slot` (likewise CK-10's `snapshot_slot` for the
head's ordinary Index Snapshot). CP-3's exclusion — of the operations that
start from one head exactly one succeeds — rests on every writer targeting
that one identifier, which Google Drive refuses to create twice.

The commit path does not do this. `journal::commit`
(`coffret-usecase/src/commit/journal.rs`) gets its slot from
`reserve_commit`, which calls `ControlHead::reserve_commit_slot`
(`coffret-usecase/src/control_head.rs`), which calls
`store.reserve_create(successor_name)`. On Drive that mints a fresh identifier
on every call (`google-drive-store/src/google_drive.rs`, `generateIds`). No
usecase code reads the head record's `next_commit_slot` back. So two devices
committing on the same head each mint their own identifier, both
`put_if_absent` calls succeed, and Drive holds two `head-<g+1>` objects.
`ControlListing` keys handles by name (`BTreeMap<String, ObjectRef>`), so a
reader keeps one and silently drops the other; the writer of the dropped one
believes it committed, settles its pending rows and spool, and its additions
are never seen by the Library. The S3 and in-memory stores exclude by name, so
they are not affected; this is Drive only, and only when writers race on one
head.

Fix the implementation to match CP-2, CP-3 and CP-15 (the spec is right; do
not change its rules):

1. **Reproduce first.** Add a store double that behaves like Drive in what
   matters here — `reserve_create` mints a fresh identifier per call,
   `put_if_absent` excludes by identifier only, and the listing can return two
   objects under one name — and a commit conformance test where two writers
   commit concurrently on one head. Before the fix the test shows both
   succeeding; after it, exactly one commits and the other rebases (CP-3,
   CP-4) and both batches end up in the Journal. Run it against the existing
   stores too if the conformance suite is shaped that way.
2. **Consume the carried slot.** When a head exists, the commit slot is the
   one the authenticated head record carries: its `next_commit_slot` where the
   Storage mints identifiers, and the name `head-<generation + 1>` (re-derived,
   CP-15) where it does not. Do the same for the ordinary Index Snapshot's
   `snapshot_slot` (CK-10) wherever a checkpoint is written into a head's slot.
   Reserving fresh slots is still right for what the new record itself carries
   (its own successor's and snapshot's slots). Thread what catch-up already
   read of the head record to where the slot is consumed rather than reading
   the head again, unless the code shows a reason to.
3. **A head that carries no slot on a minting Storage** (a record that should
   have one and does not) is a malformed or foreign head: refuse the commit
   with a typed error rather than minting a fresh slot. Decide whether such
   heads can exist in Libraries written by the current code; say so in the PR.
4. **The first record** (generation 0, no head) has no head to carry a slot.
   Find what the spec and the code say about two devices creating a Library's
   first record at once on Drive, and either show it is excluded some other
   way (for example only one device ever creates a Library) or record the gap
   in the PR. Do not invent a rule for it here.
5. **Duplicate names in a listing.** Whether `ControlListing` should refuse or
   report a control name it sees twice, instead of keeping one silently, is
   worth deciding: after this fix it can still happen on a Library that raced
   under the old code. Make the listing at least log a warning naming the
   generation when a control object name appears twice; if refusing is clearly
   right per the spec (FM-12, RV-1), say so in the PR rather than changing the
   behaviour here.

Keep the existing `reserve_create` port; the change is in which slot a commit
consumes, not in how stores reserve. Update the doc comments in `journal.rs`
and `control_head.rs` that describe slot reservation so they say which slot is
consumed and why (CP-2, CP-3, CP-15).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A store double that mints identifiers per reservation and excludes by
      identifier exists, and a conformance test with two concurrent writers on
      one head shows exactly one successor of that head and both batches in
      the Journal (the test fails against the pre-fix commit path)
- [x] A commit on a head consumes the slot that head's record carries on a
      minting Storage, and the re-derived name on a name-keyed one; the same
      holds for the ordinary Index Snapshot slot
- [x] A head with no carried slot on a minting Storage is refused with a typed
      error, covered by a test
- [x] A control listing that sees one control name twice logs a warning naming
      it, covered by a test
- [x] Existing commit, catch-up, checkpoint and sync conformance tests pass
      unchanged in meaning
- [x] `make check` passes

### Before merge (verified outside the check command)

- [ ] On the agent Library (Google Drive, `coffret-agent`), a sync commits and
      the new `head-<g+1>` object on Drive has the identifier the previous
      head's `next_commit_slot` named
- [ ] On the agent Library, two devices (`coffret-agent` and
      `coffret-agent-2`) committing on one head leave exactly one
      `head-<g+1>`, and both batches are visible after a catch-up
- [ ] A read-only listing of the agent Library and the user's development
      Library (`coffret-dev`) shows whether any `head-*` name already appears
      twice; any found is reported in the PR, not repaired
