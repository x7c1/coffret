---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && bash -n scripts/drive-round-trip-it.sh && grep -q 'uploaded no Container\\|uploads_in' scripts/drive-round-trip-it.sh && grep -Eq 'sync --library \"\\$JOINER\"' scripts/drive-round-trip-it.sh && ! grep -Eq 'read -r?s' scripts/drive-round-trip-it.sh"
assignee: null
branch: task/0919-1835-let-the-second-device-sync-without-re-uploading
created_at: 2026-09-19T18:35:00Z
updated_at: 2026-09-20T02:50:00Z
---

# test(drive): let the second device sync and check it re-uploads nothing

## Overview

`make drive-round-trip-it` (`scripts/drive-round-trip-it.sh`) plays two
devices on one machine: `main` carries a batch of generated files into a
Library on real Google Drive, and `second`, joined with the Recovery
Code, fetches it back and compares byte for byte. What `second` never
does is `sync`. On a real second device that is the next thing a person
does — map the folder the fetch filled and sync it, so that what they add
there goes up too — and it is where the one real defect the real-Drive
checks have found so far lived: a device that scanned its folder before
catching the catalog up saw every fetched file as new and re-uploaded
the lot (fixed on the CLI side; the catalog catch-up now runs before the
scan). No target exercises that path against Drive; the index-layout
target checks a rebuild on the *same* device, not a second one.

Add the step to the round trip, after the fetch and the byte-for-byte
comparison and before the deletion step:

1. **`second` syncs the folder it fetched into.** `second`'s mapping
   already exists (`map --library "$JOINER"` at the top of the run).
   Run `sync --library "$JOINER" --passphrase-stdin` through `run_cli`
   and take `log_of_the_last_run`-style evidence the way the
   index-layout script does: read the log file the CLI names, and count
   Container uploads in it. Assert: the exit status is 0 or `FINDINGS`
   (a deletion an earlier run made on `main` is surfaced on `second`
   too only if `second` deleted it, which it did not — so 0 is the
   expected value; treat `FINDINGS` as a failure with a message that
   says what was surfaced); the summary line reports `added 0` and
   `replaced 0`, with `unchanged` equal to the number of files under
   `$JOINER_ROOT`; it says `committed nothing`; and the log records
   **no** Container upload. Say in the step's comment why this is the
   check: the catch-up before the scan is what keeps a second device
   from re-packing what it fetched.
2. **The helpers the assertion needs.** The round-trip script has no
   `log_of_the_last_run` or `uploads_in`; copy them from
   `scripts/drive-index-layout-it.sh` (the two scripts stay
   independently runnable — copy, do not source) and keep the log level
   pinned (`COFFRET_LOG=info`) with the same reasoning that script gives
   at its `export COFFRET_LOG=info`: an upload event that never reached
   the file is indistinguishable from one that never happened.
3. **The run's summary says it.** The `=== the round trip held ===`
   block gains a line for the second device's sync (files unchanged,
   nothing uploaded), and the Makefile's `## drive-round-trip-it:` block
   and the script's header mention that the second device now syncs as
   well as fetches.

The step runs every time, including the run right after a join (where
`second`'s folder holds the whole Library after the fetch) — the
assertion is the same in both cases: nothing added, nothing uploaded.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/drive-round-trip-it.sh` runs `sync` on the joined device after the fetch and asserts `added 0`, `replaced 0`, `committed nothing`, and no Container upload in the CLI's log
- [x] The script pins `COFFRET_LOG=info` and reads the log file the CLI names, with the helpers copied rather than sourced
- [x] The summary block, the script header, and the Makefile's `## drive-round-trip-it:` block describe the second device's sync

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-round-trip-it` runs green against real Drive, and the new step reports the second device's files unchanged with no upload

## Out of scope

- Propagating a deletion from one device to the other (not implemented in the CLI; the round trip only surfaces it)
- `scripts/drive-index-layout-it.sh`
- Cleaning up what earlier runs left on the account or on disk
