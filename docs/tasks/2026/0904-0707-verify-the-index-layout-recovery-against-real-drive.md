---
status: completed
pipeline_phase: null
plan: null
base_ref: task/0903-1308-rebuild-the-catalog-of-an-older-index-layout
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && test -x scripts/drive-index-layout-it.sh && bash -n scripts/drive-index-layout-it.sh && grep -q '^drive-index-layout-it:' Makefile && grep -q 'DEVICE_SCHEMA_VERSION' scripts/drive-index-layout-it.sh && ! grep -Eq 'read -r?s' scripts/drive-index-layout-it.sh && ! grep -Eq 'COFFRET_PASSPHRASE' scripts/drive-index-layout-it.sh"
assignee: null
branch: task/0904-0707-verify-the-index-layout-recovery-against-real-drive
created_at: 2026-09-04T07:07:39Z
updated_at: 2026-09-04T07:57:51Z
---

# test(drive): verify the Index layout recovery against real Drive from one target

## Overview

The Index layout change (discarding an older catalog layout, reading the
mappings out of a refused file, and `sync` catching the catalog up
before it scans) has two acceptance criteria that only a device with a
real Google Drive grant can decide. Checking them by hand exposed what
manual verification costs: a person typed the Passphrase, the check ran
against one of the owner's own Libraries (35 files, four megabytes, five
minutes of uploads), and the one real defect it found — `sync`
re-uploading everything — was found by luck of ordering rather than by
a step anyone could repeat. The project now wants on-device checks that
run from one command with nothing typed, against test data sized for
the question being asked, and that is what this task builds — the first
such target, shaped so later scenarios (a second device catching up, a
grant expiring) can be added as functions.

Build `make drive-index-layout-it`, a manual target in the mould of
`make drive-round-trip-it` (`scripts/drive-round-trip-it.sh`; read it
first and reuse its conventions and helpers by copying, not by
sourcing — the two scripts must stay independently runnable):

1. **Its own place on disk and nothing typed.** Everything under
   `.tmp/drive-index-layout/` — `state/` (exported as
   `COFFRET_STATE_DIR`), `logs/` (`COFFRET_LOG_DIR`), a transcript, the
   mapped folder. One fixed Passphrase written in the script, in the
   clear, like the round trip's: it protects generated test bytes and
   nothing else, and a script that asks a person for it is not a test.
   The script must contain no `read -s` and no environment-variable
   Passphrase. Without `COFFRET_DRIVE_FOLDER_ID` it says so and exits 0,
   exactly as the round trip does; `COFFRET_DRIVE_CLIENT_ID` /
   `_SECRET` are read the same way.
2. **One small Library, made once.** A Library the device calls
   `layout`, created by `init` on the first run and reused after (the
   settings file says whether it exists). The first run's `authorize`
   blocks on the consent screen — the one interactive step the round
   trip also has; copy its handling (the URL is printed live, an expired
   grant stops the run with the command that renews it). The mapped
   folder holds **three files of a few kilobytes each with
   deterministic content** written by the script itself (no
   `coffret-fixtures`, no JPEGs — the question is about the Index, not
   about images). Total well under 100 KB, so a whole run is a handful
   of Drive calls. On every run: `sync`, then assert the catalog holds
   exactly those three Entries (`sqlite3` on `state/libraries/layout/index.sqlite`).
3. **Scenario A — an older layout is discarded and the next `sync`
   rebuilds without re-uploading.** Read `SCHEMA_VERSION` and
   `DEVICE_SCHEMA_VERSION` out of
   `backend/crates/gateway/coffret-sqlite-index/src/schema.rs` at run
   time (a `grep` on the two `const` lines — the script lives in the
   repo, and hard-coding the numbers would drift). Record the mappings
   and the Container ids, restamp the Index to `DEVICE_SCHEMA_VERSION`
   with `PRAGMA user_version`, run `sync`, and assert: exit 0; the
   summary reports nothing added and three unchanged; the stamp is
   `SCHEMA_VERSION` again; `coffret mappings` lists the same mapping;
   the catalog holds the same three Entries and the same Container ids
   as before; no pending rows; the run's log holds exactly one WARN
   about the older layout with `found` = `DEVICE_SCHEMA_VERSION` and
   `supported` = `SCHEMA_VERSION`, and no "uploaded a Container" event.
4. **Scenario B — a refused file still lists its mappings.** Copy the
   Library directory to a second device-side name (`refused`) inside
   the same state directory — it points at the same Drive folder, and
   nothing in this scenario reaches Drive — restamp its Index to
   `DEVICE_SCHEMA_VERSION - 1`, and assert: `coffret mappings --library refused`
   exits 0, its stdout equals scenario A's mappings listing, its stderr
   says the Index cannot be opened and names `coffret map` and
   `coffret sync`; the stamp is unchanged afterwards; `sync --library refused`
   exits non-zero with the older-layout refusal. Remove the copy at the
   end (it is local only).
5. **Report like the round trip.** One line per assertion, a final
   summary naming the state directory and the log directory, exit 0
   only if every assertion held, and the CLI's own output in the
   transcript. Failures print what was expected and what was found.
6. **Makefile.** A `drive-index-layout-it` target with the same doc
   comment style as `drive-round-trip-it` (manual, needs an account,
   what state it keeps, that the app folder is reused and never
   trashed). Mention it where `drive-round-trip-it` is documented
   (`README` or `docs/`, if it is).

This target covers the two manual criteria of the Index layout task; a
green run is the evidence for ticking them. Drive-side cleanup (the app
folder is reused and grows by nothing — every run re-syncs the same
three files) and further scenarios are later tasks.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/drive-index-layout-it.sh` exists, is executable, parses
      (`bash -n`), reads both layout constants from `schema.rs` rather
      than hard-coding them (grep gate on `DEVICE_SCHEMA_VERSION`), and
      asks nobody for a Passphrase (grep gates: no `read -s`, no
      `COFFRET_PASSPHRASE`).
- [x] The Makefile has a `drive-index-layout-it` target (grep gate) and
      `make check` still passes (the target itself is manual and is not
      run by the check).

### Manual / on-hardware (verified by a human before merge)

- [ ] `COFFRET_DRIVE_FOLDER_ID=… make drive-index-layout-it` on a device
      with the consent answered: the first run creates the Library and
      passes both scenarios; a second run reuses it, uploads nothing, and
      passes both again. The run's green result is recorded on the Index
      layout task's manual items.

## Out of scope

- Trashing the app folder on Drive at the end of a run (the round trip
  does not either; the folder is reused and does not grow).
- Size arguments for `coffret-fixtures` and the round trip's fixture
  defaults — a separate small change.
- A second-device catch-up scenario and an expired-grant scenario —
  later functions in this script, once it exists.
- Running any of this in CI.
