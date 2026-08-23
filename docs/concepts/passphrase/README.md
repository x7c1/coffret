# Passphrase

## Definition

**Passphrase** is the secret the user memorizes to protect the
[Master Key](../master-key/) stored on a device. Each device has its own
Passphrase. Devices using the same Library share its Master Key, but their
Passphrases do not have to match. Entering a device's Passphrase derives the
key that decrypts its stored Master Key, unlocking it for use — without this
protection, anyone holding the device would hold the Library's root secret.

## Collocations

- enter (the Passphrase to unlock the Master Key)
- change (the Passphrase)

## Domain Rules

- Changing the Passphrase on one device re-protects the Master Key stored on
  that device. It does not change any other device's Passphrase or stored
  copy. The Master Key value is unchanged, so every
  [Storage Object](../storage-object/) stays exactly as it is — Containers
  and control objects alike, the [Keyring](../keyring/) included
  (spec: DK-6).
- The Passphrase protects only the stored Master Key: a thief who takes
  the device cannot extract the Master Key and use it to open the control
  state and reachable Key Envelopes on [Storage](../storage/)
  (spec: DK-1, DK-2).
  - An unlocked Master Key sits in memory in the clear, beyond the
    Passphrase's reach. Whoever reaches the device while it is unlocked can
    open the control state and every reachable Key Envelope, so the key is
    kept unlocked no longer than the work at hand needs
    (spec: DK-3, DK-4, DK-7, DK-8, DK-9).
  - Plaintext files, decrypted caches, and the [Index](../index/) on the
    device are outside it too — guarding those is the job of disk
    encryption.
  - What the Master Key seals on the device is protected along with it: a
    thief who cannot unlock the stored Master Key cannot read the OAuth
    token cache either, so a locked device hands over no usable credential
    for Storage (spec: KD-4, KD-10).
- The strength of the encryption on Storage comes from the random Master Key
  alone, so a weak Passphrase weakens only the device-local protection,
  never the ciphertext on Storage (spec: KD-1, KD-5, KD-6, KD-7, KD-8).

## Related Concepts

- [Master Key](../master-key/) — what the Passphrase protects
- [Specification register](../../spec/) — the behavioral rules cited by ID
