# Library

## Definition

**Library** is the complete set of files a user entrusts to coffret. On the
user's machine it is rooted at a single local folder (the library root);
every [Entry](../container/entry/) records an [Entry Path](../entry-path/)
relative to this root.

## Examples

- A family photo collection: `albums/2024-summer/IMG_0001.jpg`, …
- Scanned books: `books/some-novel/page-001.png`, …

## Collocations

- scan (the Library for new or changed files)
- sync (the Library to Storage)
- restore (the current Library state from intact Storage control state)
- salvage (decryptable file contents when Storage control state is incomplete)
- freeze (eligible local files in a folder directly into Packs)

## Domain Rules

- One Library has one active [Master Key](../master-key/) epoch and one
  [Storage](../storage/) location.
- Multiple enrolled devices may write to one Library. Writes are serialized
  at the [Journal](../journal/) commit point; no device is the permanently
  designated writer.
- The Library can be restored from the Master Key and Storage while the
  required control state remains intact. A restore preserves current
  membership, including committed removals and replacements.
- If required Journal history or its Index Snapshot checkpoint is missing,
  coffret can salvage contents from decryptable Containers but cannot prove
  which candidates are current. Salvage is not a restore.
- `freeze` is a one-time packing operation, not a persistent folder state. It
  can pack new local files directly, without first uploading one-file
  Containers, and can replace existing one-file Containers with Packs. It
  leaves no `frozen` flag to restore. Existing Packs are never rewritten by
  `freeze`; files added later become eligible for a later invocation.
  Rewriting existing Packs is a separate repack or compaction operation.

## Related Concepts

- [Container](../container/) — the encrypted unit files are packaged into
- [Entry Path](../entry-path/) — a file's canonical name in the Library
- [Storage](../storage/) — where the encrypted Library lives
- [Index](../index/) — the local catalog of the Library
