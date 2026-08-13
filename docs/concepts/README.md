# Concepts

Coffret is a personal, end-to-end-encrypted file library. It encrypts folders
(family photo albums, scanned books) on the user's own machine, stores only
ciphertext on a third-party storage service (Google Drive first), and serves
the files back for fast local browsing. The storage provider never sees
plaintext, file names, or folder structure.

This directory defines the product's shared vocabulary. Each subdirectory
defines one Domain Model; documentation and code use these names in the sense
defined here.

## Concept Map

The user's files form a [Library](library/). [Storage Objects](storage-object/)
are the encrypted objects that represent that Library on [Storage](storage/).
User files are packaged into [Containers](container/), each holding one or more
[Entries](container/entry/) identified by canonical
[Entry Paths](entry-path/), and uploaded under opaque names. A one-time
`freeze` operation sorts the files currently selected from a folder by Entry
Path and cuts them into target-sized segments, each stored as a [Pack](pack/).
An individual Entry larger than the target remains one oversized singleton
Pack. `freeze` does not persist a frozen folder state; files added later remain
one-file Containers until another `freeze`. A book or an album is simply a
folder, opened by fetching the Packs its path range overlaps.

All encryption hangs off a single [Master Key](master-key/): each Container
is encrypted with its own [Container Key](container/container-key/), which
travels as a [Key Envelope](key-envelope/) — its wrapped form — collected in
the [Keyring](keyring/) on Storage. Rotating the Master Key re-wraps these
envelopes under a new Master Key epoch, but never rewrites the data
Containers. On a device, the Master Key is protected by a
[Passphrase](passphrase/); across devices and disasters, it is carried by a
[Recovery Code](recovery-code/).

Bookkeeping uses control Storage Objects, not Containers. Each upload batch
appends a [Journal](journal/) record on Storage recording which Containers it
added and removed — replaying the Journal yields the current Container set,
so even an interrupted replacement or deletion is unambiguous. The
[Keyring](keyring/) checkpoints their Key Envelopes. Locally, the
[Index](index/) is a cache mapping the Library to its Containers, and an
[Index Snapshot](index-snapshot/) uploaded to Storage checkpoints the Journal
and lets a new device rebuild the cache quickly. Journal records, Keyrings,
and Index Snapshots are encrypted directly with purpose-specific keys derived
from the Master Key, so recovery can open them without a Key Envelope.
Restoring current membership requires an intact checkpoint and its
later Journal history. Without that control state, coffret can salvage
decryptable Container contents, but cannot infer committed removals or
replacements from self-description alone.

## Domain Models

- [Library](library/) — the complete set of files a user entrusts to coffret
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
- [Keyring](keyring/) — the control object checkpointing current Key Envelopes
- [Journal](journal/) — the control-object log of Container additions and
  removals on Storage
- [Index](index/) — the local catalog of the Library (a cache)
- [Index Snapshot](index-snapshot/) — a control object containing an uploaded
  copy of the Index and the Journal's checkpoint
