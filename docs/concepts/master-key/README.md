# Master Key

## Definition

**Master Key** is the single root secret of a [Library](../library/). Every
other key in coffret is derived from it or wrapped under it. Holding the
Master Key and access to [Storage](../storage/) is sufficient to restore the
entire Library; without the Master Key, the stored data is unreadable — to
the storage provider and to the user alike.

## Collocations

- unlock (the Master Key with the Passphrase)
- derive (keys from the Master Key)
- back up (the Master Key as a Recovery Code)
- import (the Master Key on a new device)

## Domain Rules

- The Master Key is random; it is **not** derived from the
  [Passphrase](../passphrase/).
- The Master Key never leaves the user's devices except as a
  [Recovery Code](../recovery-code/); the storage provider never receives any
  key material.
- Losing every device copy **and** every Recovery Code makes the data
  permanently unrecoverable. This is accepted by design and must be made
  unmistakably clear to the user.

## Related Concepts

- [Passphrase](../passphrase/) — protects the Master Key on a device
- [Recovery Code](../recovery-code/) — carries the Master Key across devices
- [Container Key](../container/container-key/) — wrapped under the Master Key
