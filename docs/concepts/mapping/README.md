# Mapping

## Definition

A **mapping** is a device's record that one folder on its own disks holds one
part of the [Library](../library/): either the Library root or one top-level
component of the [Entry Path](../entry-path/) namespace. The folder it names is
the **mapped root**, and a **mapped folder** is that root or any folder below
it. Entry Paths are Library-relative and never a device path, so without a
mapping a device would have nowhere to translate one to — no folder to scan for
new files, and no place to put a fetched one.

A mapping belongs to the device that recorded it. It is never uploaded, so two
devices may arrange one Library differently — or one of them may map nothing at
all — and both still catalog the whole Library (spec: EP-9, CK-7).

## Examples

- A laptop maps `albums/` to `/Volumes/Photos/albums` and nothing else: it
  scans and fills that folder alone, while its [Index](../index/) still lists
  every page under `books/`
- A desktop maps its Library root to `~/Coffret` and `albums/` to an external
  disk: the `albums/` mapping represents that subtree, and the root mapping
  represents everything beside it, so a folder called `albums` under
  `~/Coffret` is not a second spelling of the `albums/` subtree
- `coffret map --library family --prefix albums /Volumes/Photos/albums`
  records the first mapping, and `coffret mappings` lists what this device has
  mapped, the Library root first

## Collocations

- map (a local folder to the Library root or to a top-level component) — the
  `map` command
- list (this device's mappings) — the `mappings` command
- represent (a subtree, as a top-level mapping does, or what the top-level
  mappings leave, as the root mapping does)
- reach (a folder of the Library, as the mapping that represents it does) —
  what a listing's `mapped` says of the folder it lists and of each child
  folder in it
- translate (an Entry Path into a local path under the mapped root)
- stamp (the filesystem a mapped root stood on, during a scan)
- adopt (the identity a mapped root's marker already carries, when the mapping
  is recorded)
- issue (a new identity for a mapped root, when a person asks for one)

## Domain Rules

- A device maps a folder either to the Library root or to a top-level
  component, with at most one root mapping and at most one mapping per
  component. Where both kinds are present, a top-level mapping represents its
  subtree and the root mapping represents the remainder, so every Entry Path
  translates through exactly one mapping or through none (spec: EP-9).
  - A top-level mapping is recorded under that component's NFC spelling, the
    one every Entry Path under it carries, so the key a mapping is recorded
    under and the paths a scan composes from it are one string (spec: EP-9,
    EP-1).
  - A mapping whose root is unavailable still represents its subtree, so the
    root mapping neither walks the folder standing where that subtree belongs
    nor reads the subtree as emptied (spec: EP-12).
- A mapping only translates. It never asserts that the Entries it reaches are
  on this device, which is what lets a device hold part of a subtree without
  the rest counting as deleted; [Entry Path](../entry-path/) and
  [Library](../library/) state what a scan may conclude instead (spec: EP-10).
- **What a mapping does assert is two things about its root, and neither is
  what is in it.** The filesystem the root stood on when a scan last looked
  says whether the root is there to be read from — observed, and re-stamped by
  a scan that finds a root holding files on another one — while the identity
  its marker is expected to carry says whether the folder is the one that was
  registered — chosen once, and set by nothing but recording the mapping
  (spec: EP-12, EP-13).
  - The two answer different failures. An unmounted disk leaves an empty
    folder on another filesystem, which reads as a root to reconnect rather
    than an emptied one; a disk that came back empty, or a folder that merely
    answers to the registered name, carries no marker the mapping agrees with,
    and nothing is placed into it (spec: EP-12, EP-13).
- A folder of the Library is **reached** where a mapping represents it. The
  Library root is reached by a root mapping alone, and a folder no mapping
  reaches has nowhere on this device for a file to go, so the explorer is told
  so before it asks for one rather than after a round trip to Storage
  (spec: EP-9, EP-11).
- Mapping a prefix that is already mapped **moves** the mapping rather than
  adding a second one. Everything under the old root leaves the Library's
  reach on this device at that moment, so the device says what the mapping
  stood for before, and recording a mapping afresh clears the filesystem
  identity for the next scan to stamp (spec: EP-9, EP-12).
- The configured root is a deliberate trust boundary: a device follows the
  root itself if the person named it through a symbolic link, and below it
  descends without following links, so a mapping reaches the folder the person
  pointed it at and nothing a link inside it points to (spec: EP-8).
- Recording a mapping needs no [Passphrase](../passphrase/): it is device state
  in a plaintext catalog, and what a device calls its own folders says nothing
  the Library keeps secret.

## Related Concepts

- [Library](../library/) — what a mapping lays out on this device's disks, and
  where the management area and the reserved scratch prefix under a mapped
  root are stated
- [Entry Path](../entry-path/) — what a mapping translates
- [Index](../index/) — keeps the mappings beside the catalog, as device state
- [Specification register](../../spec/) — the behavioral rules cited by ID
