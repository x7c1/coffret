# Passphrase

## Definition

**Passphrase** is the secret the user memorizes to protect the
[Master Key](../master-key/) at rest on a device. Entering the Passphrase
unlocks the Master Key for use.

## Collocations

- enter (the Passphrase to unlock the Master Key)
- change (the Passphrase)

## Domain Rules

- Changing the Passphrase re-protects only the stored Master Key; no stored
  data is re-encrypted.
- The Passphrase guards against device theft. The strength of the encryption
  on [Storage](../storage/) does not depend on it, because the Master Key is
  random.

## Related Concepts

- [Master Key](../master-key/) — what the Passphrase protects
