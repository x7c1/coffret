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
branch: task/1101-0200-describe-the-library-in-the-details-pane-when-no-row-is-focused
created_at: 2026-10-31T19:23:02Z
updated_at: 2026-10-31T20:04:27Z
---

# feat: describe the Library in the details pane when no row is focused

## Overview

When no row is focused, the details pane says "Nothing selected". Draw the
Library there instead, in the same layout as a folder's details:

- **Preview**: a box-shaped Library icon in place of a folder's mark, with
  "N items" under it (the Library's top-level rows).
- **Heading**: the Library's name. A Library with no name yet shows the same
  wording the header uses for it, in `dim`. No location line.
- **Facts**, in this order:

  | Label | Example | From |
  |---|---|---|
  | Kind | Library | fixed |
  | Stored on | Google Drive | the provider `GET /api/library` gives |
  | Objects on Storage | 214 | the number of objects the Library keeps on Storage (Containers and control objects counted as Storage counts them — say which in the spec) |
  | Items | 5 | the top-level rows |
  | Files | 12,480 | every file in the Library |
  | Size | 5.48 GB | the plaintext size of every file, as the tree's Size column sums them |
  | Modified | Oct 28, 2026, 21:14 | the newest modification under it |
  | On this device | 3,210 of 12,480 files | files present on this device, worded like a folder's |
  | Cached for reading | 1.4 GB | the total size of the parcels this device holds (Index concept, "held parcel") |

  The server does not give Objects on Storage, the Library-wide file counts,
  or the held parcels' total today. Add them to the server's answer the
  explorer already reads for the Library (or a route beside it), from the
  Index without reading Storage, and state them in `docs/spec/` beside the
  route's other fields.

- **Activity row**: drop the "N parcels kept, …" text from the Activity
  row's right end; Cached for reading replaces it. The `/api/work` field it
  came from may stay or go as the code needs.

The values follow the Library as it changes (an add, a delete, a fetch, a
catch-up) the way a folder's details do. The end-to-end journeys
(`frontend/packages/apps/e2e/`, run by `make e2e-it` and `make e2e-electron`,
and in CI) must keep passing.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A server test covers the new fields: the object count, the file
  counts (all and present), and the held parcels' total, on a Library with
  Packs, loose files, remote and present files, and held parcels.
- [x] The spec states the new fields, and `make check`'s spec checks pass.
- [x] A web test covers the Library's details with no row focused: every fact
  in order, a named and an unnamed Library, and a Google Drive and an S3
  provider.
- [x] A web test covers the Activity row no longer showing kept parcels.
- [x] A web test covers the details updating after a change to the Library.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent clears the focus and records in the PR the
  Library's details as drawn, in Light and Dark, and checks the counts against
  the CLI or the Index.
