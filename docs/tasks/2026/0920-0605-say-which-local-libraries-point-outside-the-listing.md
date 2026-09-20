---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && bash -n scripts/drive-it-reset.sh && grep -q 'NOT LISTED' scripts/drive-it-reset.sh && grep -q 'FORCE' scripts/drive-it-reset.sh && grep -q 'FORCE' Makefile && ! grep -Eq 'read -r?s' scripts/drive-it-reset.sh"
assignee: null
branch: task/0920-0605-say-which-local-libraries-point-outside-the-listing
created_at: 2026-09-20T06:05:00Z
updated_at: 2026-09-20T10:10:00Z
---

# test(drive): say which local Libraries point at folders the listing does not show

## Overview

`scripts/drive-it-reset.sh` pairs each folder under
`COFFRET_DRIVE_FOLDER_ID` with the local scenario Libraries whose
`settings.json` names it (`scenario_of`). The pairing only runs one way:
from the listing to the state. A Library under `.tmp/drive-round-trip/`
or `.tmp/drive-index-layout/` whose `folder_id` is **not** in the
listing — because `COFFRET_DRIVE_FOLDER_ID` was changed after the
Library was made, so its folder sits under the old parent — appears
nowhere. `make drive-it-list` says nothing about it, and
`make drive-it-reset` then removes that Library's state without having
trashed its folder, leaving a folder on the account that nothing points
at any more: the very "stale" state the tool exists to prevent, made by
the tool itself.

Make the other direction visible, and make the reset stop at it:

1. **A second section in the listing**, in every mode, after the folder
   table: `NOT LISTED` — one line per local Library whose `folder_id` is
   not among the listed ids, giving the scenario/Library name (the same
   `drive-round-trip/main` form `IN USE BY` uses) and the folder id it
   points at. Say in one line what it means: the folder is under another
   parent than `COFFRET_DRIVE_FOLDER_ID` names now, so this tool cannot
   see or trash it. Omit the section when there is none (the common
   case); do not print an empty heading.
2. **`reset` refuses while that section is non-empty**, before trashing
   anything: exit 1, with the section above as the reason and the two
   ways out — point `COFFRET_DRIVE_FOLDER_ID` back at the parent those
   Libraries were made under and reset there first, or run with
   `FORCE=1` to remove the state anyway and accept the orphaned folder.
   `FORCE=1` is an environment variable the Makefile passes through
   (`make drive-it-reset FORCE=1`); document it in the
   `## drive-it-reset:` block, including what it gives up.
3. **`trash` mode is unaffected** except for printing the section; it
   never touches state.
4. **The `## drive-it-list:` block** mentions the section in one
   sentence.

No new consent: the tool's grant is cached. On the machine this is
verified on, the section is expected to be empty; the non-empty path is
exercised against a stub.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The listing prints a `NOT LISTED` section naming each local Library whose `folder_id` is not in the listing, and omits it when there is none
- [x] `reset` refuses with exit 1 while the section is non-empty unless `FORCE=1`, and the Makefile passes `FORCE` through and documents it
- [x] The script contains no `read -s`

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-it-list` on a machine where the targets have run prints no `NOT LISTED` section (every local Library's folder is under the current parent)

## Out of scope

- Trashing folders under another parent (the tool only ever lists and trashes under `COFFRET_DRIVE_FOLDER_ID`)
- The single-folder `make drive-it-trash` target (its own task)
