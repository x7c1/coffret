# Entry

## Definition

**Entry** is one stored representation of a file inside a [Container](../),
together with its metadata: the [Entry Path](../../entry-path/) where it
participates in the [Library](../../library/), the modification time, the
birth time where the platform that wrote the Container reported one, a hash
of the file content, an optional media type, and — when the Entry holds
derived data — a reference to the Entry it was produced from. Replacing a
file creates a new
Entry, even when the new Entry occupies the same Entry Path. The metadata
preserves the file's Library name as of the moment the Container was written
and lets its stored content be verified inside an otherwise opaque
Container.

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

## Related Concepts

- [Container](../) — the encrypted object an Entry lives in
- [Entry Path](../../entry-path/) — the canonical Library position occupied
  by a current Entry
- [Pack](../../pack/) — a Container explicitly managed by the pack policy
- [Library](../../library/) — where Entry Paths point back to
- [Specification register](../../../spec/) — the behavioral rules cited by ID
