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
branch: task/1031-0627-move-rows-by-dragging-them-onto-a-folder-and-rename-or-move-them-from-the-row-menu
created_at: 2026-10-30T21:27:49Z
updated_at: 2026-10-30T22:16:27Z
---

# feat(web): move rows by dragging them onto a folder, and rename or move them from the row menu

## Overview

The server can now preview and carry out a move of files and folders — several
`(from, to)` pairs in one Journal record — and a device's files follow the
move where they can and leave the device where they cannot (see the move
routes in `docs/spec/` and the gateway's typed calls). Build the explorer's
side.

**Dragging rows.**

1. Rows of the tree can be dragged: a file, a folder, or a selection. Grabbing
   a row inside the selection drags the whole selection; grabbing a row outside
   it drags that row alone (the same rule as the right-click). When the
   selection holds a folder and rows under it, only the folder moves; the rest
   goes with it.
2. Where it lands: a folder row is the destination; a file row means the folder
   that file stands in (as a drop of files from outside does). The Library
   root has no row: the Library's name in the header takes the drop, and while
   it is over it says `Move to Library`.
3. While a drag from inside the tree is in the air, the landing row is
   outlined as for an add, and the pill at the bottom of the tree says
   `Move to <folder>` (an add from outside keeps `Drop to add to <folder>`).
4. Nothing happens over the folder the rows are already in, over a dragged row
   itself, or over anything under a dragged folder: no outline, no pill, no
   confirmation.
5. A closed folder the drag rests over is not opened. To move deeper, the
   person opens the folder first.
6. Letting go asks for a small confirmation before anything is written:
   `Move “Vol. 03” to “Series A”?` (or `Move <n> items to …`) with the count.
   The first focus is Cancel. The confirmation is built from the server's
   preview:
   - where some of this device's files cannot follow and will leave it, it
     says how many and their size, for example `196 files (1.2 GB) on this
     device will be removed from it; opening them fetches them again`;
   - where the destination already holds a name being moved, or the preview
     refuses the move (an edited file, an Entry still uploading), it says why
     and offers Cancel alone — nothing is overwritten or merged.
7. On confirmation the move is sent; the tree shows the rows at their new place
   once the server answers, and the move appears in the Activity panel.
   A prepared folder (made with "New folder…", not in the Library yet) is a
   valid destination; moving into it makes it an ordinary folder.

**The row menu.** Add two items to the menu a row's right-click and ⋮ open
(below Open / New folder… / Pack this folder…, above the separator before
Delete…):

- **Rename…** — a dialog like "New folder…": one name field, the current name
  filled in and selected. A name that cannot be used (empty, holding `/`, `.`
  or `..`, or the name of another row in the same folder) is refused with the
  reason under the field. Where renaming a folder would leave some of this
  device's files behind, the same dialog says how many and their size, as the
  move confirmation does. Committed through the same move call.
- **Move to…** — a dialog holding a tree of folders alone, with a `Library` row
  at the top so the root can be chosen; choosing one goes on to the same
  confirmation a drag's release opens.

Both act on the selection under the same rule as the right-click. Neither is
offered on a prepared folder (it has nothing in the Library to rename).

Since "Rename…" asks the move route for one rename, extend the Library concept doc's *rename* collocation (`docs/concepts/library/README.md`) to name the explorer's "Rename…" beside "Move to…" and a drag as what asks for a move.

Keep every colour going through `theme.ts`. Write code and comments so they do
not tie the explorer to one kind of content.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it`
and `make e2e-electron`, and in CI) must keep passing; add a journey for a drag
move and one for Rename….

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Unit tests cover where a drag lands (folder row, file row, header
  `Library`), the places where nothing happens, and which rows a drag of a
  selection moves.
- [x] Unit tests cover the confirmation's lines from a preview: a plain move,
  files leaving the device, and a refused move offering Cancel alone.
- [x] Unit tests cover Rename…'s refusals and Move to…'s folder tree.
- [x] An e2e journey drags a folder onto another folder, confirms, and finds it
  at its new path; another renames a folder from the row menu.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent drags a folder within a top-level folder, a
  selection of files onto a folder, a folder onto the header's Library, and
  a folder into a prepared folder; renames one; moves one with Move to…; and
  records in the PR what each confirmation said and where the rows ended up.
