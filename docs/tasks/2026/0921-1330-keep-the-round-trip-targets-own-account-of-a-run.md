---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && bash -n scripts/drive-round-trip-it.sh && bash -n scripts/drive-index-layout-it.sh && grep -q 'report.log' scripts/drive-round-trip-it.sh && grep -q 'flush_the_report' scripts/drive-round-trip-it.sh && grep -q '^echo \"Report: ' scripts/drive-round-trip-it.sh && grep -q 'report.log' Makefile"
assignee: null
branch: task/0921-1330-keep-the-round-trip-targets-own-account-of-a-run
created_at: 2026-09-21T04:30:35Z
updated_at: 2026-09-21T05:54:46Z
---

# test(drive): keep the round-trip target's own account of a run, and end every run's with how it exited

## Overview

`make drive-round-trip-it` (`scripts/drive-round-trip-it.sh`) keeps what
the CLI printed in `.tmp/drive-round-trip/transcript.log`, and nothing
of what the script itself said: the step headings, the assertion that
stopped the run (`fail`), and the closing `=== the round trip held ===`
summary go to the terminal only. Whether a run held cannot be read
afterwards, and the transcript is appended across runs with nothing
marking where one ends and the next begins.

`scripts/drive-index-layout-it.sh` already answers this, and its answer
is the one to carry over rather than a second design:

- a `REPORT="$WORK/report.log"` beside the transcript, with the comment
  saying what it is that the transcript is not
- a `=== run <UTC time> on <branch> <short commit> ===` header appended
  to it before the first word about the run, placed after the skip and
  the precondition checks so that a run configured for nothing leaves no
  file
- `exec > >(tee -a "$REPORT") 2>&1` with `REPORT_TEE=$!`, a process
  substitution so that `pipefail` and the consent URL's liveness are
  both untouched
- `flush_the_report`, called from `fail` and at the end, so the report
  is finished by the time the run is
- a `Report:` line under `Transcript:` in the closing summary

Do these:

1. **Give `drive-round-trip-it.sh` the same report**, with the same
   names (`REPORT`, `REPORT_TEE`, `flush_the_report`, `report.log`) and
   the same placement rules. `run_cli` there shows the CLI's output live
   through `tee "$LAST"`; once the script's standard output is the
   report's `tee`, that output reaches the report as well, which is
   wanted — the report is the terminal's account of the run, CLI output
   included, as it already is for the commands `drive-index-layout-it.sh`
   shows live. The transcript stays what it is (the CLI's output, for
   the script to have read back); do not remove or merge it. The grant-
   expired path in `run_cli` ends in `fail`, so it is covered once
   `fail` flushes.
2. **End each run's block with how the run exited, in both scripts.**
   One line, written last, from an `EXIT` trap, naming the exit status —
   and what that status means where the script gives it a meaning (0
   held; 1 failed; in the round trip, nothing the script itself exits
   with is `$FINDINGS`, that is a status of the CLI's it asserts on, so
   do not invent a meaning for 2). This is what makes a run that died
   under `set -e` on a line nobody wrote a `fail` for distinguishable
   from one still running or one whose terminal was closed: today such
   a run's block simply stops. The line goes to the report even when
   `flush_the_report` has already run — that function's comment already
   says it leaves standard output on the report file for exactly this
   late printer. `drive-index-layout-it.sh` sets and clears its own
   `EXIT` traps for temporary directories (`REFUSED_DIR`,
   `UNGRANTED_DIR`); those must go on working, so compose rather than
   overwrite (one trap function that does the cleanup that is due and
   then writes the line is fine). The skip and the precondition checks
   before the report exists must still leave no file. Two paths would
   break the promise that a block always ends with that line, and both
   are closed here: a cleanup that fails inside the trap is reported
   and passed over rather than left to `set -e` (which would also turn
   a run that held into one that failed), and a run somebody stops with
   Ctrl-C or `kill` leaves through the same trap, exiting 130 or 143 —
   the consent wait is where a Ctrl-C really happens. A terminal that
   was closed and a `kill -9` still leave a block that stops.
3. **Say so in the Makefile.** The `## drive-round-trip-it:` block gets a
   sentence naming `.tmp/drive-round-trip/report.log` as where a run's
   own account and its exit are kept, and the `## drive-index-layout-it:`
   block the same for its report if it does not already say it.
4. **The Recovery Code.** The round trip's transcript holds the Recovery
   Code because the CLI prints it, and the script's comments say the
   transcript has to be kept secret for that reason. The report now
   holds it too, by the same route. Say so where `REPORT` is declared,
   in the terms the existing comment uses. No new secret reaches disk:
   the Passphrase is a constant of the script and is only ever written
   to the CLI's standard input.

Write comments in the voice the two scripts already have, and do not
restate in the round trip what `drive-index-layout-it.sh` explains at
length where a shorter comment pointing at the same reasoning reads
better; the two files are read separately, though, so each must stand
on its own.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/drive-round-trip-it.sh` declares `REPORT` as `report.log` under its work directory, appends a run header and tees both streams into it after the skip and precondition checks, flushes it from `fail` and at the end, and names it on a `Report:` line in the closing summary
- [x] Both scripts pass `bash -n`, and `make check` passes
- [x] The Makefile's `## drive-round-trip-it:` block names `report.log`

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-round-trip-it` against real Drive holds, shows the CLI's output live as before, and leaves in `.tmp/drive-round-trip/report.log` one block for the run: header, the step headings, the closing summary, and a last line saying it exited 0
- [ ] A run made to fail after the report exists (for instance with one assertion broken locally) ends its block with the failure's message and a last line saying it exited non-zero
- [ ] A run stopped with Ctrl-C ends its block with a last line saying it exited 130, and exits 130
- [ ] With `COFFRET_DRIVE_FOLDER_ID` unset, both targets skip and create no `report.log` where there was none
- [ ] `make drive-index-layout-it` still ends its block with its verdict followed by the exit line, and its temporary directories are still removed

## Out of scope

- Linting the scripts with shellcheck / shfmt, and moving assertions off the CLI's human-readable output (a separate follow-up)
- Sharing code between the two scripts through a sourced library
- `scripts/e2e-it.sh`, which CI runs and keeps the output of
- Rotating or truncating `report.log` and `transcript.log`
