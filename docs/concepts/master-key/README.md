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
  `generation`.
- Rotation re-wraps every current Container Key and refreshes the control
  objects under a new Master Key, while Containers remain byte-for-byte
  unchanged ([spec: MR-1, MR-2](../../spec/master-key-rotation/)).
- Rotation is complete only after every old-epoch control object reachable by
  coffret has been permanently deleted
  ([spec: MR-3](../../spec/master-key-rotation/)).
  - A copy retained by an attacker or the Storage provider before deletion
    remains readable with the old Master Key and cannot be invalidated by
    rotation.
- Losing every device copy **and** every Recovery Code makes the data
  permanently unrecoverable. This is accepted by design and must be made
  unmistakably clear to the user.

## Related Concepts

- [Passphrase](../passphrase/) — protects the Master Key on a device
- [Recovery Code](../recovery-code/) — carries the Master Key across devices
- [Container Key](../container/container-key/) — wrapped under the Master Key
- [Storage Object](../storage-object/) — control objects use keys derived from
  the Master Key
