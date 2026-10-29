---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/explorer-v0.2
perspectives: [completeness, clarity, user-experience, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1030-0301-add-a-details-pane-that-follows-the-focused-row
created_at: 2026-10-29T18:01:34Z
updated_at: 2026-10-29T19:03:53Z
---

# feat(web): add a details pane that follows the focused row

## Overview

Part of the explorer redesign on the `feat/explorer-v0.2` integration branch.
The middle of the screen is already one tree table (Name | Size | on this
device | Modified) with a focused row driven by the arrow keys. Add a
**details pane** on the right of the tree that always shows the focused row:

- A preview area at the top: for an image file, the image; for a folder that
  holds images, its first image (for a scanned book that is nearly always the
  cover); for another file, a plain placeholder showing its extension; for a
  folder with no images, a placeholder with its item count.
- Below it: the containing path (small), the name (heading), and an **Open**
  button when the row can be opened in the reader (an image, or a folder with
  images). Open does what Enter does.
- Then a two-column list of facts. Words are chosen to fit any kind of file,
  not only books: **Kind** (`Folder`, or the file's kind the way the macOS
  Finder words it — `JPEG image`, `PDF document`, `Plain text`,
  `QuickTime movie`, falling back to `Document`), **Items** (folders only: the
  number of direct children), **Size**, **Modified** (date and time to the
  minute), **On this device**. On this device reads `Yes` / `No` /
  `Fetching…` / `Yes · uploading` / `Yes · upload failed` for a file, and
  `All files` / `<n> of <m> files` / `No` for a folder, counting every file
  at every depth below it. The server answers the folder counts.
- A prepared folder (made with "New folder…" and still empty, so not yet in
  the Library) shows `Not in the Library until a file is added`.
- When several rows are selected (a later task adds modifier-key selection;
  model the pane on a selection list so it can), the pane switches to a
  summary: a stacked-sheets illustration with the count, `<n> items selected`,
  and the totals (folders, files, size). Until multi-select exists this state
  is reachable only from tests.
- The pane is a fixed-width column (about 320px) on wide windows; it may hide
  below a narrow width, but the tree stays usable.

Out of scope: the reader sheet, row menus, multi-select input, the add flow,
the Activity panel (later tasks on the branch).

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it` and in CI) must keep passing: when a change moves something a journey relies on (a role, a label, a row it counts), update the journey in this same change so it still proves what it proved.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test maps a file's extension to its Kind for jpg/jpeg, png, pdf,
  txt, mov and an unknown extension.
- [x] A unit test covers the folder "On this device" wording for all present,
  some present (`18 of 196 files`), and none present.
- [x] A unit test covers which rows offer Open: an image, a folder with images,
  a folder without images, a non-image file.
- [x] A unit test covers the selection summary's totals for a mixed selection
  of folders and files.
- [x] A backend test covers the per-folder present/total file counts at every
  depth.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent moves the focus through folders, images and a
  non-image file and records in the PR what the pane showed for each.
