---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && bash -n scripts/drive-round-trip-it.sh && make -n drive-round-trip-it >/dev/null && make help | grep -q drive-round-trip-it && env -u COFFRET_DRIVE_FOLDER_ID scripts/drive-round-trip-it.sh | grep -q -i skip"
assignee: null
branch: task/0831-0030-drive-round-trip-in-one-make-target
created_at: 2026-08-31T00:30:00Z
updated_at: 2026-08-30T17:22:42Z
---

# feat(backend): drive a real-Drive round trip from one make target

## Overview

Checking the CLI against a real Google Drive account today is a hand-run
sequence of eight commands across two Library names — build, generate
fixtures, `init`, `map`, `sync`, `join`, `map`, `fetch`, then a manual
`diff` — with a Recovery Code and a folder id copied by hand from one
command's output into another's arguments. It took the last verifier the
better part of an hour, most of it reading output for the next value to
paste. The MinIO round trip is already one target (`make s3-store-it`); the
Drive one should be too.

Add `scripts/drive-round-trip-it.sh` and a `make drive-round-trip-it`
target that runs the whole round trip unattended except for the browser
consent, and can be re-run without leaving anything new on the account.

### What the script does

Inputs are the variables the Drive conformance suite already uses:
`COFFRET_DRIVE_CLIENT_ID`, `COFFRET_DRIVE_CLIENT_SECRET` (optional) and
`COFFRET_DRIVE_FOLDER_ID` (the parent folder — required, since `init
--drive` requires `--parent`). Without `COFFRET_DRIVE_FOLDER_ID` the script
says so and exits 0 like the other manual targets. No token cache variable:
the CLI keeps one per Library.

1. **Build** the release CLI (`cargo build --release -p coffret-cli`) and
   pick the binary from `backend/target/release/coffret`.
2. **State is persistent, not temporary.** `COFFRET_STATE_DIR` points at
   `.tmp/drive-round-trip/state/` (gitignored) so a second run finds the
   Libraries the first one created. Two device-side names: `main` (the
   uploader) and `second` (the joiner). The passphrase is a fixed test
   string fed with `--passphrase-stdin`; it protects nothing real here and
   the script says so.
3. **First run only**: `init --name main --drive --parent
   "$COFFRET_DRIVE_FOLDER_ID"`, parsing the Recovery Code (the stdout line
   starting `coffret1`) and the app folder id (the `On Storage: the Google
   Drive folder <id>` line) from the output; then `join --name second
   --recovery-code … --drive --folder-id …`. Each opens the browser once —
   the script prints what is about to happen before each consent. Later
   runs detect `main` and `second` under the state dir and skip both.
   An expired grant (the consent screen is in Testing, so refresh tokens
   die after 7 days) is reported by the CLI with its `coffret authorize`
   hint; the script surfaces that line and exits 1 rather than trying to
   authorize on the user's behalf.
4. **Each run adds fresh files.** Generate a small fixture set
   (`coffret-fixtures`, a dozen photos and a few pages) into
   `.tmp/drive-round-trip/main/runs/<UTC timestamp>/`; `main` maps the
   `runs` prefix to `.tmp/drive-round-trip/main/runs/` (recorded once, the
   re-map on later runs prints the was-at line and is fine); `sync
   --library main` must exit 0 and its summary must say `committed head
   <n>` (the number grows by one per run — the script prints it).
5. **Fetch on the joiner.** `second` maps `runs` to
   `.tmp/drive-round-trip/second/runs/`; `fetch --library second --under
   runs` must exit 0; `diff -r` of this run's subfolder on both sides must
   be empty; the script also asserts `fetched <count>` equals the number of
   files it generated this run (earlier runs' files are already present and
   count as `skipped`).
6. **A finding path.** Delete one file from this run's subfolder on `main`
   and `sync` again: exit must be 2 and stdout must carry a `surfaced …:
   this device had it and it is gone from disk` line; the script restores
   nothing (deletion propagation is off, the Entry stays in the Library).
7. **Report.** One block at the end: the app folder id and name (so the
   user can find it in Drive), heads committed, files round-tripped, the
   log paths. Exit 0 only when every step above held.

Nothing on Drive is trashed or purged by the script: the app folder is
created once and reused, so re-running does not litter the account. The
one folder stays until the user deletes it (a `coffret` command that
discards a Library, on the device and on Storage, is a later change).

### Makefile

`## drive-round-trip-it: …` in the comment form `help` parses, next to
`drive-store-it`, with the same "Manual: needs an account and a grant, so
CI never runs it" note. `.tmp/` is already gitignored.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/drive-round-trip-it.sh` parses (`bash -n`), is executable,
      and `make -n drive-round-trip-it` resolves; `make help` lists the
      target (all three appended to `check_command`).
- [x] With `COFFRET_DRIVE_FOLDER_ID` unset the script prints that it is
      skipping and exits 0 without building or touching `.tmp/`
      (`env -u COFFRET_DRIVE_FOLDER_ID scripts/drive-round-trip-it.sh | grep -q -i skip`
      is appended to `check_command`; the skip branch runs before any
      build).

### Manual / on-hardware (verified by a human before merge)

- [x] First run on a real Google account: two consents, then `init` /
      `join` / `sync` / `fetch` / the deletion `sync` all pass and the
      report prints the folder id; `.tmp/drive-round-trip/` holds `main`
      and `second`.
- [x] Second run, no consent: `sync` commits the next head, `fetch` places
      exactly this run's files, the earlier ones count as skipped, and
      Drive shows the same single `coffret-<hex>` folder.

## Out of scope

- Reusing one account's grant across Libraries on a device (so `join` on
  the same machine would not need a second consent) — a `coffret-device`
  design change, tracked separately.
- Discarding a Library (trashing the app folder from the CLI).
- Running this in CI.
