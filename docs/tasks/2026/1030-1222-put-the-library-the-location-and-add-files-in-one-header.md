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
branch: task/1030-1222-put-the-library-the-location-and-add-files-in-one-header
created_at: 2026-10-30T03:22:06Z
updated_at: 2026-10-30T04:14:33Z
---

# feat(web): put the Library, the location and Add files in one header

## Overview

Part of the explorer redesign on the `feat/explorer-v0.2` integration branch.
The tree, the details pane, the reader sheet, the actions menu, the add flow
and the Activity panel are in place, but the screen has no header: the
Library's name sits at the right end of the Activity panel's row, the bar over
the tree shows only the focused folder's path, and the **+** (Add files)
button lives in that bar. Give the screen one header row above the tree and the details pane:

- **Left — the Library.** A button-like element showing the open Library's
  name, with no icon in front of it (a lock icon would read as "locked"). It
  is where switching Libraries will live; while the server serves one Library
  it shows the name and nothing to switch to, so do not draw a dropdown arrow
  that opens nothing. Remove the Library name from the Activity panel's row
  if it is only a label there.
- **Middle — the location.** The focused row's containing path and name,
  `Comics / Series A / Vol. 03`, truncated with an ellipsis when it does not
  fit. It follows the focus like the details pane does.
- **Right — Add files.** Move the **+** icon button (labelled "Add files",
  with a tooltip) and its two-item menu (Files… / A folder…) here from the bar
  over the tree, keeping its behaviour. The bar over the tree keeps what only
  it can offer (New folder… for the Library root, which has no row; Pack this
  folder… is never offered at the root) or goes away if nothing is left in it.
- The header stays one row on wide windows and wraps without horizontal
  scrolling on narrow ones; the tree and the details pane fill the height
  between the header and the Activity panel.

Out of scope: a search box (what it searches and how it shows results is not
designed yet).

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it` and `make e2e-electron`, and in CI) must keep passing: when a change moves something a journey relies on (a role, a label, a row it counts), update the journey in this same change so it still proves what it proved.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test covers the location text for a top-level folder, a nested
  file, and the Library root, and that it follows the focus.
- [x] A unit test covers that the header's Add files button opens the
  Files… / A folder… menu and that its destination is the focused folder.
- [x] A unit test covers that the Library name appears in the header without
  an icon before it.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent checks the header at a wide and a narrow
  window width, moves the focus and watches the location follow, and adds a
  file through the header's + — and records what it saw in the PR.
