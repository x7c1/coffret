---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqE "\[.Files., fromTotals" frontend/packages/apps/web/src/'
assignee: null
branch: task/1106-0640-drop-files-from-the-library-details
created_at: 2026-11-06T06:40:27Z
updated_at: 2026-11-06T06:47:12Z
---

# fix(web): drop Files from the Library's details

## Overview

With nothing focused, the explorer's details pane describes the whole Library
(`TheLibrary` in `frontend/packages/apps/web/src/DetailsPane.tsx`). Its facts
include **Files** (`files_total` from `GET /api/library/totals`), and right
below it **On this device** already says the same total ("`0 of N files`" /
"`All N files`"). The count is shown twice.

Remove the **Files** fact from the Library's details. The Library's facts then
follow a folder's own order — Kind, Items, Size, Modified, On this device —
with the three facts only the Library has (Stored on, Objects on Storage, Size
on this device) where they are today. The comment above `TheLibrary` already
describes that set ("the facts a folder has, over the whole Library, with
three of its own"); keep it true.

Guards:

- `GET /api/library/totals` and its `files_total` field are unchanged: On this
  device still reads them, and the server, its spec and the API contract tests
  are not touched.
- The **Files** fact of several selected rows (`Several` in the same file) is
  a different fact — how many of the selected rows are files — and stays.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The Library's details no longer list Files: the web tests that read the
      Library's facts (`frontend/packages/apps/web/src/App.test.tsx`) expect
      the list without it, and no `'Files', fromTotals` fact remains under
      `frontend/packages/apps/web/src/`.
- [x] On this device in the Library's details still shows the Library-wide
      file count, under a test.
- [x] `make check` passes.
