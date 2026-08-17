# Recovery Code

## Definition

**Recovery Code** is a human-transcribable encoding of the
[Master Key](../master-key/) — something the user can print or write down on
paper. It serves two purposes: the canonical backup of the Master Key, and
the way the Master Key is carried to a new device. Its encoding carries the
Master Key epoch as well as the key, so the epoch in the code identifies
which control objects on [Storage](../storage/) that key opens.

It is not an identity check or a password-reset token: anyone who has the code
has the Master Key it carries and does not need a device's Passphrase.

## Collocations

- print / write down (a Recovery Code)
- restore (the Master Key from a Recovery Code)
- enter (a Recovery Code on a new device)

## Domain Rules

- A Recovery Code backs up the **Master Key**; [Storage](../storage/) holds
  the [Library](../library/)'s data. A restore draws on both — the code
  supplies the key, and Storage must still supply the required control state,
  Key Envelopes, and Containers. The code cannot recreate anything missing
  from Storage.
- A Recovery Code alone contains no Library files. Someone who has both the
  code and access to the matching Storage can open its control state and every
  current Container that still has a reachable Key Envelope; a key-lost
  Container remains locked (spec: RV-2, RV-3, RV-7).
- The code must be kept secret like the Master Key itself. A photograph or
  text copy is enough to use it, so it should be kept separately from Storage
  access where practical.
- If a code may have leaked, Master Key rotation replaces it. The old code
  cannot open new-epoch control objects, but it remains useful with old-epoch
  control objects until they are permanently deleted. Rotation cannot
  invalidate copies of those objects that someone already kept (spec: MR-3).
- Devices are added by entering the Recovery Code; keys are never distributed
  over the network.
- Master Key rotation creates a new Recovery Code for the new epoch. Devices
  holding the previous Master Key must be enrolled again with that code
  (spec: MR-4).

## Related Concepts

- [Master Key](../master-key/) — what a Recovery Code encodes
- [Specification register](../../spec/) — the behavioral rules cited by ID
