# Recovery

Rule prefix: `RV`. What a restore requires, how recovery bootstraps its keys,
and when recovery degrades to salvage.

Concept background: [Library](../../concepts/library/),
[Storage](../../concepts/storage/), [Index](../../concepts/index/),
[Storage Object](../../concepts/storage-object/).

## Rules

- **RV-1.** A restore reconstructs the current Container set from a valid
  Index Snapshot checkpoint followed by every later Journal record, or from
  the complete unpruned Journal history. For any pruned history, the Snapshot
  that covers it is the required baseline until a newer valid checkpoint
  supersedes it. *(→ tests)*
- **RV-2.** A restore additionally requires at least one committed valid
  Keyring replica matching the tuple the checkpoint records (KL-3, CK-3) and
  the current Containers themselves; a degraded replica set permits the
  restore, with repair gated by KL-11. *(→ tests)*
- **RV-3.** Recovery bootstrap is acyclic: purpose-specific keys derived from
  the Master Key directly open the control objects — Index Snapshots, Journal
  records, Keyrings — and the Keyring's envelopes then open the Containers.
  *(→ tests)*
  - Control-object keys are domain-separated by purpose: a key derived for a
    Journal record is never used for a Keyring or an Index Snapshot.
- **RV-4.** If the required checkpoint or Journal history is incomplete,
  recovery becomes salvage: coffret may present contents from decryptable
  Containers but cannot distinguish current Containers from removed,
  replaced, or uncommitted candidates. Salvage performs no automatic cleanup,
  never authorizes deletion or mutation, and is not a restore. *(→ tests)*
- **RV-5.** An exact Index rebuild follows RV-1 and then opens the resulting
  current Containers; without the required control state, opening every
  decryptable Container yields recoverable content candidates, not an
  accurate Index. *(→ tests)*
- **RV-6.** Authenticating Storage Objects proves their integrity, not their
  freshness: Storage can replay a coherent earlier Library state by
  withholding newer objects, so recent additions may disappear and removed
  entries may reappear. Detecting that rollback is not a coffret requirement;
  preventing it would need a trusted checkpoint outside Storage and may be
  added as a separate feature if the threat model changes. *(prose-only: an
  accepted limit against an adversarial Storage counterparty — a
  non-requirement cannot be executed as a test)*
