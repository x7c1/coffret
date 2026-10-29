---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/explorer-v0.2
perspectives: [completeness, clarity, user-experience, spec-alignment, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1030-0638-act-on-rows-through-one-menu-and-modifier-key-selection
created_at: 2026-10-29T21:38:52Z
updated_at: 2026-10-29T22:45:18Z
---

# feat(web): act on rows through one menu and modifier-key selection

## Overview

Part of the explorer redesign on the `feat/explorer-v0.2` integration branch.
The tree table, details pane and reader sheet are in place. Rework how a
person acts on rows:

- **One actions menu**, opened by right-clicking a row or by a ⋮ button at the
  row's right end. The ⋮ shows only on the focused row and the row under the
  pointer. While the reader sheet covers the right part of the tree,
  right-click is the way in. The menu is a small popover at the pointer (or
  under the ⋮), closed by a click outside or Esc; the item under the pointer
  or keyboard focus is shaded, the destructive item in light red.
- Items depend on the row: **Open** (when the reader can show it), **New
  folder…** (folders), **Pack this folder…** (folders, only under a top-level
  folder this device has a folder for, as today), then a separator and
  **Delete…** — or, for a prepared folder that is not in the Library yet,
  **Forget this folder** instead of Delete…. Delete is not bound to any key:
  the key differs between systems and invites mistakes.
- **Multi-select with modifier keys**, like a desktop file manager:
  Shift+click selects the range from the anchor, ⌘/Ctrl+click adds or
  removes one row, Shift+↑/↓ grows the range. A plain click or a plain arrow
  move drops the selection. Selected rows are lightly tinted, the focused row
  keeps its strong highlight. Remove the old "Select" mode and the row
  checkboxes.
- Right-clicking a row inside a selection of several acts on the whole
  selection (its menu offers `Delete <n> items…`); right-clicking any other
  row first makes it the only focus. The details pane shows the selection
  summary with a `Delete <n> items…` button.
- Restyle the three dialogs to match the new screen while keeping what they
  say today:
  - **Delete**: `Delete “<name>”?` (or `Delete <n> items?`), how many files and
    how much leave the Library, and — only when Packs holding other files must
    be rebuilt — a highlighted note `<n> Pack(s) holding other files will be
    rebuilt` with `Reads <x>, writes <y>`. Keep the existing lines for refused
    Packs, files not in the Library and a deletion that waits for another, each
    shown only when it applies, and `This cannot be undone from the explorer.`
    The initial focus is Cancel.
  - **New folder…**: `New folder in <path>`, one Name field, and `Nothing is
    created until you add files to it.` A name the Library would refuse (empty,
    containing `/`, `.` or `..`, or one already used in that folder) is refused
    in the dialog with the reason under the field. Creating focuses the new
    prepared row.
  - **Pack this folder…**: `Pack “<name>”?`, how many files and how much will
    be packed, a "Left as they are" box listing what is skipped and why
    (already in Packs, not on this device, not in the Library yet), and one
    line explaining that a Pack keeps a folder's files together so a book or an
    album is fetched in one go. With nothing to pack it says why and offers
    only Close. Keep the existing wait line when a freeze is already running.

Out of scope: the add flow and the Activity panel (later tasks).

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it` and `make e2e-electron`, and in CI) must keep passing: when a change moves something a journey relies on (a role, a label, a row it counts), update the journey in this same change so it still proves what it proved.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test covers selection: Shift+click range from the anchor,
  ⌘/Ctrl+click toggling, Shift+↑/↓ growing, and a plain click or arrow
  dropping the selection.
- [x] A unit test covers which menu items each row kind gets: image, folder
  with images, folder under an unmapped top-level folder, prepared folder,
  non-image file, and a multi-row selection.
- [x] A unit test covers that right-clicking inside a selection targets the
  selection and right-clicking outside it targets that row alone.
- [x] A unit test covers New folder name validation: empty, `/`, `.`, `..`,
  a duplicate, and a valid name.
- [x] A unit test covers the Delete dialog's rebuild note: present when a Pack
  keeps other files, absent when whole Packs or one-file Containers go.

### Before merge (verified outside the check command)

- [ ] On the agent's own development Library, the agent opens the menu by
  right-click and by ⋮, selects several rows with Shift and ⌘/Ctrl, opens each
  dialog, and records in the PR what it saw. It does not confirm a deletion on
  a Library the user owns.
