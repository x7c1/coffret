---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rniE "escape[^a-z]{0,2} [^.]*climb" docs --include=*.md --exclude-dir=tasks'
assignee: null
branch: task/1106-0614-close-folders-on-the-way-up-with-escape
created_at: 2026-11-06T06:14:40Z
updated_at: 2026-11-06T06:39:16Z
---

# feat(web): close folders on the way up with Escape

## Overview

Today `Escape` on the explorer's tree climbs without closing anything: the
focus moves to the folder the focused row stands in (`climbOrLetGo` in
`frontend/packages/apps/web/src/TreeTable.tsx`), and from a top-level row it
lets go of the focus. The folders it climbs out of stay open, so after a few
presses the tree still shows every level that was open, and climbing is the
only thing `Escape` does to it.

Make each press of `Escape` fold one level of what is shown:

- **On an open folder's row**, `Escape` closes that folder. The focus stays
  on that row.
- **On a file's row, or a closed folder's row**, `Escape` closes the folder
  the row stands in (its parent) and moves the focus to that parent's row.
- **On a top-level row** that is a file or a closed folder (its parent is the
  Library root), `Escape` lets go of the focus, as today. A top-level folder
  that is open is closed first by the first rule; the next `Escape` lets go.
- A folder closed by `Escape` also forgets which of its subfolders were
  open: opening it again shows it with every subfolder closed. Closing a
  folder with `←` or with a click keeps remembering its subfolders' state, as
  today, so opening it again restores the shape it had.
- The selection is dropped whenever `Escape` moves the focus or lets go of it,
  as today.
- While the reader's sheet is open, `Escape` closes the sheet, as today
  (`escapeLetsGo` is false then), and does not fold or move anything in the
  tree.
- `←` and clicks are unchanged.

Text that describes `Escape` as climbing must be brought in line, for example
`docs/guides/environments.md` ("`Escape`, which climbs to the parent folder
and then lets go") and the comments next to `climbOrLetGo`. If a spec or
design document in this repository describes the tree's `Escape`, update it
to the behaviour above.

Out of scope: what any other key does, and the handling of keys that reach
`document.body` (a separate change). If that change has landed when this one
is written, the same behaviour applies to an `Escape` that reaches the tree
that way.

States `Escape` can arrive in, each covered by a test:

- an open folder's row (nested, and top-level): the folder closes, the focus
  stays;
- a file's row inside a folder: the parent closes, the focus moves to the
  parent's row, the selection is dropped;
- a closed folder's row inside a folder: same as a file's row;
- a top-level file or closed folder: the focus is let go of;
- repeated presses from a file three levels deep: each press folds one level,
  and the last lets go;
- a folder closed by `Escape` and opened again: its subfolders that were open
  show closed;
- a folder closed by `←` (and by a click) and opened again: its subfolders
  that were open show open again;
- the reader open: `Escape` does not change the tree's open folders or focus.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Web unit tests (`frontend/packages/apps/web/src/`) cover each state
      listed in the Overview, for the tree's state function and for the
      rendered tree.
- [x] Closing with `←` and with a click still restores the subfolders' open
      state when the folder is opened again, under a unit test.
- [x] No document under `docs/` (task files aside) still says that `Escape`
      "climbs" the tree.
- [x] `make check` passes.
