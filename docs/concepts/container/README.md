# Container

## Definition

**Container** is the [Storage Object](../storage-object/) that holds user data.
A Container packages one or more [Entries](entry/) together with their
encrypted metadata ([Entry Paths](../entry-path/), timestamps, content hashes)
into a single object stored on [Storage](../storage/) under an opaque,
meaningless name.
Control objects such as Journal records, Keyrings, and Index Snapshots are not
Containers and do not have Container Keys or Key Envelopes.

A Container is **self-describing** about its content: Entry Paths,
timestamps, and hashes travel inside it, so no external catalog is needed to
know what it holds. Self-description does not say whether the Container is
current, removed, replaced, or uncommitted; that membership comes from the
[Journal](../journal/) and its checkpoints. Opening a Container requires the
[Master Key](../master-key/) and the Container's
[Key Envelope](../key-envelope/) from the [Keyring](../keyring/).

## Examples

- A Container holding one photo that was just added to an active album folder
- A Container holding a target-sized path-ordered segment created by `freeze` (a
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
- **Entries required**: a Container always has at least one Entry. Bookkeeping and
  recovery metadata live in control Storage Objects instead.

## Related Concepts

- [Entry](entry/) — a single file inside a Container
- [Entry Path](../entry-path/) — identifies each Entry in the Library
- [Container Key](container-key/) — the key a Container is encrypted with
- [Key Envelope](../key-envelope/) — the wrapped key that opens a Container
- [Pack](../pack/) — a Container holding one path-ordered `freeze` segment
- [Storage](../storage/) — where Containers are kept
- [Storage Object](../storage-object/) — the broader object category a
  Container belongs to
- [Library](../library/) — where a Container's files come from and return to
