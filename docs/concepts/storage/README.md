# Storage

## Definition

**Storage** is the remote object store that holds a [Library](../library/)'s
[Storage Objects](../storage-object/) — Google Drive first, other services
such as S3 later. Storage sits outside the user's trust boundary, so coffret
hands it only ciphertext. [Containers](../container/) have opaque names; the
recognizable names of control objects, and of the app folder they all live
in, are an explicit, limited exception needed for recovery.

## Examples

- A Google Drive folder containing a few thousand opaque encrypted objects

## Collocations

- upload (a Storage Object to Storage)
- fetch (a Storage Object from Storage)
- scan (Storage to rebuild the Index)
- salvage (decryptable Container contents when control state is incomplete)

## Domain Rules

- **Storage is the source of truth for committed Library state.** Together
  with the [Master Key](../master-key/), intact required control state
  reconstructs exactly which Containers are current. Their contents open
  only where the committed Keyring supplies reachable envelopes; a key-lost
  Container remains current but locked. Local state — the
  [Index](../index/), caches — remains expendable
  (spec: RV-1, RV-2, RV-7).
- If required control state (defined in
  [Storage Object](../storage-object/)) is missing, scanning Storage can
  salvage contents from decryptable Containers, but salvage cannot prove
  which Containers are current, and it never authorizes automatic deletion or
  mutation (spec: RV-4).
  - If the loss is the Keyring itself — every committed valid replica — the
    intact Journal and checkpoints still prove which Containers are current,
    but those Containers become unreadable; coffret enumerates and
    reports them, and after a rebuild carries them with explicit key-lost
    markers, present but locked (spec: RV-7, RV-8).
- One Library's objects live flat in one **app folder** of the Storage
  location, named after the **Library ID** — a Drive folder, or the matching
  key prefix on a store that keys objects by name. One name identifies one
  object within it, and coffret only ever creates the folder under, and works
  inside, the place the user configured: where it sits is the user's
  arrangement of their own Storage, and it is the folder's name that a device
  recovering with only a Recovery Code enumerates for (spec: FM-18).
- `object_ref` is Storage's own identifier for an object, the same value
  whichever device reads it, carried in control state as a cache so a device can
  fetch without listing Storage first. It is never evidence of membership,
  because a listing re-derives it and only the control state says what is current
  (spec: FM-15, FM-16).
- Authenticating Storage Objects proves their integrity, not their freshness:
  Storage can replay a coherent earlier Library state by withholding newer
  objects, and detecting that rollback is an accepted non-goal
  (spec: RV-6).
- Reaching Storage takes a credential the device keeps for the provider — for
  Google Drive, an OAuth refresh token in a token cache — and it is a bearer
  credential for the whole Library: whoever holds it can read and write every
  object coffret put there, though not open any of them, since Storage only
  ever sees ciphertext. The cache is therefore sealed under a
  [purpose key](../purpose-key/) of its own and never leaves the device
  (spec: KD-4, KD-10).

## Related Concepts

- [Storage Object](../storage-object/) — what Storage holds
- [Container](../container/) — a Storage Object holding user data
- [Index Snapshot](../index-snapshot/), [Journal](../journal/), and
  [Keyring](../keyring/) — the specially named objects on Storage
- [Library](../library/) — what Storage can restore
- [Purpose Key](../purpose-key/) — seals the credential a device keeps for
  the provider
- [Specification register](../../spec/) — the behavioral rules cited by ID
