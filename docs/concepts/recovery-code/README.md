# Recovery Code

## Definition

**Recovery Code** is a human-transcribable encoding of the
[Master Key](../master-key/) — something the user can print or write down on
paper. It serves two purposes: the canonical backup of the Master Key, and
the way the Master Key is carried to a new device. Its encoding carries the
Master Key epoch as well as the key, so the epoch in the code identifies
which control objects on [Storage](../storage/) that key opens.

## Collocations

- print / write down (a Recovery Code)
- restore (the Master Key from a Recovery Code)
- enter (a Recovery Code on a new device)

## Domain Rules

- Physical storage on paper is the canonical backup of the **Master Key** —
  not of the [Library](../library/). The Library itself is backed up by
  [Storage](../storage/); restoring it takes both, and the paper alone
  restores nothing.
- Devices are added by entering the Recovery Code; keys are never distributed
  over the network.
- Master Key rotation creates a new Recovery Code for the new epoch. Devices
  holding the previous Master Key must be enrolled again with that code
  (spec: MR-4).

## Related Concepts

- [Master Key](../master-key/) — what a Recovery Code encodes
- [Specification register](../../spec/) — the behavioral rules cited by ID
