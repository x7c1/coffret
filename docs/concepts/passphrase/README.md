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
- The Passphrase protects only the Master Key at rest: a thief who takes
  the device cannot extract the Master Key and reach the whole Library on
  [Storage](../storage/). It does not protect plaintext files, decrypted
  caches, or the Index on the device — that is the job of disk encryption.
- The strength of the encryption on Storage does not depend on the
  Passphrase, because the Master Key is random.

## Related Concepts

- [Master Key](../master-key/) — what the Passphrase protects
