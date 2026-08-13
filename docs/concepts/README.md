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

The user's files form a [Library](library/). Files are packaged into
encrypted [Containers](container/), each holding one or more
[Entries](container/entry/), and uploaded to [Storage](storage/) under opaque
names. A Container that bundles files of one complete unit — such as one
scanned book — is a [Pack](pack/); a unit larger than the size target spans
several Packs.

All encryption hangs off a single [Master Key](master-key/): each Container
is encrypted with its own [Container Key](container/container-key/), which
travels inside the Container wrapped under the Master Key. On a device, the
Master Key is protected by a [Passphrase](passphrase/); across devices and
disasters, it is carried by a [Recovery Code](recovery-code/).

Bookkeeping is local: the [Index](index/) is a cache that maps the Library to
its Containers, and an [Index Snapshot](index-snapshot/) uploaded to Storage
lets a new device rebuild that cache quickly. Storage plus the Master Key is
always sufficient to restore everything.

## Domain Models

- [Library](library/) — the complete set of files a user entrusts to coffret
- [Container](container/) — the self-describing encrypted unit kept on Storage
  - [Entry](container/entry/) — a single file inside a Container
  - [Container Key](container/container-key/) — the key unique to one Container
- [Pack](pack/) — a Container bundling (a slice of) one complete unit
- [Storage](storage/) — the remote object store holding the Containers
- [Master Key](master-key/) — the single root secret of a Library
- [Passphrase](passphrase/) — protects the Master Key on a device
- [Recovery Code](recovery-code/) — carries the Master Key across devices
- [Index](index/) — the local catalog of the Library (a cache)
- [Index Snapshot](index-snapshot/) — an uploaded copy of the Index
