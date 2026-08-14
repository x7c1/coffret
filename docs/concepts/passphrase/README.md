# Passphrase

## Definition

**Passphrase** is the secret the user memorizes to protect the
[Master Key](../master-key/) at rest on a device. Entering the Passphrase
derives the key that decrypts the stored Master Key, unlocking it for use —
without this protection, anyone holding the device would hold the Library's
root secret.

## Collocations

- enter (the Passphrase to unlock the Master Key)
- change (the Passphrase)

## Domain Rules

- Changing the Passphrase re-protects only the stored Master Key; no stored
  data is re-encrypted.
- The Passphrase protects only the Master Key at rest: a thief who takes
  the device cannot extract the Master Key and reach the whole
  [Library](../library/) on [Storage](../storage/).
  - Plaintext files, decrypted caches, and the [Index](../index/) on the
    device are outside its protection — guarding those is the job of disk
    encryption.
- The strength of the encryption on Storage comes from the random Master Key
  alone, so a weak Passphrase weakens only the device-local protection,
  never the ciphertext on Storage.

## Related Concepts

- [Master Key](../master-key/) — what the Passphrase protects
- [Specification register](../../spec/) — the behavioral rules cited by ID
