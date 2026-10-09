---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1009-1056-move-deleted-entries-local-files-to-the-trash
created_at: 2026-10-09T01:56:44Z
updated_at: 2026-10-09T08:50:21Z
---

# feat(sync): move a deleted Entry's unedited local file to the OS trash, and keep an edited one reported

## Overview

When an Entry leaves the Library (a delete from any device, committed and then
caught up), a device that materialized it still has the file in its mapped
folder. Today nothing handles that file: catch-up drops the Entry from the
catalog (`coffret-sqlite-index/src/library_state.rs`, `apply`) and keeps the
device's `local_entries` row (by design, `schema/mod.rs`), but the next scan
finds no Entry at the path (`coffret-usecase/src/sync/scan/examine.rs`) and
selects the file as new, so `sync` carries it back into the Library. A deletion
of anything a device holds locally therefore does not stick. The Library
concept leaves "what a device does with its own local copy of a deleted Entry"
as a separate question; this change answers it.

**1. The rule.** On a scan, a file at a path this device materialized
(`local_entries` row `present`) where the Library no longer has an Entry is a
**departed** file:
- **unedited** — its content is what this device last made match the Library —
  is moved to the OS trash (freedesktop Trash on Linux, the Finder's trash on
  macOS), its row is closed, and the run reports it as moved to trash with the
  path;
- **edited** is left in place, never selected for upload, and reported every
  run as "deleted from the Library, kept here because it changed", until the
  person moves or removes it. Moving it to another path makes it an ordinary new
  file there (existing rules).
The deleting device and every other device behave the same. A path where a
new Entry has arrived since (another device added a file at the same path) is
not departed: existing rules apply. A path with no `present` row is not
departed either: a file never materialized is still new (EP-10). State the
rule in the Entry Path spec next to EP-10 / EP-11 (a new rule ID, or extend
EP-10 if it fits), and update the Library concept's passage on a deleted
Entry's local copy and its add / materialize Domain Rules.

**2. Deciding "unedited".** `local_entries` holds only the observed size and
mtime, and catch-up deletes the Entry's hash, so the device cannot compare the
content with the departed Entry today. Record the materialized Entry's hash
with the row when it is marked present (upload and fetch both know it) — a
schema bump of the device-local tables. A file is unedited when its size and
mtime equal the row's; when they differ, when it hashes to the recorded hash.
A row without a recorded hash (made before this change) whose size or mtime
differs counts as edited: keep the file rather than guess.

**3. Moving to the trash.** Use the `trash` crate behind a small port in the
device / usecase layer so tests use an in-memory fake and never touch the real
trash. A trash that fails (no trash available on the volume, permission)
leaves the file in place, reports the failure with its reason, and is retried
on the next run; it never deletes the file outright and never fails the rest of
the run. Do the move only after the scan has confirmed the root vouches for
itself and is available (EP-12, EP-13).

**4. Reports.** Add the two outcomes to the run's findings (the `Surfaced`
family in `sync/surfaced.rs` and the device `Finding`) and render them in the
CLI answer and in what the explorer shows for a sync run, following the existing
`DeletedLocally` path. The explorer's listing keeps showing an edited departed
file the way it shows other files on disk without an Entry; label it if the
listing already distinguishes such files.

Out of scope: restoring from the trash; re-registering an edited departed file
automatically; files the device never materialized; Storage-side trash.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] After a catch-up that removed an Entry this device materialized, a sync moves the unedited file to the trash port, closes its row, reports it, and uploads nothing for it (conformance test, for an Entry removed by this device and by another device)
- [x] An edited departed file (size or mtime differ and the hash differs) stays in place, is not uploaded, and is reported on this and the next run (conformance test)
- [x] A file whose size or mtime changed but whose content hashes to the recorded hash is treated as unedited (test)
- [x] A row without a recorded hash whose file changed is kept and reported (test)
- [x] A path where a new Entry arrived, and a file never materialized, follow the existing rules (tests)
- [x] A failing trash leaves the file, reports the reason, does not fail the run, and is retried next run (test)
- [x] Nothing is trashed under an unavailable or refused root (test)
- [x] The schema upgrade keeps existing rows and the conformance suites of both index implementations pass
- [x] The spec states the rule, and `make spec-citations spec-rule-ids` passes
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Needs a person: under `make desktop-dev`, deleting a materialized file from the explorer and letting sync run moves it to the desktop's trash, and an edited one stays with the report
