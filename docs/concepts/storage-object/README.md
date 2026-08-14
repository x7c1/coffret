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
  [Keyrings](../keyring/), and [Index Snapshots](../index-snapshot/) — which
  make Containers discoverable and recoverable.

Control objects are encrypted and authenticated directly with
purpose-specific keys derived from the [Master Key](../master-key/). They do
not have Container Keys or Key Envelopes. This gives recovery an acyclic
bootstrap path: derive the control keys, open the Keyring and Journal, then
use the Keyring's Key Envelopes to open Containers.

Together, the information the current control objects carry — the Journal
records, their Index Snapshot checkpoints, and the committed Keyring — is
the Library's **control state**: what determines which Containers are
current and how to open them. Other documents use the term in this sense.

## Examples

- An opaque, randomly named Container holding one photo
- A recognizably named Journal record committing an upload batch
- One replica of the current Keyring generation, holding the current Key
  Envelopes

## Collocations

- upload (a Storage Object to Storage)
- fetch (a Storage Object from Storage)
- identify (a control object by its recovery name)

## Domain Rules

- Every Storage Object is either a Container or a control object, never
  both.
- Containers have opaque names. Control objects have recognizable names
  so recovery can find them without an Index; their type and update frequency
  are accepted metadata leakage.
  - Opaque naming still leaves the provider the Containers' existence, their
    count, their padded ciphertext sizes (spec: PK-6), and the timing and
    pattern of uploads and reads — accepted residual leakage.
- Control-object keys are domain-separated by purpose: a key derived for a
  Journal record is never used for a Keyring or an Index Snapshot
  (spec: RV-3).
- Every control object belongs to the Master Key epoch that encrypts it.

## Related Concepts

- [Storage](../storage/) — where Storage Objects live
- [Container](../container/) — the Storage Object that holds user data
- [Journal](../journal/), [Keyring](../keyring/), and
  [Index Snapshot](../index-snapshot/) — control Storage Objects
- [Master Key](../master-key/) — the root of control-object keys
- [Specification register](../../spec/) — the behavioral rules cited by ID
