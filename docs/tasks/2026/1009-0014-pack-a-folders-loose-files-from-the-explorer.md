---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1009-0014-pack-a-folders-loose-files-from-the-explorer
created_at: 2026-10-08T15:14:17Z
updated_at: 2026-10-08T16:42:54Z
---

# feat(explorer): pack a folder's loose files into Packs from the explorer, after showing what would be packed

## Overview

A book that was added one file at a time (dropped onto a folder and added "one
by one", or synced from the mapped folder) sits in the Library as many one-file
Containers. `freeze` absorbs such files into Packs (spec: PK-1, PK-2; concept:
Pack), but today only the command line can start it for a chosen folder
(`coffret freeze`). In the explorer, `POST /api/freeze?path=<folder>`
(`backend/crates/apps/coffret-server/src/routes/freeze.rs`) exists only to
retry a stopped drop, and its doc says on purpose that there is no "pack this"
button. The person now wants one: packing a folder they already added is a
daily operation, and the command line should not be needed for it.

**1. What would be packed.** Add a read-only route that answers, for a folder,
what a freeze of it would select on this device: the number of files and their
total size, at every depth under the folder, using the same scan and the same
eligibility rules the freeze applies (PK-1, PK-2, EP-10, PK-17's folder scope)
— not a separate estimate. Files the scan would not select are not counted:
Entries already in a Pack, Entries this device has not materialized, locally
changed files, key-lost Containers (spec: KL-7, KL-17). If it is cheap, also
say how many were left out and why in broad groups (in a Pack / not on this
device / changed here), so the person is not surprised by a smaller count. It
takes no key-requiring action beyond what the scan needs and does not change
the Library.

**2. Ask, then pack.** Give a folder in the explorer an action "Pack this
folder…" (where it fits the existing folder UI). It asks the route above, then
shows a confirmation like the drop's: "📁 BookX — 80 files, 58.0 MB will be
packed into Packs" with "Pack" and "Cancel". Where nothing would be selected,
say so and why instead of offering Pack. Confirming calls
`POST /api/freeze?path=<folder>` and follows the freeze the way a drop's freeze
is followed (progress, packed notice). The freeze is the folder-scoped run
(no selection), so what is packed is exactly what the preview counted, give
or take files that changed between the two calls.

**3. The route's documented purpose.** `POST /api/freeze` stops being "only a
retry": update its doc to say it packs a folder, and that a retry of a stopped
drop with a kept selection is the same call. Keep its existing behaviour and
refusals (unmapped folder; the Library root without a selection). Update the
Library concept's collocations if they describe how a freeze is started from
the explorer.

**4. Where it is offered.** Offer the action only on a folder this device maps
(an unmapped folder cannot be frozen, EP-9) and not on the Library root (a
folder-scoped freeze of the root would be the whole Library). While a freeze
is running, the action still works and queues, as a drop's does, and the
confirmation says it will run after the current one.

Out of scope: regrouping existing Packs (repack / compaction), deleting, and
packing files the device has not materialized (that would need a fetch first).

## Acceptance criteria

### Automated (pipeline-verified)
- [x] The preview route answers the count and size of exactly the files a freeze of the folder would select, at every depth, with a route test that compares it against the freeze's own selection on a folder holding one-file Entries, a Pack, a file not on this device and a locally changed file
- [x] The preview changes nothing in the Library or on disk (test)
- [x] The explorer offers "Pack this folder…" only on a mapped folder that is not the Library root, with tests
- [x] Confirming arms `POST /api/freeze?path=<folder>` and the explorer follows it; Cancel arms nothing; a folder with nothing to select shows why instead of Pack — each with tests
- [x] `POST /api/freeze`'s doc and any concept collocation describe it as packing a folder; existing retry and refusal tests still pass
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Needs a person: under `make desktop-dev`, on a folder whose pages were added one by one, "Pack this folder…" shows the count and size, and confirming ends with the folder's files in Packs
