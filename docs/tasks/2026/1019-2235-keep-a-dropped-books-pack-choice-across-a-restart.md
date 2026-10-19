---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1019-2235-keep-a-dropped-books-pack-choice-across-a-restart
created_at: 2026-10-19T13:35:32Z
updated_at: 2026-10-19T15:11:47Z
---

# feat(freeze): keep a dropped book's Pack choice across a server restart and resume it

## Overview

When a person drops a folder and chooses to add it as a Pack, the upload route lands the files in the mapped folder and arms a freeze with the landed Entry Paths as its selection (spec: PK-17; `coffret-server/src/routes/upload/mod.rs`). That armed freeze lives only in the process (spec: LA-12). The server has no signal handling, so a desktop-app Quit stops it as abruptly as a crash does. Crash tests against a real Library showed:

- After a restart, any sync — even one armed by an unrelated drop — walks the mapped folder and carries the book in as one Container per page: 300 one-file Containers instead of one Pack. PK-13 can absorb them later, at the cost of uploading and trashing everything again.
- A freeze does not settle its own leftovers (`coffret-usecase/src/freeze/run.rs` leaves them to the next sync), so a Pack-only workflow keeps an interrupted spool (up to one Pack's size) and its pending row until a sync runs.

### Decisions (already made; implement them)

1. **Record the choice on the device.** Before the first file of a Pack drop lands, write a reservation into the device's Index: the folder and the set of Entry Paths the drop will pack, extended as files land. It is device-local state, like mappings and pending rows (spec: EP-9, OC-2): never uploaded, never in a Snapshot.
2. **Sync leaves reserved paths alone.** A sync's scan does not select reserved paths for addition or update; it reports them as waiting for their Pack (a finding or a count in the answer, never silent).
3. **Resume automatically.** After the server starts or unlocks and the catch-up has finished, it arms a freeze for every reservation, in the order they were made, through the same queue `freeze_folder` uses. Progress shows in the explorer as any freeze's does.
4. **Settle before freezing.** A freeze — resumed or not — first settles this device's leftover pending rows and spool under OC-2 / OC-3 (reuse sync's settle), then builds its Pack.
5. **Drop the reservation when done.** When the freeze commits, the reservation is deleted in the same step that completes its rows. When none of its paths is eligible any more (files gone, already in a Pack, refused per PK-1), it is deleted too, with a finding saying why. A reservation whose paths are partly gone packs what remains.
6. **Amend LA-12.** Keep the rule that a process's account of its work lives as long as the process; add the reservation as the one recorded exception, and its automatic resumption after start / unlock. Write PK-17 (reservations and sync avoiding them) and the OC rule for a freeze settling first.

### UX

- The explorer shows a resumed freeze like any other, so a person returning after a restart sees the book being packed.
- Reserved files in a folder listing are marked as waiting to be packed rather than as plain unreflected files (reuse the existing marks; no new design language).

### Tests

- A Pack drop interrupted after landing and before the freeze commits (simulate the process ending): after restart and unlock, the freeze resumes and commits one Pack with exactly the landed paths; no one-file Container is made.
- A sync run before the resumption does not add the reserved paths.
- An interrupted freeze's spool and pending row are settled by the resumed freeze before it builds.
- A reservation whose files were deleted is dropped with a finding; a partly deleted one packs the rest.
- Two reservations resume in order.
- Index schema migration (or refusal of an older layout, per the existing policy) for the new table, on both Index implementations.

### Out of scope

User-initiated cancellation of an import, signal handling, the format of the Library.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A Pack drop's choice survives a restart and its freeze resumes after unlock, producing one Pack (tests).
- [x] Sync never adds reserved paths and reports them (tests).
- [x] A freeze settles leftover pending work before building (tests).
- [x] Reservations are deleted on commit or when nothing eligible remains, with a finding in the latter case (tests).
- [x] LA-12, PK-17 and the OC rules describe the reservation, and the spec checks pass.

### Before merge

- [x] Against the agent's development Library, drop a 300-page fixture book as a Pack via `POST /api/upload?freeze=true`, SIGKILL the server during the spool and again during the put, restart, and confirm one Pack is committed with no one-file Containers and no leftover spool.
