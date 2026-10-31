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
branch: task/1101-0150-fetch-one-cover-for-a-book-not-on-this-device-and-page-through-collapsed-folders
created_at: 2026-10-31T17:58:17Z
updated_at: 2026-10-31T19:05:09Z
---

# feat: fetch one cover for a book not on this device, and page through collapsed folders with the reader's arrows

## Overview

1. **One cover, without a fill.** When the reader's sheet (or the details
   pane) shows a folder whose first image is not on this device, it peeks and
   says "Cover not on this device yet": a peek never fetches, so that passing
   over a folder does not start fetching the whole book. Add a way to fetch
   that one file without starting a fill (for example a `fill=false` form of
   the file route, or a route of its own — pick what fits the server's routes
   and say why in the spec). The parcel read for it is held like any other
   read (PK-19/PK-20), so a later fill or read of the same pages asks Storage
   for nothing more. What such a read finds that a fill would have reported
   (KL-15, PK-21 findings) goes into the `findings` of the `/api/work`
   response, as for any other work, so nothing is reported in silence. Write
   the rule into `docs/spec/` beside the file route's rules. The unmapped
   case is unchanged: a folder this device has not mapped refuses as today.
   The sheet and the details pane use it for a folder's cover; while it is in
   flight they show the cover as loading rather than as missing.

2. **The reader's ↑↓ from a collapsed folder.** Today ↑↓ in the reader move
   to the next and previous file in reading order, entering folders. Change
   this only when the focused row is a **collapsed folder**: then ↑↓ move to
   the adjacent drawn row of the tree — a folder or a file, whichever is
   drawn next — and never enter the collapsed folder. When the focused row is
   a file or an expanded folder, ↑↓ keep moving in reading order as today.
   For example, with collapsed `Vol. 1`, `Vol. 2`, `Vol. 3` side by side, ↓
   pages through their covers; with an expanded folder above `Vol. 1`, ↑ from
   `Vol. 1` lands on that folder's last drawn file. At the first and last
   drawn rows the arrows do nothing. The folder ahead's cover is fetched with
   the one-cover route of item 1, and the fetch-ahead of the next folder
   (which already exists for reading on across a folder boundary) keeps
   working for files.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it`
and `make e2e-electron`, and in CI) must keep passing.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A server test covers fetching one file without a fill: the bytes come
  back, the parcel is held, no fill is recorded or started, and a finding the
  read makes reaches the work response's findings.
- [x] A server test covers an unmapped folder still refusing.
- [x] The spec states the new rule, and `make check`'s spec citation and rule
  id checks pass.
- [x] A web test covers the sheet showing a cover fetched this way for a
  folder whose first image is remote, and the loading state before it lands.
- [x] Web tests cover ↑↓ from a collapsed folder: to a collapsed sibling, to a
  file drawn after the folders, ↑ to the last drawn file of an expanded folder
  above, nothing at either end; and ↑↓ from a file and from an expanded
  folder unchanged.

### Before merge (verified outside the check command)

- [x] On `make local` with a second device (or a Library whose pages are not
  all on this device), the agent pages through collapsed folders with ↓ and
  records in the PR that each cover appears and that no fill starts.
