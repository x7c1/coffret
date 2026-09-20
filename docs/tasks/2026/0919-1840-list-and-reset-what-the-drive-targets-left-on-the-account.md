---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && test -x scripts/drive-it-reset.sh && bash -n scripts/drive-it-reset.sh && grep -q '^drive-it-list:' Makefile && grep -q '^drive-it-reset:' Makefile && test -f backend/crates/gateway/google-drive-store/examples/app_folders.rs && grep -q 'createdTime' backend/crates/gateway/google-drive-store/examples/app_folders.rs && ! grep -Eq 'read -r?s' scripts/drive-it-reset.sh && grep -q 'drive-admin' scripts/drive-it-reset.sh"
assignee: null
branch: task/0919-1840-list-and-reset-what-the-drive-targets-left-on-the-account
created_at: 2026-09-19T18:40:00Z
updated_at: 2026-09-20T03:55:00Z
---

# test(drive): list and reset what the real-Drive targets left on the account

## Overview

The real-Drive targets — `make drive-round-trip-it` and
`make drive-index-layout-it` — each keep one Library, made by `init` on
the first run and reused after, because a grant belongs to a Library on
a device and a new Library would mean a new consent. Each Library is a
`coffret-<library id>` folder under the parent named by
`COFFRET_DRIVE_FOLDER_ID`, and the name says nothing about when it was
made or which target made it. Nothing trashes those folders: the
Makefile says removing them "is the account owner's to do", and a run
that failed inside `init` after Drive had minted the folder (the folder
is minted only once the consent has been answered) leaves a folder no
local state points at. Telling a live
Library from a stale one today means opening Drive, sorting by date, and
comparing ids with what `.tmp/drive-round-trip/state` and
`.tmp/drive-index-layout/state` say — by hand.

Give the account owner a listing and a reset, from `make`:

1. **A small tool that lists and trashes app folders under the parent.**
   An example binary in `google-drive-store`,
   `backend/crates/gateway/google-drive-store/examples/app_folders.rs`,
   in the mould of the existing `examples/authorize.rs` (same
   environment: `COFFRET_DRIVE_CLIENT_ID` / `_SECRET`,
   `COFFRET_DRIVE_TOKEN_CACHE`, `COFFRET_MASTER_KEY`; same logging and
   `require` helpers, copied or shared as reads best). Two subcommands:
   `list <parent id>` prints one line per live child folder of the
   parent whose name starts with `coffret-` — id, name, `createdTime`,
   tab-separated, oldest first — and `trash <id>...` trashes each given
   folder (Drive's trash, not a delete: a folder that turns out to have
   mattered is recoverable for a while). Use the crate's own pieces
   (`DriveApi`, `Endpoints`, `authorization`, `FailedResponse`, the
   `AccessTokens` / `OAuthTokens` / `TokenCache` set) rather than a
   second HTTP layer; the `files` list needs a `q` of the parent, the
   folder MIME type and `trashed = false`, and `fields` naming
   `createdTime`, which `GoogleDrive::list` does not ask for. Refuse
   `root` as the parent for the reason the conformance support does
   (`tests/support/mod.rs`, `MY_DRIVE`). A `drive.file` grant sees what
   *this OAuth client* created, on any device, which is what lets a
   grant of its own see the Libraries the CLI made — say so in the
   example's doc comment, and make it a manual criterion below.
2. **Its own grant, made once.** `scripts/drive-it-reset.sh` (executable,
   `chmod +x`, like the other scripts) keeps the
   tool's token cache under `.tmp/drive-admin/` (`COFFRET_DRIVE_TOKEN_CACHE`)
   sealed under a `COFFRET_MASTER_KEY` fixed in the script — 32 bytes,
   base64, in the clear and commented like the two targets' fixed
   Passphrases: it protects a test grant on a test folder and nothing a
   person would keep. When the cache is missing, run the `authorize`
   example first (`cargo run --release -p google-drive-store --example
   authorize`), which prints the consent URL and waits — the one
   interactive step, once per machine. The reset never removes
   `.tmp/drive-admin/`, so a reset and the run after it cost no new
   consent for the tool itself.
3. **`make drive-it-list`** runs the script in listing mode: for every
   folder the tool lists, print its id, name, creation time, and which
   local scenario Library points at it — read `folder_id` out of every
   `settings.json` under `.tmp/drive-round-trip/state/libraries/` and
   `.tmp/drive-index-layout/state/libraries/` (the `settings_value`
   helper in either target script shows how) — or `stale` when none
   does. This is the answer to "which of these is old"; it changes
   nothing.
4. **`make drive-it-reset`** prints the same listing, trashes every
   folder in it, and then removes `.tmp/drive-round-trip/` and
   `.tmp/drive-index-layout/` (not `.tmp/drive-admin/`), so the next run
   of either target starts from a fresh Library — and, as the script
   says before it does anything, asks its consents again. Without
   `COFFRET_DRIVE_FOLDER_ID` both targets say so and exit 0, as the
   others do. The reset touches only folders under that parent: it
   never lists or trashes anything else, and the Makefile comment says
   so, so that a parent shared with a real Library is the one mistake
   the tool cannot make on its own.
5. **The Makefile blocks.** `## drive-it-list:` and `## drive-it-reset:`
   in the style of the other `drive-*-it` blocks, and a sentence in the
   `## drive-round-trip-it:` and `## drive-index-layout-it:` blocks
   replacing "removing it is the account owner's to do" with a pointer
   to the reset target.

The example binary is not shipped and not part of the CLI; it is a
test-support tool that lives with the gateway it uses. Keep it small.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `examples/app_folders.rs` builds, lists live `coffret-` folders under a parent with id, name and `createdTime`, trashes by id, and refuses `root`
- [x] `scripts/drive-it-reset.sh` keeps the tool's grant under `.tmp/drive-admin/`, authorizes once when it is missing, lists with the local scenario each folder belongs to, and in reset mode trashes and removes the two scenario directories
- [x] `make drive-it-list` and `make drive-it-reset` exist, are documented, and the two existing `drive-*-it` blocks point at the reset
- [x] The script contains no `read -s`; the only fixed secret is the tool's Master Key, commented as test-only

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-it-list` on a machine where both targets have run shows their Libraries as in use and any leftover as `stale` — confirming a grant of the tool's own sees the folders the CLI created under the same OAuth client
- [ ] `make drive-it-reset` trashes them, and the next `make drive-round-trip-it` asks for its consents again and runs green

## Out of scope

- Per-run folders on Drive or any change to how the targets name or reuse their Libraries
- Purging (permanently deleting) anything
- Folders outside `COFFRET_DRIVE_FOLDER_ID`, and any Library a person keeps under a different parent
