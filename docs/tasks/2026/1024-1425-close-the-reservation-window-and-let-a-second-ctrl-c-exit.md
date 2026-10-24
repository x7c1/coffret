---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rqE "\\b130\\b" backend/crates/apps/coffret-cli/src/ && grep -rqE "fn [a-z_]*(slow|deadline|late)[a-z_]*\\(" backend/crates/apps/coffret-explorer-host/tests/ && grep -rq "system_files" backend/crates/domain/coffret-usecase/src/relocate/ && ! grep -q "declare -A" scripts/rename-smoke.sh'
assignee: null
branch: task/1024-1425-close-the-reservation-window-and-let-a-second-ctrl-c-exit
created_at: 2026-10-24T05:25:00Z
updated_at: 2026-10-24T08:22:14Z
---

# fix: close the reservation window, let a second Ctrl-C exit, give readers a schema floor of their own, test the head timeout, and keep rename and deletion to EP-17

## Overview

Review findings left open after earlier work, all small and each with its
fix already decided.

1. **A window between `reserve` and `take_over`.** In
   `backend/crates/apps/coffret-device/src/reservations.rs`, between drop A's
   `index.reserve` and its `live.hold`, a drop B to the same destination can
   `take_over` with `try_hold` and win the reservation A just made. Make the
   reserving side hold the same way (`try_hold` or equivalent, atomically with
   the reservation), so exactly one of two concurrent drops to one folder
   holds it and the other either continues it (PK-17) or is told it is taken.
   Also settle what happens when one folder has two cut-off reservations
   (both left by drops ended before they finished): pick one deterministic
   rule — for example the oldest is continued and the other released — write
   it into PK-17's sub-bullets in `docs/spec/pack-construction/README.md`, and
   test both.
2. **A second Ctrl-C does nothing, and the exit status is 1.** After the CLI
   is interrupted (`backend/crates/apps/coffret-cli/src/interrupted.rs`), the
   runtime's drop waits on work in `spawn_blocking`, and the registered
   handler swallows a second Ctrl-C. Decided: a second Ctrl-C exits at once
   with status 130, and an interrupted run that ends by itself also exits 130
   rather than 1. Say in the first Ctrl-C's message that a second one stops at
   once.
3. **Readers take their schema floor from the writers' `SCHEMA`.** The
   journal record and Index Snapshot decoders
   (`backend/crates/domain/coffret-format/src/control/journal_record/decode.rs`,
   `.../index_snapshot/decode.rs`, and the TypeScript codecs of the same names
   under `frontend/packages/domain/format/src/`) refuse anything below the
   schema the writer writes. Raising `SCHEMA` for a new field would then make
   every reader refuse the schema before it. Give the readers a separate
   lowest-accepted constant (equal to today's value, so nothing accepted
   changes now), and test that a record at the floor is read and one below it
   is refused.
4. **LA-19's head timeout has no test.** LA-19 in
   `docs/spec/loopback-access/README.md` is *(Form: test)*, but
   `backend/crates/apps/coffret-explorer-host/tests/` covers only the head
   size limit (`serve.rs`) and the `/auth` body timeout (`auth.rs`). Add a
   test that sends a request head slower than the header read timeout and
   observes the connection refused or closed. Shorten the timeout through
   whatever the host already exposes for tests, or add such a seam, rather
   than sleeping five seconds per case.

5. **Two paths EP-17 does not reach yet.** EP-17 (`docs/spec/entry-path/README.md`)
   says a name an operating system keeps is never Library content, and the
   scan, the drop walk and the upload route keep to it through
   `backend/crates/domain/coffret-usecase/src/system_files.rs`. Two places do
   not ask it:
   - **rename** — `fn reserved` in
     `backend/crates/domain/coffret-usecase/src/relocate/look.rs` refuses a
     destination in the trash folder, the management area or the scratch
     prefix, but not a system name, so `coffret rename --to book/.DS_Store`
     goes through and leaves an Entry no scan will ever see again. Refuse it
     the way the other reserved names are refused, and say so in the rename
     rules of the spec where the others are listed.
   - **a deletion's local copies** — `mapped_file` in
     `backend/crates/domain/coffret-usecase/src/departure/mapped_source.rs`
     promises to answer as the walk finds a file and lists EP-11/14/16 as what
     the walk passes over, but not EP-17. Deleting an Entry carried in before
     the rule (say `book/.DS_Store`) would then discard the device's
     `.DS_Store` through `delete/mapped_copies.rs`, a file the person never
     handed to the Library. Make it answer `None` for a system name as the
     walk does, so such a deletion leaves the local file alone, and state that
     in EP-17's sub-bullet about Entries already in the Library.
   - **the drop walk's event** — `backend/crates/apps/coffret-device/src/drops/walk_drop.rs`
     logs `not_taken = walk.not_taken.len()`, which counts the system files
     the walk left out among what it did not take, so almost every drop logs
     a non-zero `not_taken`. Count them apart (a field of their own), as the
     page already says them apart (LA-26).

6. **`scripts/rename-smoke.sh` does not run on macOS's own bash.** Line 230
   uses `declare -A` (an associative array), which bash 3.2 — the one macOS
   ships as `/bin/bash` — does not have, so the three `rename_smoke` cases of
   `make s3-store-it` fail on a Mac while CI on Linux passes. Rewrite that
   part without an associative array (the script's other helpers show the
   idiom), so the script runs on bash 3.2 as well.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Two drops to one destination racing through `reserve` / `take_over`
      leave exactly one holder, by a test that interleaves them; the
      two-cut-off-reservations rule is in PK-17 and tested.
- [x] A second interrupt exits with status 130, and an interrupted run that
      ends by itself exits 130, by tests on the CLI's interrupt handling
      (gate on `130` in `coffret-cli/src/` appended).
- [x] The journal record and Index Snapshot decoders (Rust and TypeScript)
      read their floor from a constant separate from `SCHEMA`, with a test at
      and below the floor on each side.
- [x] A test in `coffret-explorer-host/tests/` holds the head read timeout of
      LA-19, in a test whose name says the head is slow or late (gate on such a name in that directory appended).
- [x] A rename to a path with a system-file component is refused, and
      deleting an Entry at such a path leaves the device's file in place, each
      by a test (gate on `system_files` in `relocate/` appended).
- [x] `scripts/rename-smoke.sh` uses no associative array (gate
      `! grep -q "declare -A" scripts/rename-smoke.sh` appended).
