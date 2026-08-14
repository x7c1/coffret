# Checkpoint and Prune

Rule prefix: `CK`. What an Index Snapshot checkpoint records, which Journal
records become eligible for `prune`, and the gate that must pass before they
are deleted.

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
  Containers, Library entries, or Library files. Its purpose is to bound
  retained Journal history and recovery replay. *(Form: test)*
  - `prune` is the formal operation name in documentation and code.
