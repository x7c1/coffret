---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 2
retries_remaining: 1
check_command: "make check && ! grep -rn --exclude='*.test.ts' --exclude='*.test.tsx' -e '◈' -e '▣' -e '▤' frontend/packages/apps/web/src"
assignee: null
branch: task/1016-1943-settle-the-explorers-marks-and-open-by-double-click
created_at: 2026-10-16T10:43:54Z
updated_at: 2026-10-16T11:46:42Z
---

# feat(explorer): replace the placeholder marks, select rows by click and open them by double-click, and delete from the tree

## Overview

The explorer's list and tree (`frontend/packages/apps/web/src/`) still carry
the characters the first implementation used as stand-ins. Some do not say what
they mean, and one is cut off. Rows also open on a single click, which leaves no
way to select one without opening it. Settle both.

**Marks and chrome**

| Where | Now | Make it |
|---|---|---|
| Tree root | `◈` (`FolderTree.tsx`) | No mark. (Showing the Library's display name is later work.) |
| Tree rows | `·` for a folder without subfolders, `▸` / `▾` for one with them | Drop `·`; rows without a disclosure arrow keep the same indent as rows with one |
| Tree rows of a book (a folder holding pages) | No way to delete it | A `⋮` menu on the row, with the same delete the list's folder `⋮` offers |
| List rows for a folder | `▸`, which reads as an expand arrow but navigates | A folder icon (inline SVG, both themes) |
| List mark column | 28px with 10px padding each side; `▸` does not fit and renders as an ellipsis | Wide enough for the icon, no ellipsis |
| List rows for a file | `▣` for an openable file, `▤` for one that is not | No mark. Keep dimming the files that cannot be opened |
| "Disconnect this tab" bottom right | Always shown | Remove it. Disconnecting stays with the desktop app's tray (all tabs) |
| Bottom edge "books — on drive" | Library name and the Storage's internal name | The Library name alone |
| Multi-select checkboxes | A delete checkbox on every row, always, and a select-all | Shown only after entering a selection mode with a "Select" control; the select-all likewise. Leaving the mode clears the selection |

Deleting a whole book is done from a folder row's `⋮` (list and tree). Deleting
several files stays in the selection mode.

**Select and open**

- A click on a list row selects it (highlighted, one at a time outside the
  selection mode). A double-click, or Enter on the selected row, opens it — a
  folder navigates into it, a file opens the reader. Same for files and folders.
- On touch, a single tap opens (`PointerEvent.pointerType === 'touch'`), since
  double-tap is not a reliable gesture there.
- Arrow keys move the selection within the list. The tree on the left keeps
  navigating on a single click.
- The selection is not a detail pane; nothing else changes on select.

**A message shown twice**

A tab with no session shows the host's refusal ("This explorer holds no
session…") and, below it, the page's own guidance ("This tab holds no session
with the explorer…"), which say the same thing (`Explorer.tsx`). Show one.

Update the e2e journeys under `frontend/packages/apps/e2e/journeys/` that click
a row to open it, so they double-click, and any that read the removed text.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Component tests show the tree and list without `◈`, `·`, `▣`, `▤`, and
      with a folder icon on folder rows; no non-test source under
      `frontend/packages/apps/web/src` contains `◈`, `▣` or `▤` (a grep
      appended to the check).
- [x] A component test shows a book row in the tree offering `⋮` → delete,
      which opens the same confirmation the list's folder delete opens.
- [x] Component tests show no row checkbox outside the selection mode, and
      that files chosen in the selection mode are deleted together.
- [x] Component tests show a click selects without opening, a double-click
      and Enter open, and a touch tap opens.
- [x] "Disconnect this tab" is gone and the bottom edge shows the Library name
      alone (component tests).
- [x] A tab without a session shows one message, not two (component test).

### Before merge (verified outside the check command)

- [x] In Playwright's Chromium against the agent's Library: the mark column
      shows no ellipsis, a double-click opens a book, a click only selects,
      and deleting a book from the tree's `⋮` works.
- [ ] The marks, icon and selection highlight look right — needs a person:
      a visual judgement.
