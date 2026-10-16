---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1016-1410-write-head-0-at-init-and-refuse-two-control-objects-of-one-name
created_at: 2026-10-16T05:04:00Z
updated_at: 2026-10-16T07:23:56Z
---

# fix(commit): write the Library's first head at init, and refuse a Library holding two control objects of one name

## Overview

Two gaps in the head chain's exclusion on a Storage that mints identifiers
(Google Drive), where names are not unique and a conditional create excludes
by identifier alone (CP-2, CP-3, FM-18).

**1. The race for generation 0.** Every commit consumes the slot its head
carries, so two writers starting from one head cannot both succeed it
(`two_writers_on_one_head_leave_one_successor`). The first record is the
exception: a Library that has committed nothing has no head to carry a slot,
so each writer of generation 0 reserves its own, and on a minting Storage both
creates succeed. `backend/crates/domain/coffret-usecase/tests/commit_conformance_minting.rs`
documents this gap and leaves the two first-record race cases out of the
minting run.

Close it by having `init` write the Library's first head itself: an empty
Journal record at generation 0 (no additions, no removals) carrying the slot
the first real commit will consume. From then on every commit — including the
first one a user makes — consumes a slot a head carries, and the race cannot
start. `join` does not write it; the Library it joins already has one.

- State the rule in the spec: CP-2 gains that a Library's first head is written
  when the Library is created, so no commit ever reserves a slot of its own.
  Adjust FM-13 / FM-15 text on generation 0 to match (an empty record is a
  valid `head-0`). Keep rule IDs stable; add sub-bullets rather than renumber.
- A Library created before this change may have no head yet (initialized,
  never committed). It must keep working. Decide how — for instance, the first
  commit on a head-less Library writes the empty generation-0 head first and
  then commits on top of it through its slot, so the race window shrinks to the
  empty head's own create — and say what remains of the race for such a
  Library in the spec sub-bullet and the PR body.
- Re-enable the two first-record race cases in the minting conformance run, and
  rewrite the module note to say what now excludes the race. If a case still
  cannot hold for a legacy head-less Library, keep the note precise about
  that one shape.
- Recovery (RV-*) and checkpoints (CK-*) must accept the empty head-0; add
  tests that a fresh Library recovers and lists nothing.

**2. Two control objects under one name.** On a minting Storage, two objects
can share a name (the exclusion was broken, or a legacy race above happened).
Today the reader warns and takes the later one. Instead, a listing that finds
two `head-*` or two `idx-*` objects under one name is a Library
inconsistency: the run refuses with a typed error, and sync / commit stop
rather than pick one. (`key-*` replicas are exempt: KL-14 makes repair an
unconditional write, so a duplicate replica is benign.) Add the rule to FM-12
as a sub-bullet. Find the current
"warn and take the later one" code (search the gateway and usecase listing of
control objects) and replace it; name the new refusal in the error types the
CLI's `--json` kinds and the server's answers already use, consistently with
neighbouring kinds.

**3. Concept text the same surface leaves stale.**

- Storage concept (`docs/concepts/storage/`): "One name identifies one object"
  and FM-18 do not hold on a Storage that mints identifiers. Say that two
  control objects share a name only when the exclusion was broken, and that a
  reader refuses such a Library rather than reading one of them (CP-3, FM-12).
- Journal concept (`docs/concepts/journal/README.md`) Mental Model: add one
  sentence on the shape of the slot a head carries — a pre-minted identifier
  where the Storage mints identifiers, a re-derived name where it names
  objects — and that a writer never reserves a slot of its own (CP-2, CP-3,
  CP-15). Mention that the first head is written when the Library is created.

Out of scope: pruning old heads (CK-4..CK-6), Keyring generation pruning.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `init` writes `head-0`, an empty Journal record whose `next_commit_slot`
      is the slot the next commit consumes; a test on the name-keyed store and
      one on `InMemoryMintingStore` show it.
- [x] The first commit after `init` consumes the slot `head-0` carries, and the
      two first-record race cases run in `commit_conformance_minting.rs` and
      pass; the module note no longer describes the gap as open.
- [x] A Library with no head (created before this change) can still be
      committed to; a test covers it.
- [x] Recovery and checkpoint reading accept an empty `head-0`; a test recovers
      a freshly initialized Library and lists no Entries.
- [x] A listing with two `head-*` or two `idx-*` objects of one name is
      refused with a typed error and the commit / sync stops; tests cover both
      on the minting store. Two `key-*` objects of one name stay accepted —
      KL-14 makes Keyring repair an unconditional write whose duplicate is
      benign — and a test and FM-12's sub-bullet say so.
- [x] CP-2 and FM-12 carry the new rules as sub-bullets, rule IDs unchanged
      (`make check` runs `spec-rule-ids` and `spec-citations`).

### Before merge (verified outside the check command)

- [x] On the agent's MinIO Library, a fresh `init` into a new prefix shows
      `head-0` in the bucket, and a following drop / sync commits `head-1`.
