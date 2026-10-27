# Catalog

## Definition

**Catalog** is the committed, Library-wide description of the current
[Containers](../container/) and [Entries](../container/entry/), including where
they stand in the [Entry Path](../entry-path/) namespace, and of the Library's
name ([Library](../library/)). It separates what the Library currently
contains from the objects Storage happens to list and the files any one
device happens to hold.

The [Journal](../journal/) records changes to the Catalog, and an
[Index Snapshot](../index-snapshot/) checkpoints it. Each device's
[Index](../index/) caches that state beside its own local records.

## Mental Model

| Information | Authority | Recovered from Storage |
| --- | --- | --- |
| Current Containers, Entries, the Library name, and the committed checkpoint | Authenticated Journal and Index Snapshots | Yes, while the required control state survives |
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
- deposit (content into the Library), carry (a deposit time forward)
- rename (an Entry, changing its current Entry Path) — from the command line,
  or by a sync carrying a file renamed on disk

## Domain Rules

- Membership is determined by committed control state, so listing Storage or
  opening Container metadata alone cannot reconstruct the current Catalog
  (spec: CP-1, RV-4, RV-5).
- Every current Entry has one unique current Entry Path, so a device can resolve
  a name to the stored representation it needs (spec: EP-5, EP-6).
- Beside its current Entry Path, the Catalog keeps each Entry's **captured
  path**: the Entry Path the Container holding it recorded when it was
  written. The two are equal until a rename changes the Entry's path, and only
  the current one changes then, because a Container is never rewritten. The
  captured path changes only when a new Container takes the Entry and records
  the name current at that write, so it is not necessarily the first name the
  Entry ever had. Keeping the captured
  path in the Catalog lets every device name both from control state
  alone, rather than by fetching and decrypting the Container that recorded it.
  Control state carries the captured path only where it differs from the
  current one, so an Entry pays for its second name only once it has been
  renamed (spec: FM-9, FM-15, FM-16).
- The Catalog keeps, for each current Entry, its **deposit time**: when the
  content now at its Entry Path was deposited into the Library. A **deposit** is
  putting new content in — adding a file, adding one again where a deletion left
  nothing, replacing one's content with different content — and each deposit
  sets a new time, stamped once by the device that prepares the batch, with its
  own clock. Whether content is new is decided by its hash. What puts no new
  content in carries the time forward: a rename keeps it, and a new Entry that
  carries an existing Entry's content takes that Entry's time. A Pack rewrite, a
  freeze absorbing an unchanged file, and a replacement with the same content (a
  key-lost Entry healed from its unchanged file among them) each make such an
  Entry. A restore is no deposit and recovers every time unchanged (spec: FM-15,
  FM-16, PK-10, PK-11, PK-12, PK-13, RV-1).
- The deposit time is a separate axis from an Entry's modification and birth
  times, which describe the original file: a photograph taken years ago and
  deposited today has an old modification time and today's deposit time. It is
  the Catalog's value, so control state carries it and no Container does
  (spec: FM-9, FM-15).
- The Catalog refers to an Entry by its Container ID and **entry number**,
  the Entry's position in that Container's immutable entry table, and not by
  its name: the pair names one Entry whatever path it currently stands at,
  which is what lets a change of name be recorded without the name it
  replaces (spec: FM-9, FM-15, FM-16).
- The Catalog keeps the **Library name**: the one the latest Journal record
  carrying a name gave. The record that creates the Library carries the name it
  is created with, a later record carries one only where it gives the Library a
  new one, and a checkpoint carries the name the records before it left, so
  pruning them does not lose it. A reader that does not know the name loses
  only the name: it is not must-understand (spec: FM-15, FM-16).
- Reconstructing the Catalog proves the state described by the available
  authenticated history; Storage can withhold newer history, so cleanup needs
  separate proof before deleting a suspected orphan (spec: RV-6, OC-1, OC-3).
- A key-lost Container remains in the Catalog with its Entries, because
  membership and the ability to decrypt its ciphertext are separate facts
  (spec: KL-7, RV-7).

## Technical Constraints

A rename changes an Entry's current path while preserving its stored identity,
the Container ID and entry number. Restore recovers that name from control
state; [salvage](../journal/#domain-rules) can recover only the name captured
in the Container. Replaying the Journal applies the renames its records carry,
so every device that catches up reaches the same current paths, and a record
whose renames the Catalog does not admit is refused whole (spec: FM-15, RV-5).
A batch committed after another device's rename is rebased over it by the
same pair: a change that replaces the renamed Entry takes it over at its new
path, and a change that writes to the same Entry or Entry Path from the other
side is a conflict (spec: CP-4, CP-7, EP-7).
Fetch reads an Entry by its entry number and places it at its current path
(spec: EP-11). A device relocates its file of a renamed Entry to the Entry's
current path in its next sync or fetch, and the device that made the rename
does so right after the commit; until it can, the file stays where it was
placed, and the device neither carries it in as a new file nor treats it as
deleted (spec: EP-10). A rename is written as one record naming every Entry
it renames, and no Container is replaced or uploaded for it
(see [Library](../library/#domain-rules)). A sync may write renames too: where
a person renamed on disk a file this device placed, and no other file the sync
scanned holds that content, the sync renames its Entry in the record that
carries the rest of its batch (spec: EP-10).

## Related Concepts

- [Library](../library/) — whose committed contents the Catalog describes
- [Index](../index/) — the device's cached Catalog and local records
- [Journal](../journal/) — committed changes
- [Index Snapshot](../index-snapshot/) — a checkpointed Catalog
