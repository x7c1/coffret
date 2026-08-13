# Container

## Definition

**Container** is the unit of encrypted data kept on [Storage](../storage/).
A Container packages one or more [Entries](entry/) together with their
encrypted metadata (original paths, timestamps, content hashes) into a single
object stored under an opaque, meaningless name.

A Container is **self-describing**: the Container alone, plus the
[Master Key](../master-key/), is sufficient to restore its original files —
no external index or catalog is required.

## Examples

- A Container holding one photo that was just added to an active album folder
- A Container holding a ~1 GiB path-ordered segment of a frozen folder (a
  [Pack](../pack/))

## Collocations

- upload (a Container to Storage)
- fetch (a Container from Storage)
- open (a Container with the Master Key)
- trash (a superseded Container)

## Domain Rules

- **Immutable**: a Container is never modified in place. Changing its content
  means uploading a replacement Container and trashing the old one.
- **Opaque**: the name and outward appearance of a Container reveal nothing
  about its content.

## Related Concepts

- [Entry](entry/) — a single file inside a Container
- [Container Key](container-key/) — the key a Container is encrypted with
- [Pack](../pack/) — a Container holding one path-ordered segment of frozen
  files
- [Storage](../storage/) — where Containers are kept
- [Library](../library/) — where a Container's files come from and return to
