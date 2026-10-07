# Catalog

## Definition

**Catalog** is the committed, Library-wide description of the current
[Containers](../container/) and [Entries](../container/entry/), including where
they stand in the [Entry Path](../entry-path/) namespace. It separates what the
Library currently contains from the objects Storage happens to list and the
files any one device happens to hold.

The [Journal](../journal/) records changes to the Catalog, and an
[Index Snapshot](../index-snapshot/) checkpoints it. Each device's
[Index](../index/) caches that state beside its own local records.

## Mental Model

| Information | Authority | Recovered from Storage |
| --- | --- | --- |
| Current Containers, Entries, and the committed checkpoint | Authenticated Journal and Index Snapshots | Yes, while the required control state survives |
| Mapped folders and materialization records | This device | No |
| Spools and pending rows | The creating device | No |

Every device can reconstruct the same Catalog while arranging its files
differently. A device may cache an older committed state; catching up advances
that cache using the available authenticated history (spec: CK-7, CK-9, RV-6).

## Examples

- A laptop lists a book in its Catalog before fetching any of its pages.
- An object left by an interrupted upload is on Storage but outside the
  reconstructed Catalog; its absence alone cannot prove it safe to delete.

## Collocations

- reconstruct (the Catalog from committed control state)
- cache (the Catalog in an Index)
- commit (a change to the Catalog through the Journal)
- checkpoint (the Catalog in an Index Snapshot)

## Domain Rules

- Membership is determined by committed control state, so listing Storage or
  opening Container metadata alone cannot reconstruct the current Catalog
  (spec: CP-1, RV-4, RV-5).
- Every current Entry has one unique current Entry Path, so a device can resolve
  a name to the stored representation it needs (spec: EP-5, EP-6).
- Reconstructing the Catalog proves the state described by the available
  authenticated history; Storage can withhold newer history, so cleanup needs
  separate proof before deleting a suspected orphan (spec: RV-6, OC-1, OC-3).
- A key-lost Container remains in the Catalog with its Entries, because
  membership and the ability to decrypt its ciphertext are separate facts
  (spec: KL-7, RV-7).

## Related Concepts

- [Library](../library/) — whose committed contents the Catalog describes
- [Index](../index/) — the device's cached Catalog and local records
- [Journal](../journal/) — committed changes
- [Index Snapshot](../index-snapshot/) — a checkpointed Catalog
