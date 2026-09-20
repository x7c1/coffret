---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && bash -n scripts/drive-index-layout-it.sh && grep -q 'scenario C' scripts/drive-index-layout-it.sh && grep -q 'token-cache.cftc' scripts/drive-index-layout-it.sh && ! grep -Eq 'read -r?s' scripts/drive-index-layout-it.sh && ! grep -Eq 'COFFRET_PASSPHRASE' scripts/drive-index-layout-it.sh && grep -q 'scenario C' Makefile"
assignee: null
branch: task/0919-1830-check-what-a-library-without-a-grant-says
created_at: 2026-09-19T18:30:00Z
updated_at: 2026-09-19T21:30:00Z
---

# test(drive): check what a Library without a grant says, from the same target

## Overview

`make drive-index-layout-it` (`scripts/drive-index-layout-it.sh`) is the
narrow real-Drive check: one small Library, made once, and scenarios
that each ask one question of it. Scenario A discards an older Index
layout and rebuilds; scenario B copies the Library to a second
device-side name, stamps the copy too old, and checks what the refused
copy still answers — nothing in B reaches Drive, and the copy is removed
when it ends. Both scenarios are gated on
`DEVICE_SCHEMA_VERSION < SCHEMA_VERSION`
(`scripts/drive-index-layout-it.sh:229`), so on a build where the two
are equal the whole target refuses to run. That is right for A and B,
which have nothing to check on such a build, but it currently leaves
the target with no scenario at all — as of the current `main`, where
both constants are 6.

The scenario this task adds is the one the target's own comment block
promised next: a grant that is gone. The two lines the CLI prints for
it are already named in the script as `NO_GRANT`
(`scripts/drive-index-layout-it.sh:118`), and `stop_at_a_dead_grant`
turns either into "stop and print the command that renews it". What no
target checks is that the CLI actually says those lines, exits
non-zero, and names `coffret authorize` — the behaviour a person hits
the week their consent screen's Testing-mode grant expires. This
scenario checks it without spending a grant or asking anyone for a
consent:

1. **Scenario C — a Library whose grant is gone says so and names the
   renewal.** Follow scenario B's shape exactly: copy
   `$STATE_DIR/libraries/$LIBRARY` to a third device-side name
   (`ungranted`), register the same `trap` to remove it, and remove the
   copy's `token-cache.cftc` (the file the CLI keeps the sealed grant
   in, beside `settings.json`, `master-key.cfmk` and `index.sqlite`).
   Then run `sync --library ungranted --passphrase-stdin` **with the
   two streams apart and without the dead-grant stop** — `run_cli_apart`
   is the right runner, but check whether it calls
   `stop_at_a_dead_grant`; if it does, give this scenario a runner that
   does not, because here the dead grant is the expected outcome, not a
   reason to halt. Assert with the script's `assert_equal` /
   `assert_says` / `held` / `broke` helpers: the exit status is
   non-zero; stderr matches the first alternative of `NO_GRANT` (the
   cache holds nothing); stderr names `coffret authorize`; the Index of
   the copy was not touched (its stamp is still `SCHEMA_VERSION`, and it
   holds the same Entries as before — read with the existing `stamp_of`
   / `entries_in`); and nothing was uploaded (the run's log, via
   `log_of_the_last_run` and `uploads_in`, holds no Container upload).
   Remove the copy afterwards, as B does.
2. **Scenario C runs even when A and B cannot.** Move the
   `DEVICE_SCHEMA_VERSION < SCHEMA_VERSION` gate from "refuse the whole
   run" to "skip A and B, and say so": on a build where the two are
   equal the run still creates or opens the Library, syncs the three
   files, checks what it holds, runs scenario C, and reports. The
   summary lines at the end (`=== both scenarios held ===` and the
   `Layout:` line) have to say which scenarios ran. Keep the existing
   message about the empty boundary; it becomes the reason A and B were
   skipped rather than the reason nothing ran.
3. **The Makefile and the script header describe three scenarios**, and
   the `## drive-index-layout-it:` block in the Makefile says the target
   still runs on a build whose layout boundary is empty, with scenario C
   alone. Name the scenarios by their letters in both places — the
   literal text `scenario C` has to appear in the script and in that
   Makefile block, since the check greps for it. Keep the block's existing points (sqlite3, the fixed
   Passphrase, the reused Library, nothing trashed on the account).

Nothing in scenario C reaches Drive: the copy has no grant, so the
first call the CLI would make is the one that fails. The rest of the
run — the sync of the three files before any scenario — does reach
Drive, exactly as today. No new consent is needed on a machine where
the target has run before.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/drive-index-layout-it.sh` has a scenario C that copies the Library, removes the copy's `token-cache.cftc`, runs `sync` on it, and asserts a non-zero exit, the no-grant line, the `coffret authorize` renewal, an untouched Index, and no upload
- [x] The run no longer refuses outright when `DEVICE_SCHEMA_VERSION` equals `SCHEMA_VERSION`; it skips scenarios A and B with the existing explanation and still runs scenario C
- [x] The script contains no `read -s` and no environment-variable Passphrase, as before
- [x] The Makefile's `## drive-index-layout-it:` block and the script header describe scenario C and the empty-boundary behaviour

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-index-layout-it` on current `main` (where both schema constants are 6) runs green with scenario C alone, reporting A and B skipped

## Out of scope

- Revoking a grant on Google's side (that needs the refresh token, which the sealed cache does not expose) — the token endpoint's `invalid_grant` mapping is covered by the gateway's unit tests
- `scripts/drive-round-trip-it.sh`
- Cleaning up what earlier runs left on the account or on disk
