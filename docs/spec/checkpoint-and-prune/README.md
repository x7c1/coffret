# Checkpoint and Prune

Rule prefix: `CK`. What an Index Snapshot checkpoint records, which Journal
records become eligible for `prune`, the gate that must pass before they are
deleted, what a Snapshot carries beyond the checkpoint, when and where one is
uploaded, and how a device brings a stale Index up to the head.

Concept background: [Index Snapshot](../../concepts/index-snapshot/),
[Journal](../../concepts/journal/).

## Rules

- **CK-1.** An Index Snapshot records both the control-head generation it
  represents and the last Journal generation it applies; recovery starts from
  the head generation and replays the later Journal successors. *(Form: test)*
- **CK-2.** An ordinary Index Snapshot preserves the next commit slot from
  the Journal record it reflects; once that record is pruned, the Snapshot
  remains the source of the slot. *(Form: test)*
- **CK-3.** An Index Snapshot belongs to one Master Key epoch and records the
  exact committed Keyring tuple it depends on: `master_key_epoch`,
  generation, replica count, and `set_digest` (KL-3). *(Form: test)*
- **CK-4.** Journal records at or before the Snapshot's last applied Journal
  generation become eligible for `prune`. *(Form: test)*
- **CK-5.** `prune` may run only when the Snapshot preserves the exact
  committed Keyring tuple and that Keyring replica set is complete (KL-2) —
  otherwise deleting the records could destroy the only evidence or envelopes
  a recovery still needs. *(Form: test)*
- **CK-6.** `prune` deletes only eligible Journal records; it never deletes
  Containers or the Entries inside them. Its purpose is to bound
  retained Journal history and recovery replay. *(Form: test)*
  - `prune` is the formal operation name in documentation and code.
- **CK-7.** An Index Snapshot carries the Index of the whole Library — every
  current Entry and its Container, including Entries under subtrees the
  uploading device does not map (EP-9) — and carries no device state: no
  local root mappings, local paths, which Entries the device has materialized
  (EP-10), spool locations, or upload progress. Two
  devices that map different parts of one Library restore identical Indexes
  from the same Snapshot. *(Form: test)*
- **CK-8.** After each successful Journal commit, the committing device
  uploads the new head's Index Snapshot (CK-10) before it reports the batch
  complete. A failed Snapshot upload leaves the commit valid — the
  records it would have covered remain replayable — and is retried on the
  next run. *(Form: test)*
- **CK-9.** A device brings a stale Index up to the head from the newest
  valid checkpoint whose head generation is at or after its own — an ordinary
  Index Snapshot under an `idx-` name, or an activation Index Snapshot under a
  `head-` name, which are equally checkpoint candidates (FM-12) — not from
  its own Index: it adopts that Snapshot's Library-wide content, keeps its own
  device state (CK-7), and replays only the Journal records committed after
  the Snapshot, opening a Container's meta section only for the additions
  those records list. The Containers a device must open are thereby bounded
  by the commits since the newest Snapshot, not since the device's own last
  sync. *(Form: test)*
  - Adopting a Snapshot another device wrote is safe for the same reason
    restoring from one is: it is authenticated under a purpose key derived
    from the Master Key (RV-3), and its checkpoint names the committed
    Keyring tuple it depends on (CK-3).
- **CK-10.** Each Journal record carries a `snapshot_slot`, reserved by its
  writer before the commit in the same form as a commit slot (CP-2): the one
  place where the ordinary Index Snapshot representing that head is created,
  by conditional create against it (CP-3). The Snapshot carries that head's
  generation and is named `idx-<generation>` for it (CP-15, FM-12, FM-13). An
  activation Snapshot is already the full checkpoint of the head it is, so no
  ordinary Snapshot is written for it. *(Form: test)*
  - The reason is that the second object would be a multi-megabyte duplicate
    of a checkpoint the Library already holds, not that its name would
    collide: an activation Snapshot is named for its place in the head chain
    and an ordinary one for the head it checkpoints, so the two names never
    meet (FM-12).
- **CK-11.** Losing that conditional create is not a failure. The loser
  reads the slot back: a valid Index Snapshot there that represents the same
  head (CK-1, CK-3) means the checkpoint exists and the loser's own upload
  is done — two Snapshots of one head would be the same checkpoint. Anything
  else at the slot is reported as Storage corruption and is neither
  overwritten nor written under another name, because a second name for one
  head would leave readers two checkpoints to choose between. *(Form: test)*
  - A slot holding nothing is not "anything else": the refusal settled
    nothing (CP-3), so the upload is attempted again rather than reported.
