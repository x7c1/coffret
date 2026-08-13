# Storage

## Definition

**Storage** is the remote object store that holds a [Library](../library/)'s
[Containers](../container/) — Google Drive first, other services such as S3
later. Coffret hands Storage only ciphertext under opaque names; the provider
can see that encrypted objects exist, but not what they are.

## Examples

- A Google Drive folder containing a few thousand opaque encrypted objects

## Collocations

- upload (a Container to Storage)
- fetch (a Container from Storage)
- scan (Storage to rebuild the Index)

## Domain Rules

- **Storage is the source of truth.** Together with the
  [Master Key](../master-key/), it contains everything needed to restore the
  Library. All local state — the [Index](../index/), caches — is expendable.

## Related Concepts

- [Container](../container/) — what Storage holds
- [Index Snapshot](../index-snapshot/), [Journal](../journal/), and
  [Keyring](../keyring/) — the specially named objects on Storage
- [Library](../library/) — what Storage can restore
