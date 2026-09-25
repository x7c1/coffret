# Entry Path

## Definition

An **Entry Path** is a canonical, Library-relative position in the logical
namespace of a [Library](../library/). It is a Unicode string normalized to
NFC and encoded as UTF-8, with `/` between components. It is not a raw
platform filesystem path — without one canonical form, the same Library
position would get different names on different platforms.

At each committed Library state, a position may be occupied by a current
[Entry](../container/entry/). Replacing a file's content or its
[Container](../container/) puts a new Entry at the same position; moving the
file removes the old position and adds the new one. For example,
`books/some-novel/page-042.png` keeps the same Entry Path when an updated page
replaces the Entry stored there.

## Collocations

- normalize (a local relative path into an Entry Path)
- compare (Entry Paths for equality or ordering)
- collide (when two local paths normalize to the same Entry Path)
- translate (an Entry Path into a local path through this device's
  [mappings](../mapping/))
- descend (validated relative components from an open mapped root, without
  following links)
- place (an Entry at its local path during a fetch)
- decline (to place an Entry, reporting the reason)
- vouch (for what stands at a local path, as the device, before a fetch places
  an Entry there)
- refuse (a name or path that may not enter the Library at all — malformed,
  unspellable, carrying a reserved name, or folding to one without being it —
  or a placement the device will not make, whether of the one file at that
  path or of every file under a mapped root) — the wider verdict beside
  decline

## Domain Rules

- An Entry Path has exactly one canonical byte form
  (spec: EP-1, EP-2).
  - Who owes that form depends on which side of the Library's boundary the text
    comes from. Text arriving from outside — a name read off a disk, a
    component a mapping is configured with, a prefix a caller narrows a run to —
    is normalized on the way in. Bytes the Library already holds are canonical,
    so a reader that meets a stored path that is not refuses it as malformed
    instead of normalizing it: composing a stored path on the way back would
    change bytes a digest was taken over (spec: EP-1).
- Equality is byte-exact and case-sensitive; ordering is lexicographic over
  the canonical bytes, independent of locale
  (spec: EP-3).
- A **folder** is derived from the Entry Paths rather than stored: nothing
  records one, and a folder exists exactly where a current Entry stands under
  it, so a path several components deep implies every folder along the way and
  the removal of a folder's last Entry ends the folder at that commit
  (spec: EP-2, EP-5).
- A local file that cannot become its own Entry Path — invalid encoding, or a
  collision where two local paths normalize to one — is reported as an
  explicit error, because a silent skip or rename could hide one of the
  user's files (spec: EP-1, EP-4).
  - The placing side owes the same answer. A device that cannot materialize
    two distinct Entry Paths — a filesystem that folds them onto one local
    name — reports an explicit compatibility error rather than putting one of
    them somewhere else or letting one land on the other (spec: EP-4).
- In the current Library state, one Entry Path identifies at most one current
  Entry. The [Journal](../journal/) commit enforces this against the current
  path map — which Entries are live, not which Containers sit on Storage,
  where a replaced Container may still carry the same Entry Path
  (spec: EP-5, EP-6).
- Two concurrent writes to one Entry Path become an explicit conflict
  (spec: EP-7, CP-7).
- A name a person chose never reaches a diagnostic event: no Entry Path, local
  path, or filename goes into one, because an event is a record left behind
  that nobody asked for and that outlives the run. A refusal answering the
  person who asked for the run may name what it refused, naming it being part
  of answering (spec: EL-1).
