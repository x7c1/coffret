---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/explorer-v0.2
perspectives: [completeness, clarity, user-experience, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1030-0807-add-files-with-the-plus-button-or-by-dropping-onto-a-row
created_at: 2026-10-29T23:07:43Z
updated_at: 2026-10-30T00:27:36Z
---

# feat(web): add files with the + button or by dropping onto a row

## Overview

Part of the explorer redesign on the `feat/explorer-v0.2` integration branch.
Rework how files come into the Library, keeping the drop pipeline the server
already has (receive → mapped folder → sync / freeze, PK-17 reservations):

- **Two ways in.** The header's **+** button (an icon button labelled "Add
  files") opens the operating system's file picker (files and, where the
  browser allows, folders); dropping files or folders onto the tree works as
  well. Both lead to the same confirmation. The destination is the focused
  folder — or the folder of the focused file — for +, and the row under the
  pointer (a folder, or a file's folder) for a drop. While dragging, the
  destination row is outlined and a pill at the bottom of the tree reads
  `Drop to add to <path>`.
- **A folder on this device, only when needed.** Adding needs a folder on
  this device for the destination's top-level folder or for the whole Library
  (EP-9). When neither exists, ask first, inside the flow: `Choose a folder on
  this device for “<top-level folder>”`, a short line saying files added are
  kept there as well as in the Library and that everything under that folder
  uses it, a path field with Browse… (the existing server-driven folder
  browser), and **Use this folder**, which records the mapping and continues
  to the confirmation. The word "map" is not shown.
- **The confirmation**, before anything is sent: `Add to <path>`, the total
  file count and size, the incoming items each marked `New folder`, `New`, or
  `Replaces`; for a drop holding a folder, a choice of how to store it — **One
  Pack** (default; "kept and fetched together, best for a book or an album")
  or **Separate files** ("each file kept on its own, best for files you will
  replace") — and the two numbering boxes (pages; volume folders) remembered
  from the last drop as today. Keep the existing one-line notes, each shown
  only when it applies: files the system keeps left out (`.DS_Store`, …),
  what the desktop app did not take, and files refused because a Pack holds
  their path.
- **The numbering preview lives inside the confirmation.** Lines that do not
  follow the rule — a name left unnumbered, a two-page spread, a name
  corrected by hand — are always shown and tinted; the regular lines between
  them fold into `… <n> more …`. A header line reads `<n> renamed · <m> to
  check` with **Show all**. Show all lists every line in the preview box and
  grows the dialog to the window's height (less a margin), the box taking the
  remaining height and scrolling inside. A new name can be clicked and
  corrected in place (Enter or blur takes it, Esc cancels, a corrected line
  offers Undo), with today's validation rules. Ticking or unticking a box
  starts again from the rule's names.
- **After Add**, the new rows appear in the tree at once with the uploading
  state, and the run shows in the status area as today (the Activity panel
  task replaces that area later).

- **One meaning for "on this device".** The tree's column already uses "On this device" for a file that is here. The banner that today says a folder "is not on this device — map …" means something else (no folder on this device is assigned to it). Reword that banner and every other place that says "map" to the person into the same terms as the new step (e.g. "No folder chosen for “<top-level folder>” yet — Choose a folder…"), so that "not on this device" is never said of a folder that merely has no folder chosen; "On this device" keeps meaning a file being here.

Out of scope: stopping an add (a separate change), the Activity panel.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it` and `make e2e-electron`, and in CI) must keep passing: when a change moves something a journey relies on (a role, a label, a row it counts), update the journey in this same change so it still proves what it proved.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test covers the destination: + on a focused folder, + on a focused
  file, a drop on a folder row, a drop on a file row.
- [x] A unit test covers when the folder-on-this-device step appears: no
  mapping for the top-level folder and no root mapping (asked), a top-level
  mapping (not asked), a root mapping only (not asked).
- [x] A unit test covers the folded preview: irregular lines always kept, the
  first and last regular lines kept, the rest folded with the right counts.
- [x] A unit test covers that ticking or unticking a numbering box discards
  corrections.
- [x] A unit test covers the incoming items' marks: New folder, New, Replaces.

### Before merge (verified outside the check command)

- [x] On the agent's own development Library, the agent adds a folder with +
  and by drop, sees the confirmation with the preview, corrects a name, and
  records in the PR what it did and saw, including the folder-on-this-device
  step (every top-level folder of the seeded Library already has a folder on
  this device, so the step is reached at the Library root; the per-folder
  wording is covered by a unit test).
