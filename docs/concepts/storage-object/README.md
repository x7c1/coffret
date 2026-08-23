# Storage Object

## Definition

**Storage Object** is any encrypted object coffret keeps in
[Storage](../storage/) as part of a [Library](../library/). There are two
disjoint kinds:

- [Containers](../container/), which hold one or more user-file
  [Entries](../container/entry/) and are encrypted with random
  [Container Keys](../container/container-key/) carried as
  [Key Envelopes](../key-envelope/); and
- control objects — [Journal](../journal/) records,
  [Keyrings](../keyring/), and [Index Snapshots](../index-snapshot/),
  ordinary and activation — which make Containers discoverable and
  recoverable.

Control objects are encrypted and authenticated directly with
[purpose keys](../purpose-key/) derived from the
[Master Key](../master-key/). They do
not have Container Keys or Key Envelopes. This gives recovery an acyclic
bootstrap path: derive the control keys, open the Keyring and Journal, then
use the Keyring's available Key Envelopes to open Containers.

Together, the information the current control objects carry — the Journal
records, their Index Snapshot checkpoints, and the committed Keyring — is
the Library's **control state**: what determines which Containers are
current and whether and how they can be opened. Other documents use the term
in this sense.

## Examples

- An opaque, randomly named Container holding one photo
- A recognizably named Journal record committing an upload batch, named for
  its position in the control-head chain rather than for being a Journal
  record
- One replica of the current Keyring generation, holding the mapping from
  current Containers to Key Envelopes or key-lost markers

## Collocations

- upload (a Storage Object to Storage)
- fetch (a Storage Object from Storage)
- discover (a control object by its name)
- trash (a Storage Object)
- purge (an old-epoch control object)

## Domain Rules

- Every Storage Object is either a Container or a control object, never
  both.
- Containers have opaque names. Control objects have recognizable names
  so recovery can find them without an Index; their type and update frequency
  are accepted metadata leakage (spec: FM-3, FM-12).
  - A control object's name says what it is **for** — its **role**: a link in
    the control-head chain, a checkpoint, a Keyring replica — while what it
    **is** rides inside the authenticated object, along with its generation
    and replica position. One role admits more than one kind, so an object is
    rejected when its name admits no object of the kind it declares, or
    disagrees with it about generation or replica position
    (spec: FM-11, FM-12).
  - Opaque naming still leaves the provider the Containers' existence, their
    count, their padded ciphertext sizes (spec: PK-6), and the timing and
    pattern of uploads and reads — accepted residual leakage.
  - The Container header is plaintext, so the provider also sees each
    object's format version, chunk size, and padded meta section length —
    the meta section is size-padded like the content stream, so only a
    bucketed approximation of the Entry count and total Entry Path length
    remains visible (spec: FM-2, FM-9).
- Control-object keys are domain-separated by purpose: a key derived for a
  Journal record is never used for a Keyring or an Index Snapshot, and an
  activation Index Snapshot has a key of its own too (spec: KD-4, RV-3).
- Every control object belongs to the Master Key epoch that encrypts it
  (spec: FM-13).
- Removal comes in two kinds. To **trash** a Storage Object is a recoverable
  soft delete: the object disappears from listings but stays the same object
  and can be restored. To **purge** it is irreversible — what Master Key
  rotation applies to old-epoch control objects, complete only when a
  read-back confirms the object is gone (spec: MR-3).

## Related Concepts

- [Storage](../storage/) — where Storage Objects live
- [Container](../container/) — the Storage Object that holds user data
- [Journal](../journal/), [Keyring](../keyring/), and
  [Index Snapshot](../index-snapshot/) — control Storage Objects
- [Master Key](../master-key/) — the root of control-object keys
- [Purpose Key](../purpose-key/) — what control objects are encrypted with
- [Specification register](../../spec/) — the behavioral rules cited by ID
