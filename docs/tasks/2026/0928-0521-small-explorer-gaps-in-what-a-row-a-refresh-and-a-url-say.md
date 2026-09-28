---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqE "#(202020|12222a|2a2413)" --exclude=theme.ts --exclude-dir=node_modules --exclude-dir=dist frontend/packages/apps/web/src/ && grep -rqF "normalize(" --exclude-dir=node_modules --exclude-dir=dist frontend/packages/apps/web/src/hash.ts && ! grep -rqE "return .the Library changed.;" --exclude-dir=node_modules --exclude-dir=dist frontend/packages/apps/web/src/'
assignee: null
branch: task/0928-0521-small-explorer-gaps-in-what-a-row-a-refresh-and-a-url-say
created_at: 2026-09-28T05:21:33Z
updated_at: 2026-09-28T05:33:29Z
---

# fix(web): small explorer gaps in what a row, a refresh and a restored URL say

## Overview

Five small gaps in the explorer, each in what the screen says or where it looks. None needs a decision made on a real device; the ones that do (the reader's edges, dismissing a notice, the hover colour of a selected row, telling repeated refreshes apart) are deliberately left out of this change.

1. **An added row's tooltip no longer says what happens next.** `rowFill` (`frontend/packages/apps/web/src/fill.ts`, around lines 87-91) answers an `added` file with "this file is in the folder and not in the Library". Every file in that state is picked up by the next sync or freeze of its folder — the scan sees it as new, whether it was never in the Library or another device removed its Container — so the sentence should say it is not in the Library *yet* and what carries it in. A file whose name a run refuses is not carried in; phrase the sentence so it does not promise that (for example by saying the run takes it in or says why not). Pin the sentence in `fill.test.ts` (around lines 348-355 only checks it is non-null today). The comment in `FolderTree.tsx` (around lines 19 and 214) that still says "not in the Library yet" should agree with the result.
2. **A hand-typed URL hash in NFD is not normalised.** `parseHash` (`frontend/packages/apps/web/src/hash.ts`, around lines 27-34) returns `path` and `open` as typed. Entry Paths are NFC (EP-1), and the comparisons that use the parsed state — the stale-folder correction (`App.tsx`, around line 550) and the refetch after a fill (around line 277) — compare against NFC paths, so an NFD hash matches nothing. `newFolder.ts:74` already normalises a folder name to NFC. Normalise both values to NFC in `parseHash`, and add NFD cases to `hash.test.ts`. Whether the address bar is rewritten to the normalised form is this task's call; say which in the PR.
3. **"the Library changed" counts nothing.** `refreshedLine` (`frontend/packages/apps/web/src/refresh.ts`, around lines 77-89) falls back to "the Library changed" when the catalog advanced and gained nothing, while the refresh answer carries `entries`, the Library's current Entry count (`gateway/api/src/refresh.ts`, around line 25; the server fills it in `coffret-server/src/routes/refresh.rs`, around lines 32 and 40), which nothing on the page reads. Use it in that line (for example "the Library changed — it now holds N files"), and pin the sentence in the refresh tests.
4. **The folder tree does not scroll to the current folder.** Opening a deep folder from a URL leaves the tree's marked row below the fold. `FileList.tsx` (around lines 410-415) already brings its marked row into view with `scrollIntoView({ block: 'nearest' })`; do the same for the tree's current row in `FolderTree.tsx` (the row component, around lines 160-225).
5. **Colours in `FileList` bypass `COLOR`.** `FileList.tsx` spells colours inline (around lines 371 `#202020`, 520 / 539 / 550 `#12222a`, 601 `#2a2413`) instead of naming them in `COLOR` (`theme.ts`). Name each in `COLOR` by what it is for and use the names. The selected row's background already comes from `COLOR.selected`; how it looks under hover is out of scope here.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `fill.test.ts` pins the added row's sentence, which says the file is not in the Library yet and what carries it in (`make check`)
- [x] `parseHash` normalises `path` and `open` to NFC, pinned by NFD cases in `hash.test.ts` (`make check`, and a grep gate for `normalize(` in `hash.ts`)
- [x] The advanced-but-nothing-gained refresh line uses `entries`, pinned in the refresh tests (`make check`, and a grep gate that the bare "the Library changed" return is gone)
- [x] The folder tree scrolls its current row into view (a test in the folder-tree tests asserts `scrollIntoView` is called for the current row; `make check`)
- [x] No `#202020`, `#12222a` or `#2a2413` literal remains under `apps/web/src/` (grep gate)
- [x] `make check` passes
