# Passphrase

## Definition

**Passphrase** is the secret the user memorizes to protect the
[Master Key](../master-key/) stored on a device. Entering the Passphrase
derives the key that decrypts the stored Master Key, unlocking it for use —
without this protection, anyone holding the device would hold the Library's
root secret.

## Collocations

- enter (the Passphrase to unlock the Master Key)
- change (the Passphrase)

## Domain Rules

- Changing the Passphrase re-protects the stored Master Key and nothing else.
  The Master Key value is unchanged, so every
  [Storage Object](../storage-object/) stays exactly as it is — Containers
  and control objects alike, the [Keyring](../keyring/) included
  (spec: DK-6).
- The Passphrase protects only the stored Master Key: a thief who takes
  the device cannot extract the Master Key and reach the whole
  [Library](../library/) on [Storage](../storage/) (spec: DK-1, DK-2).
  - An unlocked Master Key sits in memory in the clear, beyond the
    Passphrase's reach. Whoever reaches the device while it is unlocked
    reaches the whole Library, so it is kept unlocked no longer than the work
    at hand needs (spec: DK-3, DK-4, DK-7 to DK-9).
  - Plaintext files, decrypted caches, and the [Index](../index/) on the
    device are outside it too — guarding those is the job of disk
    encryption.
- The strength of the encryption on Storage comes from the random Master Key
  alone, so a weak Passphrase weakens only the device-local protection,
  never the ciphertext on Storage.

## Related Concepts

- [Master Key](../master-key/) — what the Passphrase protects
- [Specification register](../../spec/) — the behavioral rules cited by ID
