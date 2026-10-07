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
| Which stored representation is this? | Its Container ID and entry number: the zero-based position in that Container's immutable [entry table](../#domain-rules) |
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
- A thumbnail generated for that photo, stored as a derived Entry recording
  its origin (planned)

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
- An Entry may hold **derived data**: an artifact produced from a source Entry,
  such as a thumbnail. Its captured origin describes the source at generation
  time; it is not automatically a link to that source's current name.

### Planned derived data

The adopted design records the source's Entry Path at generation time and
its plaintext BLAKE3-256 hash. Repacking the same content into a different
Container does not make the derived data stale. Once the current source is
resolved, a different content hash means the derived data is stale; the
captured path alone cannot resolve a source after it has been renamed.

Derived Entries occupy `.coffret-derived/<kind>/<source Entry Path>`, initially
with `thumb` as the kind. This top-level component is reserved for coffret;
a user's file that would occupy it must be reported explicitly, never silently
excluded. Derived Entries stay out of user file listings, local-file scan and
freeze/update candidate selection, and materialization into mapped folders.
Decoded thumbnails belong in a disposable device cache.

**Derive** creates derived Entries and groups them into Packs separate from
the originals. It is distinct from freeze, which groups source files, but both
commit in the same Journal batch so the original and derived Packs appear
together. Ordinary one-file sync does not upload derived Entries. Adoption of
this design does not mean thumbnail generation is implemented or scheduled
for the current release.

## Technical Constraints

Metadata-only rename is planned, not implemented. The current Catalog format
and fetch paths do not yet carry and use an explicit entry number. Shipping
rename requires that reference in Journal additions and Index Snapshots,
ordinal-based reads with Catalog hash verification, and replay of explicit
name changes. Rewriting the Entry Path a device's [Index](../../index/)
caches would not implement rename either: every device takes current names
from the Journal and its checkpoints, so a name changed only in one device's
cache reaches no other device and is lost when that cache is rebuilt.

The current format still encodes a derived origin as the parent's Container
ID and captured Entry Path (spec: FM-9). The planned path-and-hash origin
requires coordinated format, Catalog, reader, and compatibility changes; see
[the origin transition](../../../spec/format/derived-origin-transition.md).
The reserved namespace and derive behavior above are also planned.

How the Catalog resolves a renamed source, how a derived Entry follows that
rename, and what happens when its source is deleted remain design questions.
A stored origin never changes to answer them. Equal hashes alone must not be
assumed to identify one source: different files can have identical content.

## Related Concepts

- [Container](../) — the encrypted object an Entry lives in
- [Entry Path](../../entry-path/) — the canonical Library position occupied
  by a current Entry
- [Pack](../../pack/) — a Container explicitly managed by the pack policy
- [Catalog](../../catalog/) — current membership and names
- [Library](../../library/) — where Entry Paths point back to
- [Specification register](../../../spec/) — the behavioral rules cited by ID
