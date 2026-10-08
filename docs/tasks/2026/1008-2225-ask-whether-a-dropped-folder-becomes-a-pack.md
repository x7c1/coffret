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
branch: task/1008-2225-ask-whether-a-dropped-folder-becomes-a-pack
created_at: 2026-10-08T13:25:51Z
updated_at: 2026-10-08T14:59:39Z
---

# feat(explorer): ask whether a dropped folder becomes a Pack, and pack only what was dropped

## Overview

Today whether a drop is packed depends on how the folder was made, not on what
was dropped. Dropping onto a folder made with "new folder in the Library"
arms a freeze (`frontend/packages/apps/web/src/App.tsx`, `bookDrop` /
`isPending`; `dropTarget.ts`); dropping anywhere else sends the files one by
one. And the freeze a drop arms is scoped to the folder's prefix
(`FreezeRequest.prefix`, `coffret-usecase/src/freeze/freeze_request.rs`), so
it selects every eligible file under that folder — including one-file Entries
that were there before the drop (spec: PK-1, PK-17). What the person was shown
and what gets packed can differ.

Make the choice follow what was dropped, ask before sending, and pack exactly
the files the drop carried.

**1. Ask for a folder drop.** When the drop holds at least one folder, once
the explorer has finished reading what was dropped (today's "reading what was
dropped…") and before anything is sent, show a confirmation naming the folder
and its totals, e.g. "📁 BookX — 80 files, 58.0 MB", with three choices:
"Add as a Pack (recommended)", "Add the 80 files one by one", and "Cancel".
Nothing has been sent yet, so Cancel needs no clean-up.
- Count every file at every depth under the dropped folder. Do not predict how
  many Packs result: Packs are split by freeze's target size, not by folder
  (concept: Pack).
- The choice applies to the whole drop. Where several folders are dropped at
  once, ask once with each folder's totals listed; loose files dropped beside
  folders follow the same choice.
- A drop of files only (no folder) is not asked about: the files are added one
  by one, as today.
- The pre-send budget check (files over the part budget, too many files) runs
  before the confirmation, so a drop that cannot be sent is refused without
  asking.

**2. A folder that already exists.** If the Library already has a folder with
the dropped folder's name at the drop location, the confirmation says so
("BookX already exists here") and still offers the same choices plus Cancel.
Adding proceeds as today (PK-10): a file whose path is held by a Pack is refused on
its own, and a file whose path holds a one-file Entry replaces it. The
confirmation says so. If the person chose a Pack, only the files this drop
carried are packed; one-file Entries already in that folder are not drawn in.

**3. Pack exactly the dropped files.** Give the freeze an explicit selection:
the Entry Paths the upload wrote. Add it to `FreezeRequest` (for example
`only: Option<…set of EntryPath…>`, narrowing the prefix and never widening
it) and have the upload route arm the freeze with the paths it received when
the drop asked for a Pack. The scan still applies every eligibility rule
(PK-1, PK-2, EP-10): a path in the selection that is not eligible is not
packed. State the selection in the spec where the freeze's scope is defined
(PK-17 or the rule that defines what a drop arms), and update the Pack and
Library concept docs if they describe a drop's freeze as covering the folder.

**4. "New folder in the Library" only makes a folder.** It no longer decides
how a later drop is added; remove the pending-book state that did
(`isPending`, `bookDrop`, the drop-target wording `dropTarget.ts` derives from
it) or reduce it to what is still needed. The drop-target banner says what a
drop here will do in the new terms (a folder will be asked about; files are
added one by one).

Out of scope: packing a folder that already holds one-file Entries from the
explorer (a separate change), deleting, renaming, and interrupting a drop.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] A drop holding a folder shows a confirmation with the folder's file count and total size before anything is sent, with tests; a files-only drop sends without asking
- [x] Choosing Pack arms a freeze whose selection is exactly the paths the upload wrote; choosing one-by-one arms a sync; Cancel sends nothing — each with a test
- [x] Dropping a folder into an existing same-name folder that already holds one-file Entries packs only the dropped files, with a conformance or route test showing the earlier Entries stay in their one-file Containers
- [x] The displayed count equals the number of files the request carries, at every depth, with a test
- [x] A drop refused by the pre-send budget check is refused without a confirmation, with a test
- [x] "New folder in the Library" no longer changes how a later drop is added
- [x] The spec states the freeze's explicit selection, and `make spec-citations spec-rule-ids` passes
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Needs a person: under `make desktop-dev`, dropping a scanned-book folder shows the confirmation with its count and size; choosing Pack ends with one book packed, and choosing one-by-one adds loose files