- A device's local root [mappings](../mapping/) only **translate** Entry
  Paths into local paths; they never assert that the Entries under a mapped
  subtree are on this device, which is what lets a device hold part of a
  Library without the rest looking deleted (spec: EP-9, EP-10).
  - The configured root is a deliberate trust boundary: the device follows the
    root itself if the user configured it through a symbolic link. Below that
    root, scans, source reads, served files, and fetch writes descend validated
    relative components without following links. A reader keeps the regular
    file handle it acquired, including its length, so replacing the name later
    cannot change the bytes already being read (spec: EP-8, EP-11).
  - The other thing a mapping records is the root's own identity: a marker kept
    inside the root's management area, checked before anything is placed and
    never created or repaired by ordinary operation. A disk that came back
    empty, a mount point that never came back, or a folder swapped for another
    of the same name is therefore refused rather than written into — the path
    alone cannot tell the registered folder from one that merely answers to the
    same name (spec: EP-13, EP-14).
  - A scan keeps the filesystem spelling of that validated relative location
    alongside the normalized Entry Path. This matters when a local name is in a
    decomposed Unicode spelling: the Library position is NFC, while reopening
    the source still uses the name that actually appeared in the folder
    (spec: EP-1, EP-8).
- A scan reports an Entry as deleted locally only where this device
  materialized it and the file is gone; [Library](../library/) defines that
  act. An Entry it never materialized is outside its scope, so it is never
  reported as modified, never selected for `update` or `freeze`, and never used
  as the source of a replacement (spec: EP-10).
  - Reporting one also requires the root its path translates under to be
    available, because a path under a missing or swapped root says nothing
    about its file, only about the root (spec: EP-12).
- A fetch **places** an Entry only where this device can vouch for what is at
  the path — nothing there, or a file its own materialization record still
  agrees with — and **declines** every other Entry, reporting the reason,
  because overwriting a file the Library never held would destroy content the
  Library never had a copy of (spec: EP-11, EP-4).
  - Agreement is change detection, not authentication: it says that what this
    device placed is still what stands there, so an edit that leaves the file
    looking untouched reads as unchanged. The guarantee rests on the device
    claiming only a place it left empty or filled itself, not on the check
    recognizing every edit (spec: EP-11).
  - Which reason a refusal stands on decides how far it reaches: one file's
    business is what stands at that path, while a mapping's business is
    whether the root is the folder the mapping was recorded against. A single
    writer handed several placements at once declines each placement refused
    for the first reason beside what it placed and fails the whole request on
    the second, the root being what every one of those placements would have
    gone through (spec: EP-11, EP-13).
  - An Entry becomes visible at its place only once the fetch is verified and
    complete: until the rename that publishes it, the bytes sit in a scratch
    that a scan passes over (spec: EP-11).
  - The name reserved for the device's own management area is refused for
    placement at any depth, and the refusal is reported like any other declined
    Entry. A file put under that name would sit in the person's folder where no
    later scan ever looks at it again, and one put at the marker inside it would
    take the root's identity away — so the path is declined rather than quietly
    written somewhere the Library will never point at, on the same
    no-silent-selection reasoning as the rule above (spec: EP-14, EP-4).
  - The comparison folds ASCII case, and what it finds it does not treat
    alike. The exact name is the device's own management area, which a scan
    passes over in silence; a spelling that merely folds to it is a folder of
    the person's — or, on a volume that folds, one the device cannot tell from
    its own — and is refused and reported everywhere the reservation is asked,
    a scan included. A placement refuses both alike, because either spelling
    would write where the Library will never point. The silence has a price —
    nothing under that folder is backed up — and the price is stated for one
    name: folding admits 128 spellings, so passing over all of them would
    charge it 128 times over without the person ever being told (spec: EP-14).
- [Library](../library/) states this ground from the Library's side — what a
  device's working view may claim about the current state — so the three rules
  above and that account are one rule seen twice.

## Related Concepts

- [Entry](../container/entry/) — the stored file representation that occupies
  an Entry Path in a committed Library state
- [Library](../library/) — the namespace in which an Entry Path is unique
- [Mapping](../mapping/) — translates an Entry Path into a path on this
  device's disks
- [Journal](../journal/) — serializes changes to the current path map
- [Pack](../pack/) — orders Entries by Entry Path
- [Index](../index/) — caches the mapping from Entry Path to Entry location
- [Specification register](../../spec/) — the behavioral rules cited by ID
