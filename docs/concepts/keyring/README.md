# Keyring

## Definition

**Keyring** is the control [Storage Object](../storage-object/) that
checkpoints the current [Key Envelopes](../key-envelope/) of all
[Containers](../container/). Rewriting the Keyring together with the other
compact control objects is what makes rotating the
[Master Key](../master-key/) a megabytes-scale operation instead of rewriting
all Containers.

## Examples

- A Library of ten thousand Containers has a Keyring of roughly 1 MB
- A leaked [Recovery Code](../recovery-code/) — a photographed paper — is
  neutralized by re-wrapping every envelope into a new Keyring and
  permanently deleting the old one, before the attacker ever reaches Storage

## Collocations

- rewrite (the Keyring when rotating the Master Key)
- checkpoint (the Journal's envelopes into the Keyring)
- fetch (the Keyring first, when recovering)

## Domain Rules

- The Keyring is **irreplaceable**: unlike the Index, keys cannot be rebuilt
  from anything else. Recent generations are therefore retained as
  redundant copies, and every [Journal](../journal/) addition carries the
  new envelopes, so several independent copies exist at all times.
- Losing every copy of the Keyring loses the Library, even with the Master
  Key and all Containers — the accepted price of cheap rotation.
- On rotation, old-Master generations are permanently deleted, not trashed:
  they are exactly what a leaked Recovery Code could open.
- A Keyring has no Container Key or Key Envelope of its own. It is encrypted
  and authenticated directly with a purpose-specific key derived from the
  Master Key, so recovery can open the Keyring without already having the
  Keyring.

## Related Concepts

- [Key Envelope](../key-envelope/) — what the Keyring collects
- [Journal](../journal/) — carries envelopes between checkpoints
- [Master Key](../master-key/) — what rotation replaces
- [Storage](../storage/) — where the Keyring lives
- [Storage Object](../storage-object/) — the broader object category a
  Keyring belongs to
