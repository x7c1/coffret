# Recovery Code

## Definition

**Recovery Code** is a human-transcribable encoding of the
[Master Key](../master-key/) — something the user can print or write down on
paper. It serves two purposes: the canonical backup of the Master Key, and
the way the Master Key is carried to a new device. Its encoding carries the
Master Key epoch as well as the key, so an imported key can be matched to its
control state.

## Collocations

- print / write down (a Recovery Code)
- restore (the Master Key from a Recovery Code)
- enter (a Recovery Code on a new device)

## Domain Rules

- Physical storage on paper is the canonical backup of the **Master Key** —
  not of the Library. The Library itself is backed up by
  [Storage](../storage/); restoring it takes both, and the paper alone
  restores nothing.
- Devices are added by entering the Recovery Code; keys are never distributed
  over the network.
- Master Key rotation creates a new Recovery Code. Coffret does not prepare
  the new epoch on Storage until the user confirms that this code has been
  backed up. Devices holding the old code or old Master Key must be enrolled
  again with the new code after activation.
- Deleting every reachable old-epoch control object prevents the old code
  from opening the live Library through coffret. It cannot revoke copies an
  attacker or the storage provider retained before deletion.

## Related Concepts

- [Master Key](../master-key/) — what a Recovery Code encodes
