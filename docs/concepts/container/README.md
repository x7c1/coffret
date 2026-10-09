# Container

## Definition

**Container** is the [Storage Object](../storage-object/) that holds user
data — the unit in which files are encrypted, uploaded, and replaced. A
Container packages one or more [Entries](entry/) together with their
encrypted metadata ([Entry Paths](../entry-path/), timestamps, content
hashes, and each derived Entry's origin) into a single object stored on
[Storage](../storage/) under an opaque,
meaningless name. Which Containers are current, and how to open them, is
tracked instead by the other kind of Storage Object — the control objects
(Journal records, Keyrings, Index Snapshots), which are opened without
Container Keys or Key Envelopes.

A Container is **self-describing** about its content *as of its creation*: the
Entry Paths, timestamps, and hashes that were true when it was written travel
inside it, so knowing what it holds and verifying it needs no record kept
anywhere else. What it does not describe is the present. Whether a Container is
*current* — still in the Library — and what its Entries are called now are
separate questions, and only the [Journal](../journal/) and its checkpoints
answer them. Opening a Container requires the [Master Key](../master-key/) and
the Container's [Key Envelope](../key-envelope/) from the
[Keyring](../keyring/).

## Examples

- A Container holding one photo that was just added to an active album folder
- A pack-policy-managed Container holding a target-sized path-ordered segment
  (a [Pack](../pack/))

## Collocations

- upload (a Container to Storage)
- fetch (a Container from Storage, by the parcel: the parcels overlapping the
  wanted Entries, or every parcel of it)
- open (a Container with the Master Key and its Key Envelope)
- read (a parcel of a Container, or several adjacent ones, never a range
  smaller than a parcel)
- hold (a parcel on the device, so it is never asked of Storage again)
- trash (a superseded or removed Container, after the commit that takes it out
  of the current set)

## Domain Rules

- **Immutable**: a Container is never modified in place. Changing its content
  means uploading a replacement Container, under a new Container ID, and
  trashing the old one (spec: PK-10, PK-12, CP-14).
  - The **entry table** is the list, inside a Container's encrypted meta
    section, recording each Entry's path, times, place in the content stream,
    and hash. It names those paths and timestamps `original_*`: they are
    captured once and never revised, so successive Containers holding one file
    may each record a different name for it, and the [Journal](../journal/) and
    its checkpoints are what say which is the Library's now (spec: FM-9,
    FM-15).
- **Opaque**: a Container's name is drawn independently of its content, so it
  names nothing about what is inside (spec: FM-3). What the provider still
  sees despite opaque naming is listed under
  [Storage Object](../storage-object/).
- **Entries required**: a Container always has at least one Entry. Control
  state lives in control Storage Objects instead (spec: FM-10).
- **Explicit kind**: a Container records whether it is a one-file Container or
  a [Pack](../pack/), and the kind is never inferred from its Entry count — a
  Pack left with a single Entry is still a Pack, and an `update` replacement
  for a one-file Container is still one-file (spec: PK-15).
- **Fetched by the parcel**: a Container's chunk sequence is divided, from
  its first chunk, into **parcels** of a fixed length set by the register, and
  every read of its chunks asks for whole parcels — one, several adjacent
  ones, or every parcel at once. Neither an Entry nor the whole Container is
  the fetch unit; the parcel is (spec: PK-16, PK-19).
  - Parcel boundaries have nothing to do with where Entries begin or end, so
    the storage provider can tell which parcels of which object were read, and
    when, but not where an Entry begins or ends nor how large one is (spec:
    PK-20; the residual leakage is listed under
    [Storage Object](../storage-object/#domain-rules)).
  - An Entry is shown early without a smaller read: its parcel streams, and
    the Entry is released as soon as the chunks covering it have arrived. An
    Entry that spans a parcel boundary is reached by reading every parcel it
    overlaps (spec: PK-16).
  - A device **holds** a parcel it read until every Entry with bytes in it
    that the device maps is on the device or witnessed absent, or until the
    Container leaves the current set, and never asks Storage for a held parcel
    again, so reading a page twice shows the provider nothing. Cancelling and
    reading ahead stop and start at parcel boundaries only (spec: PK-21).
  - The header and meta section at the front of the object belong to no
    parcel: they are read on their own, the same read whichever Entry is
    wanted (spec: PK-16).
  - An Entry taken out of parcels is verified by its chunks' authentication
    and its plaintext hash against the catalog; the Container's ciphertext
    hash is verified only by a read of every parcel (spec: PK-22).
- **Streamable**: the entry table travels ahead of the content (spec: FM-2,
  FM-9) and every chunk authenticates on its own (spec: FM-5), so neither
  writing a Container nor reading one requires holding it in memory. A writer
  fixes the table first and then emits chunk by chunk; a reader releases each
  chunk's plaintext as it verifies.
  - The consecutive chunks covering one plaintext extent are a **chunk run**.
    Where its bytes lie follows from the header and the entry table alone, so a
    reader can name the bytes covering one Entry, and the parcels they lie in,
    before any of them arrive (spec: FM-5, FM-2, FM-9, PK-16). A parcel is a
    chunk run of fixed length at a fixed place (spec: PK-19).

## Related Concepts

- [Entry](entry/) — a single file inside a Container
- [Entry Path](../entry-path/) — names the Library position occupied by each
  current Entry
- [Container Key](container-key/) — the key a Container is encrypted with
- [Key Envelope](../key-envelope/) — the wrapped key that opens a Container
- [Pack](../pack/) — a Container explicitly managed by the pack policy
- [Storage](../storage/) — where Containers are kept
- [Storage Object](../storage-object/) — the broader object category a
  Container belongs to
- [Library](../library/) — where a Container's files come from and return to
- [Specification register](../../spec/) — the behavioral rules cited by ID
