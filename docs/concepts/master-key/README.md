# Master Key

## Definition

**Master Key** is the current root secret of a [Library](../library/). Every
other key in coffret is derived from it or wrapped under it. It opens the
control objects and unwraps the Key Envelopes that [Storage](../storage/)
still carries; it cannot recreate missing control state, Containers, or
envelopes. An exact restore therefore needs both the Master Key and intact
Storage containing the required control state, the current Containers, and a
committed valid Keyring. A current Container recorded as key-lost remains
present but locked. Replacing the Master Key starts a new **Master Key
epoch**.

## Collocations

- unlock (the Master Key with the Passphrase)
- derive (keys from the Master Key)
- back up (the Master Key as a Recovery Code)
- import (the Master Key on a new device)
- rotate (the Master Key into a new epoch)
- activate (a prepared Master Key epoch)

## Domain Rules

- The Master Key is generated randomly, independent of the
  [Passphrase](../passphrase/), so the strength of the encryption on Storage
  never depends on passphrase quality (spec: KD-1).
- The Master Key never leaves the user's devices except as a
  [Recovery Code](../recovery-code/); the storage provider receives wrapped
  [Container Keys](../container/container-key/), but never an unwrapped key,
  nor any stored value that would let a thief test Passphrase guesses
  offline (spec: KD-8).
- Purpose-specific keys derived from the Master Key directly encrypt control
  [Storage Objects](../storage-object/) such as [Journal](../journal/)
  records, [Keyrings](../keyring/), and
  [Index Snapshots](../index-snapshot/) (spec: KD-3, KD-4, RV-3).
- Exactly one Master Key epoch is active for a Library, and only rotation
  starts a new one; each control object separately numbers its own
  `generation` — its update counter within an epoch — so `master_key_epoch`
  and `generation` count different things.
- Rotation re-wraps every current Container Key and refreshes the control
  objects under a new Master Key, while Containers remain byte-for-byte
  unchanged (spec: MR-1, MR-2).
  - Rotation is a prepare-then-activate two-step: the new epoch's control
    objects are prepared first, then the activation Index Snapshot takes the
    current commit slot, fencing old-epoch writers (spec: MR-2).
- Rotation is complete only after every old-epoch control object reachable by
  coffret has been permanently deleted
  (spec: MR-3).
  - A copy retained by an attacker or the Storage provider before deletion
    remains readable with the old Master Key and cannot be invalidated by
    rotation.
  - Because rotation re-wraps envelopes without changing the Container Keys
    inside them (spec: MR-1), a retained old-epoch Keyring plus the old
    Master Key still opens the Containers that survive into the new epoch.
- Losing every device copy **and** every Recovery Code makes an exact restore
  from Storage permanently impossible. Surviving local plaintext or
  authenticated Container Key material lies outside that restore guarantee.
  This is accepted by design and must be made unmistakably clear to the user.

## Related Concepts

- [Passphrase](../passphrase/) — protects the Master Key on a device
- [Recovery Code](../recovery-code/) — carries the Master Key across devices
- [Container Key](../container/container-key/) — wrapped under the Master Key
- [Storage Object](../storage-object/) — control objects use keys derived from
  the Master Key
- [Specification register](../../spec/) — the behavioral rules cited by ID
