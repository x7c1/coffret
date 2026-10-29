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
branch: task/1029-1104-dispose-of-superseded-keyring-generations-in-prune
created_at: 2026-10-29T02:04:28Z
updated_at: 2026-10-29T03:20:00Z
---

# feat: dispose of superseded Keyring generations in prune

## Overview

Every commit writes a new Keyring generation — the key table of every current
Container — as three replicas, and nothing ever removes an older generation
within the same Master Key epoch. `key-*` objects grow by roughly
(replica count) × (commits), each one the size of the whole key table.
`coffret prune` (CK-4 to CK-6) removes the Journal records an Index Snapshot
covers but does not touch the Keyring.

Two changes, both to `prune` and its rules.

### 1. The current head is never eligible (CK-4)

CK-4 makes every Journal record at or before the Snapshot's last applied
generation eligible, which after an ordinary commit includes the current head.
The implementation already keeps the head, and CK-6 explains why in a note
("this implementation keeps the record that is the current head"). Make it the
rule instead of a choice: a device on another machine that read the head a
moment ago, and any implementation, must be able to rely on the head record
existing.

- CK-4: records at or before the Snapshot's last applied Journal generation
  **and older than the current head** become eligible.
- CK-6: turn the "Eligible does not oblige deletion, and this implementation
  keeps the record that is the current head" note into the rationale for
  CK-4's exclusion (the head record costs one object; without it every writer
  would depend on a head chain whose newest link is a Snapshot, which no
  ordinary commit exercises). Keep the other CK-6 notes.
- The existing test that pins "the head is kept" binds to CK-4.

### 2. `prune` disposes of superseded Keyring generations (new KL-18)

Add to `docs/spec/keyring-lifecycle/README.md`:

```markdown
- **KL-18.** A Keyring generation is **superseded** once a later generation
  of the same epoch is committed and complete (KL-2, KL-3). `prune` disposes
  of every replica of each superseded generation older than the generation
  its Snapshot records (CK-3, CK-5), after it has deleted the Journal records
  that Snapshot covers (CK-4, CK-6), and never of the generation the Snapshot
  records or the generation the current head selects. Disposal is a
  recoverable `trash`, the same removal `prune` applies to Journal records;
  only a Master Key rotation deletes Keyring replicas irreversibly (MR-3).
  *(Form: test)*
  - Nothing left on Storage after `prune` reaches such a generation: every
    current Container is mapped by the Snapshot's generation or a later one
    (KL-7), the Journal records that selected the older generations are gone,
    and an older Index Snapshot that records one can no longer be replayed to
    the head, so it is not a starting point a recovery could use (CK-9, RV-1).
  - A replica of a superseded generation that is missing is not a degraded
    set (KL-5) and is never repaired (KL-13).
  - A device that read an older head a moment ago has already read its key
    table; a generation disposed of under it does not stop the fetch it is
    making, and `trash` keeps the rest recoverable.
```

Amend KL-12 (append):

```markdown
  A superseded generation (KL-18) is not an orphan: it was committed, and
  `prune` disposes of it under KL-18 instead.
```

Amend KL-13 (append to the rule text):

```markdown
  Repair rewrites only the generation the device's head selects after it has
  caught up (CK-9); a device that finds replicas missing from an older
  generation catches up first and repairs nothing if that generation has been
  superseded (KL-18).
```

Wording may be tightened by refine, but the substance above was decided by
the maintainer and must not change: the disposal condition, `trash` (never
`purge`), happening as the last step of `prune`, and no retention count or
grace period.

Implementation:

- In `prune` (`backend/crates/domain/coffret-usecase/src/prune/`), after the
  Journal records are removed, list the `key-*` objects of the current epoch
  and `trash` every replica whose generation is older than the generation
  recorded by the Snapshot `prune` rested on. Never touch the Snapshot's
  generation, the head's generation, or a generation of another epoch.
- Only after the CK-5 gate passed (it already runs before removal). A replica
  that will not go is reported like an unremoved record — the run goes on, the
  outcome lists it, and the CLI exits with the same "partial" status it uses
  for unremoved records (2). Name the new outcome field and JSON key in the
  same style as `unremoved`.
- `PruneOutcome`, the CLI's text answer and `--json` answer report how many
  Keyring replicas were trashed.
- Repair (KL-13): make sure a device never rewrites replicas of a superseded
  generation. Find where repair decides which generation to rewrite; if it can
  ever target a generation other than the one its caught-up head selects, fix
  that and pin it with a test. If it already cannot, add the test that pins it.
- Find any spec text, doc comment or concept doc that says Keyring generations
  are retained, or that retention is a setting, and make it agree. The
  concept docs under `docs/concepts/` are in scope.

### Out of scope

- An automatic prune.
- `purge`, or emptying the provider's trash.
- The explorer.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] CK-4 excludes the current head and CK-6's note gives the reason; the
      head-kept test binds to CK-4
- [x] The spec has KL-18 and the KL-12 and KL-13 amendments, bound to tests by
      the spec-rule checks in `make check`
- [x] Usecase tests pin: superseded generations older than the Snapshot's are
      trashed after the records; the Snapshot's and the head's generations
      stay; another epoch's generations stay; nothing is trashed when the
      CK-5 gate refuses; a replica that will not go is reported and the run
      continues; a second prune trashes nothing; repair never rewrites a
      superseded generation
- [x] The CLI's text and `--json` answers report the trashed replicas, and a
      CLI test covers them
- [x] `make check` passes

### Before merge (verified outside the check command)

- [ ] On the local Library (`make local`, MinIO): make a few commits (e.g. a
      rename and back), run `coffret prune`; the `key-*` objects left are only
      the head's generation's replicas, the rest are in `trash/`; the explorer
      lists books and the reader opens a page after a restart; a further
      commit and a second prune succeed
