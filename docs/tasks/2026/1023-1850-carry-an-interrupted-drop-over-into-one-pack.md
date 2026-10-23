---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "received" backend/crates/gateway/coffret-sqlite-index/src/schema/reservations.rs'
assignee: null
branch: task/1023-1850-carry-an-interrupted-drop-over-into-one-pack
created_at: 2026-10-23T09:50:00Z
updated_at: 2026-10-23T19:30:00Z
---

# fix(server): carry a drop that was cut off by a quit over into one Pack when it is dropped again

## Overview

A drop's files are received into the mapped folder one by one, and the drop is
frozen into a Pack once the last one has arrived (`routes/upload/finish_drop.rs:51-60`).
Quit the app halfway and the reservation stays in the catalog with the paths
that arrived; the next start takes every reservation up and freezes it
(`freeze/resume.rs:36-74 resume_reserved`, spec PK-17 and LA-12), so the
files that arrived become a Pack. Dropping the same folder again then sends the
rest, and the paths that were packed are refused as inside a Pack
(`routes/upload/receive.rs:95-97`, "the Library holds this file inside a Pack")
while the remainder becomes a second Pack. One book, two Packs.

### Decision (already made; implement it)

A reservation whose receiving was never finished is not frozen at the next
start. It is kept as a drop in progress, and dropping the same folder again
takes it over, so that one Pack results.

- Give a reservation a mark that its receiving finished (a column on the
  reservations table, `backend/crates/gateway/coffret-sqlite-index/src/schema/reservations.rs`
  and `device_state/reservations.rs`; an `Index` method to set it; the
  in-memory Index follows). `finish_drop` sets it just before it calls the
  freeze. A reservation created before this change has no mark and counts as
  unfinished.
- `resume_reserved` takes up only the marked reservations. An unmarked one is
  left in place and logged at start-up as a drop in progress that was cut off
  (count and folder; no paths, EL-1).
- A drop onto a folder that holds an unmarked reservation of this device
  (`routes/upload/mod.rs:268`, and `import/mod.rs:105` for a real-path drop)
  takes that reservation over instead of making a new one: its `paths` start
  as the ones already received, a file that arrives again replaces the one on
  disk (same name, same place), and the freeze that follows packs the whole
  of it. The explorer needs no change: it sends the drop as it always does.
- The listing of such a folder keeps showing the received files as it does
  today (`added_locally`), so a person sees what arrived.
- Spec: rewrite PK-17 (`docs/spec/pack-construction/README.md:192-252`) and
  LA-12 (`docs/spec/loopback-access/README.md:132-`) so that a server takes up
  the reservations whose receiving finished, keeps the others, and a drop onto
  their folder continues them. Update the Library concept where it describes
  what happens to a drop when the app quits, if it does.

### Out of scope

- Stopping a drop from the explorer (the explorer still has no control for
  it).
- Forgetting an unfinished reservation on any trigger other than its own
  re-drop; a later task may add a way to discard it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The reservations table has the finished mark (`grep -q "received"` on `schema/reservations.rs` is appended to `check_command`); `index_conformance/device_state/reservations.rs` covers setting it and that a reservation without it is listed as unfinished; the SQLite layout version is bumped and the upgrade keeps existing reservations as unfinished.
- [x] `freeze_conformance/reservations.rs` and `coffret-server/tests/routes/resumed.rs`: a reservation marked finished before the server stopped is taken up and packed by the next server (as today); one not marked is left in place, not packed, and the start-up log says a cut-off drop is in progress.
- [x] Server route test (`tests/routes/upload.rs` and the import route): a drop onto a folder with an unfinished reservation of this device continues that reservation — the paths already received are in its `paths`, no `pack_resident` refusal is raised for them, and the freeze that follows produces exactly one Pack holding every file of both drops.
- [x] `make spec-citations` and `make spec-rule-ids` pass after the PK-17 and LA-12 edits.

### Before merge (verified outside the check command)

- [x] On this machine, with the agent's development Library (`libraries-dev/agent`, see `docs/guides/environments.md`) and the development server: drop a folder of several files through the explorer, stop the server while the drop is being received, start it again, drop the same folder again, and confirm the explorer lists one Pack-resident folder holding every file and the server log says the cut-off drop was taken over. The merging session records what it ran in the PR.
