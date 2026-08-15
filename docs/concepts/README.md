# Concepts

Coffret is a personal, end-to-end-encrypted file library. It encrypts folders
(family photo albums, scanned books) on the user's own machine, stores only
ciphertext on a third-party storage service (Google Drive first), and serves
the files back for fast local browsing. The storage provider never sees
plaintext, file names, or folder structure.

This directory defines the product's shared vocabulary. Each subdirectory
defines one Domain Model; documentation and code use these names in the sense
defined here. Behavioral rules — the procedures, verifications, and
parameters needed to build the system correctly — live in the normative
[specification register](../spec/), which the concept documents cite by
rule ID as plain text — an ID like `KL-3` is a unique token resolved by
searching the repository, so citations survive the register's shrinkage.

## Concept Map

The user's files form a [Library](library/). [Storage Objects](storage-object/)
are the encrypted objects that represent that Library on [Storage](storage/).
User files are packaged into [Containers](container/), each holding one or more
[Entries](container/entry/) identified by canonical
[Entry Paths](entry-path/), and uploaded under opaque names. A one-time
`freeze` operation gathers eligible local files in a folder, sorts them by
Entry Path, and cuts them into target-sized segments, each stored as a
[Pack](pack/); regrouping existing Packs is a separate repack or compaction
operation. A book or an album is simply a folder, opened by fetching the
Packs its path range overlaps.

All encryption hangs off a single [Master Key](master-key/): each Container
is encrypted with its own [Container Key](container/container-key/), which
travels as a [Key Envelope](key-envelope/) — its wrapped form — owned by the
[Keyring](keyring/) on Storage. Rotating the Master Key re-wraps these
envelopes under a new Master Key epoch and permanently deletes the old
epoch's control objects, but never rewrites the data Containers. On a
device, the Master Key is protected by a
[Passphrase](passphrase/); across devices and disasters, it is carried by a
[Recovery Code](recovery-code/).

Which Containers are current is tracked by control Storage Objects, not by
Containers themselves. Each upload batch
appends a [Journal](journal/) record listing the Containers it added and
removed and selecting, in the same commit, the [Keyring](keyring/) generation
that holds exactly the resulting set's Key Envelopes. Replaying the Journal
yields the current Container set, so even an interrupted replacement or
deletion is unambiguous. Locally, the
[Index](index/) is a cache mapping the Library to its Containers, and an
[Index Snapshot](index-snapshot/) uploaded to Storage checkpoints the Journal
and lets a new device rebuild the cache quickly. Journal records, Keyrings,
and Index Snapshots are encrypted directly with purpose-specific keys derived
from the Master Key, so recovery can open them without a Key Envelope.
Restoring the current Container set requires an intact checkpoint and its
later Journal history; without that control state, coffret can still salvage
decryptable Container contents.

## Domain Models

- [Library](library/) — a set of files a user entrusts to coffret, and the
  scope every other concept lives in
- [Storage Object](storage-object/) — any encrypted object coffret keeps on
  Storage
- [Container](container/) — a self-describing Storage Object holding user data
  - [Entry](container/entry/) — a single file inside a Container
  - [Container Key](container/container-key/) — the key unique to one Container
- [Entry Path](entry-path/) — an Entry's canonical, Library-relative identity
- [Pack](pack/) — a Container created from one path-ordered `freeze` segment
- [Storage](storage/) — the remote object store holding Storage Objects
- [Master Key](master-key/) — the single root secret of a Library
- [Passphrase](passphrase/) — protects the Master Key on a device
- [Recovery Code](recovery-code/) — carries the Master Key across devices
- [Key Envelope](key-envelope/) — a Container Key wrapped under the Master
  Key
- [Keyring](keyring/) — the control object owning current Key Envelopes
- [Journal](journal/) — the control-object log of Container additions and
  removals on Storage
- [Index](index/) — the local catalog of the Library (a cache)
- [Index Snapshot](index-snapshot/) — a control object containing an uploaded
  copy of the Index and the Journal's checkpoint

## Document Format

Each concept document follows one skeleton: **Definition**, optional
**Examples** and **Collocations**, **Domain Rules**, and
**Related Concepts**, with an optional **Mental Model** between Definition
and Examples.

Writing conventions:

- The Definition states why the concept exists — what would break or become
  indistinguishable without it.
- When a concept has a non-trivial lifecycle, a **Mental Model** section
  presents it once, as a compact state table or diagram; the Domain Rules
  then read as consequences of that model (see [Keyring](keyring/) for the
  reference example).
- The litmus test for a Domain Rule: it stays in the concept document only if
  changing it would change what the term means or what a user can rely on —
  guarantees and their limits. A procedure, verification, or parameter needed
  only to build the system correctly belongs in the
  [specification register](../spec/); the concept document keeps at most a
  one-sentence summary citing the spec rule IDs as plain text.
- One rule per bullet, at most two sentences; caveats become sub-bullets.
- Every non-obvious rule carries one clause of why — what breaks otherwise.
- No defensive negations: instead of correcting a misreading after the fact
  ("X does not imply Y"), structure the Definition or Mental Model so the
  misreading cannot arise.
