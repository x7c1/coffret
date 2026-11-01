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
branch: task/1101-0250-show-the-librarys-size-on-this-device-and-always-give-the-file-total
created_at: 2026-11-01T04:13:02Z
updated_at: 2026-11-01T04:48:39Z
---

# feat: show the Library's size on this device, and always give the file total

## Overview

A person whose Library sits on Storage wants to know how much of this
device's disk it takes, so they can decide how much to clear locally. The
Library's details (no row focused) show "Cached for reading", which counts
only the held parcels: the files the person added, or fetched whole, are not
in it, so it reads 0 B on a device that holds the whole Library.

1. **Size on this device.** Replace the "Cached for reading" fact with
   "Size on this device", placed right after "On this device":

   | Label | Example | From |
   |---|---|---|
   | On this device | 3,210 of 12,480 files | unchanged count, new wording (item 2) |
   | Size on this device | 1.62 GB | the plaintext size of every file present on this device, plus the held parcels' files |

   Count it from the Index the way the other totals are counted, in the same
   pass over the catalog that counts `files_present`: the recorded size of
   each present Entry, never a walk or a stat of the mapped folders. The held
   parcels' part keeps its stat of each held parcel's file. Add the field to
   `GET /api/library/totals` and state it in `docs/spec/` beside the route's
   other fields; the held parcels' own field may stay or go as the code needs.
   Where the held parcels could not be counted, show the present files' part
   alone rather than nothing.

2. **Always give the file total.** "On this device" for the Library and for a
   folder says how many of the files are here, and always says how many there
   are:

   | Case | Wording |
   |---|---|
   | none here | `0 of 12,480 files` |
   | some here | `3,210 of 12,480 files` |
   | all here | `All 12,480 files` |
   | one file, here / not here | `1 of 1 file` / `0 of 1 file` |
   | no files at all | `No files` |

   This replaces `All files` and `No`. A file's own "On this device" is
   unchanged.

The values follow the Library as it changes (an add, a delete, a fetch, a
catch-up, a let go), as the other totals do. The end-to-end journeys
(`frontend/packages/apps/e2e/`, run by `make e2e-it` and `make e2e-electron`,
and in CI) must keep passing.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A server test covers the new size on a Library with present and remote
  files, inside and outside Packs, and held parcels: it is the present files'
  sizes plus the held parcels' bytes, and a remote file adds nothing.
- [x] The spec states the new field, and `make check`'s spec checks pass.
- [x] A web test covers "Size on this device" in the Library's facts, in
  order after "On this device", and the "Cached for reading" fact is gone.
- [x] A web test covers every case of the "On this device" wording in the
  table above, for the Library and for a folder.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent clears the focus and records in the PR the
  Library's details as drawn, and checks "Size on this device" against the
  sizes of the files in the mapped folder.
