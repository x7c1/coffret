# Storage

## Definition

**Storage** is the remote object store that holds a [Library](../library/)'s
[Storage Objects](../storage-object/) — Google Drive first, other services
such as S3 later. Coffret hands Storage only ciphertext.
[Containers](../container/) have opaque names; the recognizable names of
control objects are an explicit, limited exception needed for recovery.

## Examples

- A Google Drive folder containing a few thousand opaque encrypted objects

## Collocations

- upload (a Storage Object to Storage)
- fetch (a Storage Object from Storage)
- scan (Storage to rebuild the Index)
- salvage (decryptable Container contents when control state is incomplete)

## Domain Rules

- **Storage is the source of truth.** Together with the
  [Master Key](../master-key/), intact Storage contains everything needed to
  restore the current Library state. A restore requires a valid Index
  Snapshot checkpoint for any pruned Journal history, every later Journal
  record, at least one committed valid replica of the required Keyring, and
  the current Containers. Restore may proceed from a degraded Keyring replica
  set, but the set must be repaired to the configured replica count before any
  write, `prune`, or Master Key rotation. Local state — the
  [Index](../index/), caches — remains expendable.
- If required control state is missing, scanning Storage can salvage contents
  from decryptable Containers but cannot distinguish current Containers from
  removed, replaced, or uncommitted candidates. Salvage never authorizes
  automatic deletion or mutation and is not a restore.
- Authenticating Storage Objects proves their integrity, not their freshness.
  Storage can replay a coherent earlier Library state by withholding newer
  objects; detecting that rollback is not a coffret requirement. Recent
  additions may then disappear and removed entries may reappear. Preventing
  this would require a trusted checkpoint outside Storage and may be added as
  a separate feature if the threat model changes.

## Related Concepts

- [Storage Object](../storage-object/) — what Storage holds
- [Container](../container/) — a Storage Object holding user data
- [Index Snapshot](../index-snapshot/), [Journal](../journal/), and
  [Keyring](../keyring/) — the specially named objects on Storage
- [Library](../library/) — what Storage can restore
