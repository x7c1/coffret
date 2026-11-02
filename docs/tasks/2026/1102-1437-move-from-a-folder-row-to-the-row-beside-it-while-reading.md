---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1102-1437-move-from-a-folder-row-to-the-row-beside-it-while-reading
created_at: 2026-11-02T14:37:37Z
updated_at: 2026-11-02T15:12:08Z
---

# fix: while reading, move from a folder row to the row beside it, and off the first file to the rows above

## Overview

While the reader's sheet is open, `↑`/`↓` are answered by `readOn` in
`frontend/packages/apps/web/src/tree.ts`. Two of its answers surprise the
person reading, as seen in `make local`:

1. **From an open folder, `↓` jumps into a closed folder under it.** With
   `Books` open and focused, and its sub-folders closed, `↓` lands on the
   first image inside `A book whose title runs on far longer…`, opening that
   folder. The person expected the focus to move to the
   `A book whose title…` row itself. Today only a *closed* folder moves to
   the row drawn beside it; an open folder is treated like a file and reads
   on in reading order, asking the server through a closed folder.

2. **From the first file of the Library, `↑` does nothing.** With
   `Albums/album-000/img-00000.jpg` focused (both folders open, nothing
   above them), `↑` stays, because there is no file before it in reading
   order. The person expected the focus to move to the `album-000` row.

## The rule after this task

- **From a folder row, open or closed**, `↑`/`↓` go to the row drawn just
  above or below it, notes passed over, as a closed folder does today.
  `↓` from an open folder therefore lands on its first child row, whichever
  kind it is. Nothing is asked of the server from a folder row.
- **From a file row**, `↑`/`↓` keep reading on in reading order across
  folder boundaries, asking the server where a closed folder or a listing
  not on hand stands in the way, exactly as today.
- **Where the rows say there is no file further that way** (today's
  `stay` from the rows), the focus goes to the row drawn just above or below
  instead, if there is one; at the first and the last row drawn it stays.
  So `↑` from the first file reaches the folder rows above it, and from
  there the folder rule above takes over. The same holds for `↓` from the
  last file where folder rows are drawn below it.

The server-side answer (`getBeside`) is unchanged.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `readOn` follows the rule above, and its doc comment (and the
      `ReadOn` type's) states the rule as it now is.
- [x] Unit tests in `tree.test.ts` cover: `↓` and `↑` from an open folder
      (onto a closed child folder, onto a child file, from an empty open
      folder); `↑` from the first file with open folders above it; `↓` from
      the last file with rows drawn below it; the first and the last row
      drawn staying. Existing tests that assert the old open-folder
      behaviour are updated, not deleted.
- [x] Tests in `TreeTable.test.tsx` and the e2e journeys
      (`frontend/packages/apps/e2e/journeys/08-read-on.spec.ts`,
      `01-browse-and-read.spec.ts`) that relied on the old behaviour are
      updated, and a journey step covers `↓` from an open folder onto a
      closed child folder.
- [x] `docs/` states the new rule wherever it states how `↑`/`↓` move
      while reading.

### Before merge (verified outside the check command)

- [x] In `make local`, with the reader open, the person confirms: `↓` from
      the open `Books` row moves to the `A book whose title…` row, and `↑`
      from `Albums/album-000/img-00000.jpg` moves to the `album-000` row
      (needs a person).

## Out of scope

- Arrow keys while the sheet is closed.
- The fetch-ahead (`READ_ON_WITHIN`).
