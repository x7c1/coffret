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

## Domain Rules

- **Storage is the source of truth.** Together with the
  [Master Key](../master-key/), it contains everything needed to restore the
  Library. All local state — the [Index](../index/), caches — is expendable.
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
