---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/explorer-v0.2
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1030-0517-open-the-reader-as-a-sheet-that-follows-the-focus
created_at: 2026-10-29T20:17:20Z
updated_at: 2026-10-29T21:18:02Z
---

# feat(web): open the reader as a sheet that follows the tree's focus

## Overview

Part of the explorer redesign on the `feat/explorer-v0.2` integration branch.
The tree table and the details pane follow a focused row. Today the reader
(`ReaderView.tsx`) covers the whole screen and pages with its own keys.
Turn it into a **sheet**:

- Double-click, Enter, or the details pane's Open opens the reader as a dark
  sheet sliding over the right part of the screen. It leaves a fixed strip on
  the left (about 380px, never less than 64px) through which the tree stays
  visible. Vertically it covers exactly the tree's area: the header above and
  the Activity/status area below stay uncovered. Esc or the sheet's close
  button closes it.
- The sheet follows the tree's focus. Keys keep going to the tree: moving the
  focus with ↑/↓ changes what the sheet shows, so walking the page rows of an
  expanded book is paging. The reader no longer binds ← / → to paging: right-
  to-left and left-to-right books page in opposite directions and the arrows
  confused that. Remove the reader's own paging keys.
- What it shows for the focused row:
  - an image file: the image, filling the sheet;
  - a folder: its first image shown smaller, as a cover on a small stack of
    sheets, with the folder's name and `Folder · <n> items · <size>` under
    it — so a folder and the first file in it never look the same; a folder
    with no images shows the stack with "No images";
  - a file the reader cannot show: its name and "preview of this format is
    not supported yet".
- The sheet's header shows the containing path (small), the focused row's
  name with a folder or file icon, and a counter `<k> / <n>`: the focused
  row's position among its siblings, counting folders and files the reader
  cannot show alike, so it matches the parent's Items count and moves on
  every row the arrows land on.
- Opening a folder (double-click, Enter, Open) shows its cover and leaves the
  focus on the folder row. Today opening a folder of pages jumps to its first
  file, which moves the focus into the folder and so expands it as a side
  effect; that goes away — opening never expands or collapses a folder.
- Keep the existing prefetching of nearby images, now driven by the focus and
  its siblings rather than the reader's own index.

Out of scope: row menus, the add flow, the Activity panel (later tasks).

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it` and `make e2e-electron`, and in CI) must keep passing: when a change moves something a journey relies on (a role, a label, a row it counts), update the journey in this same change so it still proves what it proved.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test covers the counter: a folder among folders and files, an
  image among images and a non-image file, and the last sibling.
- [x] A unit test covers what the sheet shows for an image, a folder with
  images, a folder without images, and a non-image file.
- [x] A unit test asserts the reader binds no ArrowLeft/ArrowRight paging.
- [x] A unit test covers that moving the tree's focus while the sheet is open
  changes the shown item.
- [x] A unit test covers that opening a collapsed folder of pages leaves the
  focus on the folder and the folder collapsed.

### Before merge (verified outside the check command)

- [ ] On `make local`, the agent opens the sheet on a book, walks its pages
  with ↑/↓, steps onto the book folder and a sibling, and records in the PR
  what the sheet showed, including that the tree strip stays visible.
