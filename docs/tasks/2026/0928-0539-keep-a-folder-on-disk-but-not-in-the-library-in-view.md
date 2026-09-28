---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && ! grep -rqE "freeze\.status === .done. \? \[\]" --exclude-dir=node_modules --exclude-dir=dist frontend/packages/apps/web/src/'
assignee: null
branch: task/0928-0539-keep-a-folder-on-disk-but-not-in-the-library-in-view
created_at: 2026-09-28T05:39:32Z
updated_at: 2026-09-28T07:24:40Z
---

# fix(web): keep a folder that is on disk but not in the Library in view, and never make a new folder over one

## Overview

The explorer has a folder a person made and filled that the Library does not hold yet — a new folder whose freeze stopped, or ended with nothing committed, or a folder already sitting in a mapped root that no run has carried in. The rule for such a folder is that the explorer neither loses it nor builds on top of it unawares. Three gaps break that rule.

1. **A freeze that ended `done` with nothing committed loses its folder on reload.** `strandedFolders` (`frontend/packages/apps/web/src/newFolder.ts`, around line 184) holds a folder back only when `freeze.status !== 'done'`, on the assumption (the comment around lines 171-173) that `done` means committed. A freeze whose files all ended as findings is `done` with `packs === 0`: its pages are on disk and outside the Library, and after a reload nothing on screen points at the folder, so the person can neither walk into it nor pack it again. Hold the folder back when the freeze committed nothing — `freeze.status !== 'done' || freeze.packs === 0` — and correct the comment. The `!folders.includes(folder)` filter after it already drops a folder once the Library names it. In `newFolder.test.ts`, change the `done` → `[]` case (around line 188) to one with `packs > 0`, and add the `done`, `packs: 0`, not-in-`folders` case.
2. **The reload path has no end-to-end coverage.** Whether a stranded folder comes back after a reload depends on the work answer and `App`'s wiring together, which the unit tests do not see. Extend journey 07 (`frontend/packages/apps/e2e/journeys/07-freeze.spec.ts`; it reloads once today, around line 94) with a step that stops a freeze — the way journey 04 stops Storage — reloads, sees the folder back in the tree dimmed, then retries and sees it commit. The work answer's field is `discarded` now (it was `dropped`).
3. **A new folder can be made over a folder that exists only on disk.** The name check for a new folder (`frontend/packages/apps/web/src/App.tsx`, around lines 407-411) looks only at the catalog's folders and the pending ones. `GET /api/folders` answers from the catalog (`coffret-server/src/routes/folders.rs`), so a folder already in a mapped root with files no run has carried in looks free. Dropping a book into the "new" folder then freezes with `freeze=true`, and the freeze selects every eligible file under the folder (PK-1, PK-17): the files that were already there go into the book's Pack silently.

   Decision for this task: the explorer asks before it makes the folder. The listing of a path already reports files on disk that are not in the Library (the `added` state), so list the candidate path and refuse the name, with a notice saying a folder of that name is already in the mapped folder, when the listing shows anything there. No new route and no wire change are needed if the listing answers for a path the catalog does not know; if it does not, add the smallest server answer that does and say so in the PR. A refusal at `freeze=true` upload time was considered and not taken: the first drop into a new folder creates it on disk, so a second drop into the same pending folder, or a "pack again" into a stranded one, would be refused as well. The window between the check and the drop stays open; the explorer is a single person's screen and the check is about not surprising them, not about racing another writer.

Guard: the stranded-folder rule for `stopped` and `freezing` freezes keeps working as it does now; item 1 widens it, it does not replace it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `strandedFolders` holds back a `done` freeze with `packs === 0` and drops a `done` freeze with `packs > 0`, pinned in `newFolder.test.ts` (`make check`, and a grep gate that the `done`-only condition is gone)
- [x] Journey 07 stops a freeze, reloads, finds the folder back in the tree, retries and sees it commit (`make e2e-it`)
- [x] Making a new folder whose name a mapped folder already holds on disk is refused with a notice, pinned in a unit test of the new-folder flow (`make check`)
- [x] `make check` and `make e2e-it` pass
