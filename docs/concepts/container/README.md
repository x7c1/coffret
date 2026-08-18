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

A Container is **self-describing** about its content: Entry Paths,
timestamps, and hashes travel inside it, so no external catalog is needed to
know what it holds. Whether it is *current* — still in the Library — is a
separate question, and only the [Journal](../journal/) and its checkpoints
answer it. Opening a Container requires the [Master Key](../master-key/) and
the Container's [Key Envelope](../key-envelope/) from the
[Keyring](../keyring/).

## Examples

- A Container holding one photo that was just added to an active album folder
- A pack-policy-managed Container holding a target-sized path-ordered segment
  (a [Pack](../pack/))

## Collocations

- upload (a Container to Storage)
- fetch (a Container from Storage)
- open (a Container with the Master Key and its Key Envelope)
- trash (a superseded Container)

## Domain Rules

- **Immutable**: a Container is never modified in place. Changing its content
  means uploading a replacement Container, under a new Container ID, and
  trashing the old one (spec: PK-10, PK-12, CP-14).
- **Opaque**: a Container's name is drawn independently of its content, so it
  names nothing about what is inside (spec: FM-3). What the provider still
  sees despite opaque naming is listed under
  [Storage Object](../storage-object/).
- **Entries required**: a Container always has at least one Entry. Control
  state lives in control Storage Objects instead (spec: FM-10).
- **Fetched whole**: the normal fetch unit is a whole Container, not an
  individual Entry. This granularity bounds how much of a reading pattern the
  storage provider observes (spec: PK-16).

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
