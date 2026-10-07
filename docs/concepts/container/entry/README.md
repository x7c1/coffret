# Entry

## Definition

**Entry** is one stored representation of a file inside a [Container](../).
Its bytes and captured metadata belong to that immutable Container. The
[Catalog](../../catalog/) describes where a current Entry stands in the
[Entry Path](../../entry-path/) namespace of the [Library](../../library/).
A stored representation and its current name are separate facts.

Replacing or repacking a file creates a new Entry in a new Container, even
when its content or current Entry Path stays the same. Changing only the
current name need not change the stored representation.

## Mental Model

| Question | Answer |
| --- | --- |
| Which stored representation is this? | Its Container ID and entry number: the zero-based position in that Container's immutable entry table |
| What is it called in the current Library? | The Entry Path recorded by the Catalog |
| What name was captured when it was stored? | The Container entry table's `original_path` |
| What content was stored? | The plaintext extent and content hash in the entry table |

An entry number is local to one Container. Every one-file Container has an
Entry numbered zero; two such Entries are distinct because their Container
IDs differ. The number does not identify a file across replacements. Captured
metadata also includes modification time, optional birth time and media type,
and an origin reference for derived data.

The planned rename operation preserves the first and last answers while
changing the second. For example, renaming `books/old/page.png` to
`books/new/page.png` leaves the same bytes in the same Container, with
`books/old/page.png` as that Container's captured name. The implementation
status is stated below.

## Examples

- `books/some-novel/page-042.png` stored as one Entry of a 300-entry
  [Pack](../../pack/)
- A single photo stored as the only Entry of its Container
- A thumbnail coffret generated for that photo, stored as a derived Entry
  recording the photo's Entry as its origin

## Collocations

- verify (an Entry against its recorded hash)

## Domain Rules

- An Entry is indivisible across Containers: a file larger than the Pack size
  target remains one Entry in one oversized singleton Pack (spec: PK-3).
- **Metadata is captured, not maintained**: what an Entry records is what was
  true when its Container was written, and a Container is never rewritten. The
  [Journal](../../journal/) and its checkpoints are the authority for what the
  Library holds now, which is why the entry table spells the recorded name and
  times `original_*` while a record and a checkpoint spell the current ones
  plainly (spec: FM-9, FM-15, FM-16).
- **Birth time is capture-only**: the moment a file came into being is read
  from the local file when the Container is written, and only where the
  platform reports one — an Entry written from a filesystem that keeps none
  records none rather than a stand-in. Unlike a name it cannot be recovered
  once the original file is gone, and a fetch that stamps the file it places
  with the Entry's modification time stamps no birth time onto it
  (spec: FM-9, EP-11).
- **The media type is a hint**: an Entry's recorded media type is a guess made
  at creation and is never what decides whether a client may open the Entry
  (spec: FM-9).
- An Entry may hold **derived data** — a thumbnail or another artifact
  coffret produced from an Entry rather than a file the user entrusted. A
  derived Entry occupies an Entry Path of its own and records its origin:
  the parent's Container ID and Entry Path (spec: FM-9).

## Technical Constraints

Metadata-only rename is planned, not implemented. The current Catalog format
and fetch paths do not yet carry and use an explicit entry number. Shipping
rename requires that reference in Journal additions and Index Snapshots,
ordinal-based reads with Catalog hash verification, and replay of explicit
name changes. Changing only an Index path would not implement rename.

A rename changes neither the captured name nor a derived Entry's captured
origin. Following a renamed parent is a separate Catalog relationship; the
planned derived-data model must define it before rename can cover derived
Entries.

## Related Concepts

- [Container](../) — the encrypted object an Entry lives in
- [Entry Path](../../entry-path/) — the canonical Library position occupied
  by a current Entry
- [Pack](../../pack/) — a Container explicitly managed by the pack policy
- [Catalog](../../catalog/) — current membership and names
- [Library](../../library/) — where Entry Paths point back to
- [Specification register](../../../spec/) — the behavioral rules cited by ID
