---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, spec-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1031-0257-move-files-and-folders-through-the-server-and-let-device-files-follow-or-leave
created_at: 2026-10-30T17:57:23Z
updated_at: 2026-10-30T19:29:21Z
---

# feat: move files and folders through the explorer's server, and let a device's files follow a move or leave the device

## Overview

A rename in the Library — one Journal record giving Entries new Entry Paths
(spec: FM-15) — already works from the command line (`coffret rename --from
--to`). It reads, encrypts and uploads nothing, and a Pack is not rebuilt. The
explorer is about to offer moving and renaming files and folders (by dragging a
row onto a folder, and through "Rename…" / "Move to…" in a row's menu). This
change is the server and device side of that; the explorer's UI comes in a
later change.

**1. Several renames in one record.** The explorer moves a selection at once.
Let one rename run take several `(from, to)` pairs and commit them in one
Journal record, refusing the whole set before anything is written when any pair
is refused (the refusals `RenameRefusal` has today, plus a pair that overlaps
another pair of the same set: a folder and something under it, or two pairs
with the same destination). A folder and an Entry under it both named in the
request are one move of the folder. Keep the command line's single
`--from`/`--to` working on top of it.

**2. A device's own files follow a move across its mapped roots on one
filesystem, and otherwise leave the device.** Today a file this device placed
follows its Entry's new path only within the mapped root it stands in, with one
filesystem rename; across mapped roots, on another filesystem, or where the new
path has no mapped root on this device, the relocation is refused and the file
waits where it stands, reported on every run (spec: EP-10, and the mapping
concept doc: "A rename can give an Entry a current path under another
mapping…"). Change that rule:

- Where the old and the new path lie under two mapped roots that stand on the
  same mounted filesystem (one `st_dev`, recorded for each mapped root by the
  scan, spec: EP-12) and both roots vouch for themselves (EP-13), the file
  follows with one filesystem rename, as within one root. Where the rename is
  refused with `EXDEV` anyway (a bind mount, another btrfs subvolume), treat it
  as the next case.
- Where the file cannot follow — the new path has no mapped root on this
  device, or the roots stand on different filesystems — an unedited file is
  discarded into the desktop's trash as EP-15 discards a departed file, and its
  record is forgotten, so the Entry is "in the Library, not on this device" at
  its new path and opening it fetches it again. The run reports it as
  discarded with its Entry Path.
- An edited file (EP-15's test: length, mtime and content hash against the
  record) is never discarded: it stays where it stands, waiting, and is
  reported, as today.
- This applies wherever a device relocates (sync, folder fetch, and the
  rename run's own relocation on the device that renamed), so every device
  converges on the same rule.

Define "filesystem" once where EP-10 uses it, as "the same mounted filesystem
(one `st_dev`)" — keep the word "filesystem" in the spec. Update EP-10 (and any
rule it cross-references that this changes), the mapping and entry-path concept
docs (`docs/concepts/`), and the tests the spec rules name.

**3. The explorer's routes.** Add, to the loopback API the explorer uses:

- a preview: for a set of `(from, to)` pairs, answer without writing anything
  — the number of Entries moved, whether the set is refused and why (each
  refusal as the existing refusal shapes the delete preview uses), how many of
  this device's files would follow and how many (with their total size) would
  leave this device, and whether any file in the set is edited or any Entry in
  it not yet committed (uploading): both refuse the move, since leaving the
  device would lose what the Library does not hold;
- the move itself: the same pairs, re-checked, committed as one record, then
  this device's relocations run; it answers what it did, and is listed in the
  server's log of finished work (the Activity panel's source, spec: LA-12) like
  a delete.

Model both on the delete routes (`coffret-server/src/routes/delete.rs`):
keyed like every route (LA-2), refused while the Library is locked, the same
error envelope. Add the spec rules for the two routes beside the delete
route's in `docs/spec/` (loopback access), and the contract the frontend's
gateway reads (`frontend/packages/gateway/api/`) with typed calls for both, so
the next change only has to build the UI.

Keep code and comments free of wording that ties coffret to one kind of
content ("a book whose pages are image files", not "scanned book").

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Usecase tests cover a multi-pair rename committed as one record, and the
  refusal of a whole set for one refused or overlapping pair.
- [x] Tests cover a file following a move across two mapped roots on one
  filesystem, an unedited file discarded into the trash where it cannot follow
  (no mapped root at the new path; another filesystem; `EXDEV`), and an edited
  file kept and reported.
- [x] Server route tests cover the preview (counts, leaving-device size,
  refusals for an edited file and for an uploading Entry) and the move (one
  record, relocations, an entry in the finished-work log), and both refused
  while locked.
- [x] EP-10 and the mapping concept doc state the new rule, and the spec
  register's checks pass.
- [x] `coffret rename --from --to` still works (its smoke test passes).

### Before merge (verified outside the check command)

- [x] On `make local`, the agent calls the preview and the move through the
  server for a folder moved within a top-level folder and one moved to
  another top-level folder with no folder chosen on this device, and records
  in the PR what each answered and what happened to the files on disk.
