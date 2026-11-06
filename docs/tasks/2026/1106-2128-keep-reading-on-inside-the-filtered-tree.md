---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1106-2128-keep-reading-on-inside-the-filtered-tree
created_at: 2026-11-06T21:28:21Z
updated_at: 2026-11-06T22:35:36Z
---

# feat(web): keep the reader's read-on inside the filtered tree

## Overview

The explorer's tree can be filtered by a name, by the most recent books, or
by both (`FilterShape` in `frontend/packages/apps/web/src/filter.ts`, drawn by
`treeRows` in `tree.ts`). While the reader's sheet is open, `↑`/`↓` read on
from file to file (`readOn` in `tree.ts`, `readOnward` in `App.tsx`). Where a
closed folder or a listing not on hand stands in the way, the explorer asks
the server for the file before or after in reading order (`getBeside`,
`GET /api/beside`, LA-32 in `docs/spec/loopback-access/README.md`). That
order is the whole Library's. So, while the tree is filtered, reading on can
land on a file the filtered tree does not show, and the tree then opens the
folders down to it.

Make reading on stay inside the filtered tree while a filter is on, and stop
at its ends.

**What is inside.** A file is inside the filtered tree when it is an answered
file, when it stands anywhere under an answered folder, or, where the Library
root is answered as a book (`rootBook`), when it stands directly in the root.
The dimmed ancestors are not themselves inside: a file directly in an ancestor
that is not an answered folder is not shown, and is not read.

**Reading on while filtered.**
- `↑`/`↓` from a file go to the file before or after it in reading order
  among the files inside the filtered tree, passing over folder rows as
  today, and opening the closed folders on the way down to it (as `landedOn`
  does today). A file outside the filtered tree is never landed on, and no
  folder is opened for one.
- `↑`/`↓` from a folder row go to the row drawn beside it, as today. Rows
  drawn are all inside or ancestors, so nothing changes there.
- At either end — no file inside the filtered tree further that way — the
  focus moves to the row drawn beside it where there is one, as today
  without a filter; where no row is drawn that way, the focus stays. It never
  wraps around and never leaves the filtered tree.
- Where the server is asked (a closed folder or a listing not on hand in the
  way), the answer counts only where it is inside the filtered tree. How to
  get the next file inside is a design decision for this change: asking again
  from an answer that fell outside, asking for the folder row in the way
  (`getBeside(folder, true)` answers the first file under it on `↓`), or a new
  optional parameter on `GET /api/beside` that limits the answer to under some
  paths. Choose the one that keeps the number of requests per key press small
  on a Library of about 100,000 files (a filter can match folders far apart in
  path order), and say why in a comment. If the server is changed, write the
  change into LA-32 (or a new LA rule next to it) and add route tests for it,
  and keep `/api/beside` without the new parameter answering exactly as today.
- An answer let go of today (the focus moved, the sheet closed, or the key
  pressed again) is still let go of.
- The filter turned off while the sheet is open: reading on goes back to the
  whole Library, as today.

**Unchanged.** Without a filter, reading on is exactly as today; the existing
tests in `tree.test.ts`, `App.test.tsx` and the journeys `08-read-on.spec.ts`
and `13-filter.spec.ts` keep passing unchanged, except where a test is
extended.

**States**, each covered by a test (unit tests under
`frontend/packages/apps/web/src/`, and route tests under
`backend/crates/apps/coffret-server/tests/routes/` if the server changes):
- `↓` from the last file of one answered folder to the first file of the next
  answered folder, closed, with an unmatched folder between them in path
  order: lands in the next answered folder and opens it;
- `↑` from the first file of an answered folder to the last file of the
  answered folder before it, closed;
- an answered file alone in its folder: `↑`/`↓` pass over its unshown
  siblings;
- the root as a book (Recent): its direct files are read, the files of a
  folder above another match are not;
- the last file inside the filtered tree with `↓`, and the first with `↑`:
  the focus goes to the row beside where one is drawn, and stays where none
  is;
- a request whose answer arrives after the key is pressed again, or after the
  filter is turned off: let go of;
- no filter: reading on as today.

**The journey.** Extend `frontend/packages/apps/e2e/journeys/13-filter.spec.ts`
(or add `14-filter-read-on.spec.ts`): filter by a name that matches two books
with an unmatched book between them in path order, open the first book in the
reader, press `↓` past its last page, and see the reader on the first page
of the second matched book, not the unmatched one; then press `↓` past the
last page of the last match and see the reader stay inside the filtered tree.

Out of scope: any change to what the filter matches (LA-40 to LA-42), and to
reading on without a filter.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Unit tests under `frontend/packages/apps/web/src/` cover every state
      listed in the Overview.
- [x] If `/api/beside` is changed: route tests cover the new parameter and the
      answer without it, and the spec states the change; `make check`'s spec
      gates pass.
- [x] The journey described in the Overview runs in `make e2e-it` and passes.
- [x] `make check` passes.
