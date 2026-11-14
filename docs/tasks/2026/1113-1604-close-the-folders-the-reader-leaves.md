---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1113-1604-close-the-folders-the-reader-leaves
created_at: 2026-11-13T07:04:37Z
updated_at: 2026-11-13T07:24:53Z
---

# feat(web): close the folders the reader leaves behind as ↑/↓ read on out of them

## Overview

While the reader's sheet is open, `↑`/`↓` read on in reading order, across
folders (`readOn` in `frontend/packages/apps/web/src/tree.ts`, and
`readInside` while the tree is filtered). A folder the focus reads into is
opened so the focused row is on show, but a folder the focus reads out of
stays open. Reading through a series of volumes therefore leaves every volume
read so far open in the tree, and the tree grows longer with every book.

Change it so that when a move of `↑`/`↓` in the reader takes the focus out of
a folder, the folders it left are closed:

- Both directions: `↓` from the last file of a folder (or from its last row),
  and `↑` from the first file of a folder (or from its first row), whenever the
  row the focus lands on is outside that folder.
- Close every folder that holds the row the focus moves from and does not hold
  the row it lands on — the folder the focus stood in, and each of its
  ancestors up to (not including) the nearest folder that holds both rows. A
  folder that holds the row landed on stays open, and so does every folder
  outside that chain: only the folders left are closed.
- The same applies whether the rows answer the move or the server does
  (`getBeside`, `kind: 'ask'`), and whether or not the tree is filtered.
- A move that stays inside the folder closes nothing. A move with nowhere to
  go (`stay`) closes nothing. `↑`/`↓` with the reader closed, `→`, `←`,
  `Escape` and clicks are unchanged.
- Closing here is the ordinary closing of a folder, like `←`: decide, and say
  in the report, whether the closed folders forget the open state of folders
  inside them (as `Escape` does) or keep it (as `←` does), choosing what keeps
  a series read cover by cover short and predictable.

Update the doc comment of `readOn` (and of `readInside` where it describes the
same move) so that it states the new rule, and the e2e journey that reads on
across folders (`frontend/packages/apps/e2e/journeys/08-read-on.spec.ts`, and
`14-filter-read-on.spec.ts` for the filtered tree) so that it checks it. Add
unit tests beside the existing ones for `readOn` / `readInside`.

Do not change `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Unit tests show that `↓` from the last file of a folder and `↑` from the
      first file of a folder, with the reader open, close the folder left and
      every ancestor of it that does not hold the row landed on, and leave
      the folder that holds both rows, and folders outside the chain, open.
- [x] Unit tests show that a move inside one folder, and a move with nowhere
      to go, close nothing.
- [x] A journey shows, in headless Chromium, the folder read out of drawn
      closed after `↓` reads on into the next folder with the reader open,
      and again after `↑` reads back, in the unfiltered tree and in the
      filtered one.
- [x] `make check && make e2e-it` passes.
