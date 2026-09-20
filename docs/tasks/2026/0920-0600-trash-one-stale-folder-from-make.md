---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && bash -n scripts/drive-it-reset.sh && grep -q '^drive-it-trash:' Makefile && grep -q 'IDS' scripts/drive-it-reset.sh && grep -q 'drive-it-trash' scripts/drive-it-reset.sh && ! grep -Eq 'read -r?s' scripts/drive-it-reset.sh"
assignee: null
branch: task/0920-0600-trash-one-stale-folder-from-make
created_at: 2026-09-20T06:00:00Z
updated_at: 2026-09-20T07:45:00Z
---

# test(drive): trash one stale folder from make without resetting everything

## Overview

`make drive-it-list` (`scripts/drive-it-reset.sh list`) tells the app
folders under `COFFRET_DRIVE_FOLDER_ID` apart by whether a Library here
still opens them: the `IN USE BY` column names the scenario Libraries,
or says `stale`. What the listing does not give is a next step for one
stale folder. The only make target that trashes anything is
`make drive-it-reset`, which trashes every folder in the listing and
removes both targets' state — so a person who has just learned that
one folder is stale and two are in use has to choose between leaving
the stale one and losing the live ones (and the consents that go with
them). The tool underneath, `examples/app_folders.rs`, already trashes
by id (`trash <id>...`); nothing exposes that for one folder.

Add a third mode to the script and a target for it:

1. **`scripts/drive-it-reset.sh trash <id>...`**, reached as
   `make drive-it-trash IDS="<id> [<id>...]"`. It takes the listing the
   same way `list` and `reset` do (same grant under `.tmp/drive-admin/`,
   same skip when `COFFRET_DRIVE_FOLDER_ID` is unset, same
   `COFFRET_DRIVE_CLIENT_ID` check before the build), prints it, and
   then trashes only the ids given. Before trashing anything it refuses
   — with a line per id saying why, and exit 1 — any id that is **not in
   the listing** (a typo, or a folder under another parent, which this
   tool must never touch) and any id the listing marks as **in use**
   (say which Library opens it, and that `make drive-it-reset` is the
   way to give up a live Library). All-or-nothing: if any id is refused,
   nothing is trashed. Nothing under `.tmp/` is removed in this mode;
   the state belongs to Libraries that are still live.
2. **`IDS` empty or missing** is a usage error from the script (exit 1,
   naming the variable), and the Makefile target passes `$(IDS)` through
   unquoted so several ids can be given space-separated.
3. **The listing's closing advice** (the `Nothing was changed. A folder
   marked stale …` lines in `list` mode) now names both next steps:
   `make drive-it-trash IDS=<id>` for a stale folder, `make drive-it-reset`
   to start over. The `## drive-it-list:` and `## drive-it-reset:` blocks
   in the Makefile get one sentence each pointing at the new target, and
   a `## drive-it-trash:` block in the same style says what it refuses
   and why (the parent boundary, and that an in-use folder is only ever
   given up through the reset).
4. **The script header** describes the three modes.

Nothing here needs a new consent: the tool's grant is already cached
on a machine where `make drive-it-list` has run. The manual check can
be done entirely with refusals — the in-use folders on the account are
exactly what the target must refuse — and a stub for the trash path.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/drive-it-reset.sh trash <id>...` prints the listing, refuses ids not in it and ids in use (naming the Library), trashes nothing when any id is refused, and otherwise trashes exactly the given ids without touching `.tmp/`
- [x] `make drive-it-trash IDS="…"` runs it; an empty `IDS` is a usage error
- [x] The `list` mode's closing advice names `make drive-it-trash`, the Makefile has a `## drive-it-trash:` block, and the two existing blocks point at it
- [x] The script contains no `read -s`

### Manual / on-hardware (verified by a human before merge)

- [ ] On a machine where the targets have run, `make drive-it-trash IDS=<an in-use folder id>` refuses, naming the Library, and trashes nothing; `make drive-it-trash IDS=not-a-folder` refuses as not listed; both leave `.tmp/` as it was

## Out of scope

- Any change to what `make drive-it-reset` does
- Warning about local Libraries that point at folders outside the listing (a separate follow-up)
- Refusing `root` in the tool's `trash` subcommand (a separate follow-up)
