---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/explorer-v0.2
perspectives: [completeness, clarity, user-experience, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq "<FolderTree" frontend/packages/apps/web/src/'
assignee: null
branch: task/1030-0105-merge-the-folder-tree-and-file-list-into-one-tree
created_at: 2026-10-29T16:05:20Z
updated_at: 2026-10-29T17:36:37Z
---

# feat(web): merge the folder tree and the file list into one tree

## Overview

This is the first step of a redesign of the explorer, landing on the
`feat/explorer-v0.2` integration branch. Later steps on the same branch add a
details pane, turn the reader into a sheet, add row menus and multi-select,
rework adding files, and replace the status bar with an Activity panel. This
task lays the base they build on: the screen's middle becomes one tree.

Today the explorer shows a folder tree on the left (`FolderTree.tsx`) and the
open folder's files on the right (`FileList.tsx`, with Name / Size / Modified /
State columns). Replace the two with **one tree table**, like the list view of
the macOS Finder or the tree view of GNOME Files:

- Folders and files are rows of one table, indented by depth. A folder row has
  a disclosure button (a chevron) at its start; files and empty folders keep
  the space so names line up. Every file is a row, including each page of a
  scanned book.
- Columns, in order: **Name | Size | (on this device) | Modified**. Modified
  shows date and time to the minute (`2016-04-11 20:12`). There is no Items
  column. Headings are left-aligned; Size and Modified values do not wrap and
  Size is right-aligned. Each value column is as wide as the longest value
  among the rows on show or its heading, whichever is wider — measured from
  the rendered strings (e.g. `CanvasRenderingContext2D.measureText` with the
  row font), not estimated from character counts, so a left-aligned heading
  and the values under it span the same width.
- The "on this device" column is a narrow icon column right after Size, so
  whether a thing is here and how much room it takes are read together:

  | state of the row | shown |
  |---|---|
  | in the Library only (`remote`) | nothing |
  | on this device (`present`) | a laptop icon, labelled "On this device" |
  | being fetched or uploaded | a small progress ring, labelled accordingly |
  | failed | a red warning triangle, labelled with the failure |

  "On this device" covers files fetched here and files this device added
  itself, which is why it is not worded "downloaded". A **folder** shows the
  laptop icon only when every file at every depth below it is present on this
  device — the promise that the whole book or series reads offline — and shows
  nothing otherwise. The server computes this per folder from the Index (the
  browser does not hold every descendant), and the listing a folder row comes
  from carries it.
- Keyboard, on the tree: ↑/↓ move the focus to the previous/next visible row.
  → on a collapsed folder expands it; on an expanded folder moves to its first
  child. ← on an expanded folder collapses it; otherwise moves to the parent.
  Enter opens the focused row the way a double-click does today (the reader
  for a file it can show). ← and → never turn pages. The focused row is
  highlighted and is the row actions apply to.
- When the focus moves, the list scrolls ahead of it: the focused row is kept
  at least three rows away from the top and bottom edges of the scrolling
  area, the way a desktop file manager scrolls while the arrow keys walk a
  long list. Expanding or collapsing keeps the focused row in view.
- Mouse: a click on a row focuses it. A click on the disclosure button expands
  or collapses that folder without moving the focus — except that collapsing a
  folder that contains the focused row moves the focus to that folder, so the
  focus is never on a hidden row. A double-click on a row opens it like Enter
  (the reader for a folder or file it can show); a double-click does not
  expand or collapse.
- Expanding a folder loads its listing on demand through the existing
  per-folder listing route; folder rows that have not been expanded need no
  request beyond their parent's listing.

Keep what the two components do today that is not about layout — the
prospective (prepared) folders and their "Forget this folder" offer, the
mapping and "not on this device" states, the drop target, the per-row menu,
"Select" and delete — working in the new tree. They are reworked by later
tasks on the branch; this task only has to keep them reachable, not redesign
them.

Out of scope here (later tasks on the branch): the details pane, the reader as
a sheet, right-click menus and modifier-key multi-select, the add flow, the
Activity panel. Out of scope altogether: user-resizable columns.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test covers the keyboard model on a fixture tree: ↓/↑ across
  expanded and collapsed folders, → expanding then entering, ← collapsing then
  going to the parent, and no move past the first or last visible row.
- [x] A unit test covers collapsing a folder that contains the focused row: the
  focus moves to that folder; collapsing any other folder leaves the focus.
- [x] A unit test covers the scroll-ahead rule: given row positions and a
  viewport, the computed scroll keeps the focused row three rows from either
  edge, and does not scroll when it already is.
- [x] A backend test covers the per-folder "every file below is present"
  answer: all present, one remote at a deeper level, a failed file, and a
  folder with no files.
- [x] A unit test covers the column-width rule with an injected measuring
  function: the width is the larger of the heading and the widest value.
- [x] `FolderTree.tsx` is no longer rendered by the explorer screen.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent walks the seeded Library in a browser with the
  keyboard (↑ ↓ → ← Enter) and the mouse (disclosure button, double-click) and
  records in the PR what it did and saw, including the scroll-ahead on a
  folder with more rows than fit.
