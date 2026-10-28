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
branch: task/1029-0505-prune-the-journal-records-an-index-snapshot-covers
created_at: 2026-10-28T20:05:40Z
updated_at: 2026-10-28T20:49:35Z
---

# feat: prune the Journal records an Index Snapshot covers

## Overview

Every commit adds one Journal record (`head-<n>`) to Storage and nothing ever
removes one, so a Library's Journal grows with every sync, freeze, rename and
delete it has ever made. The rules for removing them are already in the spec
(`docs/spec/checkpoint-and-prune/`, CK-4 to CK-6, and CK-8's "before `prune`"
moment), each marked unimplemented, and the concepts
(`docs/concepts/journal/`, `docs/concepts/index-snapshot/`) already describe
the `pruned` stage. Implement `prune` as those rules describe it, and give it a
CLI command.

### The operation

`prune` runs for one Library, against the Library's current head:

1. **Checkpoint first (CK-8).** Bring the device's Index up to the head
   (CK-9), then make sure an ordinary Index Snapshot of that head exists, in
   that head's `snapshot_slot` (CK-10, CK-11): write it if it is not there,
   accept the one that is if another device wrote it. This is the Snapshot
   `prune` rests on.
2. **Gate (CK-5).** Go on only when that Snapshot records the exact committed
   Keyring tuple (CK-3) and that tuple's replica set is complete on Storage
   (KL-2). Otherwise delete nothing and say why.
3. **Delete (CK-4, CK-6).** Remove the Journal records whose generation is at
   or before the Snapshot's last applied Journal generation — **except the
   record that is the current head**. The spec makes that record eligible too
   (CK-2 lets the Snapshot carry the next commit slot), but keeping it costs one
   object and spares every writer — including one on another device that read
   the head a moment ago and is about to consume its slot — from depending on
   a path no ordinary commit exercises today. Write this down as a sub-bullet
   under CK-6 ("eligible" does not oblige deletion; this implementation keeps
   the current head) rather than changing the rule.
   - Never delete an Index Snapshot of either kind, an activation Snapshot
     stored under a `head-` name, a Keyring replica, or a Container (CK-6). The
     Journal concept's invariant — a Library that has committed anything keeps
     at least one head or Snapshot — then holds by construction.
   - Remove with the store's recoverable `trash`, as the commit already does
     for the Containers a batch removes, not `purge`.
   - A record that is already gone (another device pruned concurrently) is
     not a failure. A record that cannot be removed is reported, and the
     records that were removed stay removed: prune is safe to run again.
4. Report what it did: the Snapshot it rested on, how many records it removed,
   and the generation range.

Put the operation in the usecase layer beside the other Library operations,
with tests against the in-memory store that pin each rule: CK-4 (which
generations are eligible), CK-5 (refusal when the Snapshot's Keyring tuple is
not the committed one, and when the replica set is incomplete), CK-6 (nothing
but Journal records is touched; the current head stays), CK-8 (the Snapshot is
written first when missing, and reused when present), and that after `prune`
a commit, a catch-up from an Index older than the Snapshot (CK-9), and a join
(which asks for any head or Snapshot, never `head-0`) all still work. Then
remove "unimplemented" from CK-4 to CK-6 and the "Snapshots written before
`prune`" clause from CK-8's form note, and make the spec's rule-to-test
binding (`make spec-rule-ids` and friends) agree.

### The command

`coffret prune` (taking the Library the way the other Library commands do,
`LibraryArgs`), honouring `--json` like the others. It is an explicit command
only: nothing runs it automatically after a commit or a checkpoint in this
change — whether and when to prune automatically is a policy choice left for
later, and this change records that in the command's help text or the spec
note, not as a TODO. The explorer gets no button.

### Out of scope

- Keyring generations that accumulate (a separate problem with no spec rule
  yet). `prune` must not delete any Keyring object.
- Old ordinary Index Snapshots superseded by a newer one: no rule says when
  they may go, so they stay.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Usecase tests pin CK-4, CK-5, CK-6 and the CK-8 "before `prune`"
      Snapshot as described above, including the refusals and the
      kept current head
- [x] Tests show that after `prune` a new commit succeeds, a device whose
      Index is older than the Snapshot catches up, and a join finds the
      Library
- [x] The spec no longer marks CK-4 to CK-6 unimplemented, and the
      spec-rule checks bound into `make check` pass with the new tests
- [x] `coffret prune --help` lists the command, and a CLI test covers its
      text and `--json` answers
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] On the local Library (`make local`, MinIO), after several commits
      (a rename or a drop in the explorer), `coffret prune` against the
      local state directory removes the older `head-*` objects from the
      bucket and keeps the current head, every `idx-*` and every `key-*`;
      the explorer still lists and opens books after a restart, and a
      further commit succeeds
- [x] Running `coffret prune` a second time right away removes nothing and
      succeeds
