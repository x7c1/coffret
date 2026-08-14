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
  restore the current Library state; local state — the [Index](../index/),
  caches — remains expendable
  (spec: RV-1, RV-2).
- If required control state is missing, scanning Storage can salvage contents
  from decryptable Containers, but salvage cannot prove current membership
  and never authorizes automatic deletion or mutation
  (spec: RV-4).
- Authenticating Storage Objects proves their integrity, not their freshness:
  Storage can replay a coherent earlier Library state by withholding newer
  objects, and detecting that rollback is an accepted non-goal
  (spec: RV-6).

## Related Concepts

- [Storage Object](../storage-object/) — what Storage holds
- [Container](../container/) — a Storage Object holding user data
- [Index Snapshot](../index-snapshot/), [Journal](../journal/), and
  [Keyring](../keyring/) — the specially named objects on Storage
- [Library](../library/) — what Storage can restore
