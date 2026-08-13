# Master Key

## Definition

**Master Key** is the current root secret of a [Library](../library/). Every
other key in coffret is derived from it or wrapped under it. Holding the
current Master Key and access to [Storage](../storage/) is sufficient to
restore the entire Library; without it, the stored data is unreadable — to
the storage provider and to the user alike. Replacing the Master Key starts a
new **Master Key epoch**.

## Collocations

- unlock (the Master Key with the Passphrase)
- derive (keys from the Master Key)
- back up (the Master Key as a Recovery Code)
- import (the Master Key on a new device)
- rotate (the Master Key into a new epoch)
- activate (a prepared Master Key epoch)

## Domain Rules

- The Master Key is random; it is **not** derived from the
  [Passphrase](../passphrase/).
- The Master Key never leaves the user's devices except as a
  [Recovery Code](../recovery-code/); the storage provider receives wrapped
  Container Keys, but never an unwrapped key or a passphrase-derived verifier.
- Purpose-specific keys derived from the Master Key directly encrypt control
  Storage Objects such as Journal records, Keyrings, and Index Snapshots.
- Exactly one Master Key epoch is active for a Library. An epoch changes only
  on Master Key rotation; ordinary Journal and Keyring generations do not
  change it. `master_key_epoch` is therefore distinct from a control object's
  `generation`. It is an unsigned 64-bit integer, starts at 1, and increments
  by one on each rotation.
- Rotation is exclusive with uploads, `freeze`, `prune`, and another
  rotation. It first generates a new random Master Key and Recovery Code. The
  user must confirm that the new Recovery Code is backed up before coffret
  writes the new epoch to Storage.
- Coffret prepares a complete Keyring replica set containing every current
  Container Key re-wrapped under the new Master Key, then an Index Snapshot
  of that same current Library state. The snapshot binds its
  `master_key_epoch`, Journal generation, and the Keyring's `set_digest`.
  Coffret verifies the Keyring before activation and read-back verifies the
  snapshot afterward; Containers themselves remain byte-for-byte unchanged.
- Uploading the valid new-epoch Index Snapshot is the atomic **activation
  point**. Before it exists, the old epoch remains active and new-epoch
  objects are uncommitted orphans. Once it exists and references a complete
  new-epoch Keyring replica set, the new epoch is active and cleanup must
  resume after a crash. `commit` remains the term for a Journal batch; Master
  Key rotation uses `activate`.
- A device durably keeps enough pending rotation state to resolve an
  uncertain activation and switches its passphrase-protected local key file
  only after activation. Coffret then permanently deletes all old-epoch
  Keyrings, Journal records, and Index Snapshots. Rotation is not reported as
  complete while reachable old-epoch control objects remain.
- Other devices do not learn the new Master Key automatically. A device that
  holds only the old epoch must be enrolled again with the new Recovery Code;
  failure to open a newer control epoch must not be reported as Storage
  corruption.
- Losing every device copy **and** every Recovery Code makes the data
  permanently unrecoverable. This is accepted by design and must be made
  unmistakably clear to the user.

## Related Concepts

- [Passphrase](../passphrase/) — protects the Master Key on a device
- [Recovery Code](../recovery-code/) — carries the Master Key across devices
- [Container Key](../container/container-key/) — wrapped under the Master Key
- [Storage Object](../storage-object/) — control objects use keys derived from
  the Master Key
