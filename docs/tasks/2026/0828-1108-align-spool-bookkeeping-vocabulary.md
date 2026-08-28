---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && (cd backend && RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items) && ! git grep -qE 'complete_pending_spool|PendingSpoolState|pending_spool' -- backend && ! git grep -qiE 'provisional|pending spool|nothing was left behind' -- backend && git grep -qF 'pub enum SpoolState' -- backend/crates/domain/coffret-usecase/src/device_state/spool_state.rs && git grep -qF 'fn mark_spooled' -- backend/crates/domain/coffret-usecase/src/index.rs && git grep -qF 'SCHEMA_VERSION: i64 = 4' -- backend/crates/gateway/coffret-sqlite-index/src/schema.rs && ! git grep -qE '\"writing\"|\"written\"' -- backend/crates/gateway/coffret-sqlite-index"
assignee: null
branch: task/0828-1108-align-spool-bookkeeping-vocabulary
created_at: 2026-08-28T11:08:56Z
updated_at: 2026-08-28T13:10:00Z
---

# refactor(backend): align the spool bookkeeping vocabulary with the spool verb

## Overview

The concept documents fixed the vocabulary for this bookkeeping: the act of
writing a Container's ciphertext to a local file before uploading is **spool**
(`docs/concepts/library/README.md:45`, used normatively at `:75`), and the
Index record that names such a file is a **pending row**. The row-first change
that introduced the row's two-state lifecycle drifted from both, in four ways
this task repairs. This is a rename with no behavior change other than the
SQLite schema-version bump described below.

**1. The state enum speaks write/written where the canon says spool/spooled.**
`PendingSpoolState { Writing, Written }`
(`device_state/pending_spool_state.rs:13`–`:28`) becomes
`SpoolState { Spooling, Spooled }` in a renamed
`device_state/spool_state.rs`, with the re-export in `device_state/mod.rs`
following. The meaning of each variant is untouched — only the wording of the
variants and of doc comments that say "writing"/"written" *as the state's name*
moves to the spool verb; prose that uses "written" as ordinary English (e.g.
"the row is written before the file exists") stays. The
`PendingUpload::state` field and its invariant doc
(`device_state/pending_upload.rs:55`–`:58`) keep their content with the new
names.

**2. `Index::complete_pending_spool` gave `complete` a second meaning.** The
Index concept's Collocations register `complete` for OC-7's bookkeeping
completion of an interrupted run; the port operation at `index.rs:241` uses the
same verb for a different act (recording that one spool file is whole). Rename
it to `mark_spooled(container_id)`, following the port's own idiom for a narrow
state flip — `mark_present` / `mark_absent` (`index.rs:185`, `:195`). The
contract is unchanged: an update and not an upsert; a Container with no row
changes nothing; no new `IndexError` variant. Both adapters follow
(`in_memory_index/state.rs`, `in_memory_index/mod.rs`;
`coffret-sqlite-index/src/device_state.rs`, `sqlite_index.rs`), as does the
pass-through in `sync_conformance/refusing_index.rs` and the fault injection in
`sync_conformance/watching_index.rs` (its refusal message "completing a spool"
becomes wording that names the new operation).

**3. The SQLite adapter stores the old verbs.** `rows::spool_state_text`
(`rows.rs:54`–`:57`) maps to `"writing"` / `"written"` and reads them back at
`rows.rs:202`–`:203`; the write sites are `device_state.rs:193` and `:220`.
Store `"spooling"` / `"spooled"` instead, and **bump `SCHEMA_VERSION` from 3
to 4** (`schema.rs:18`): a database written by an older build would otherwise
be read with state texts this build no longer recognizes. The schema module's
posture (refuse as `UnsupportedSchema`, discard and rebuild) already handles
the transition; update anything that names the version.

**4. Prose and test names use two extra nouns for the same record.** The
codebase calls one thing four names: pending row (the canon), "pending spool",
"provisional row" / "provisional spool", and the type name `PendingUpload`.
Sweep the first three down to *pending row* — a row whose state is `Spooling`
may be qualified as "a Spooling row" where the distinction matters:

- `sync_conformance/watching_index.rs`: the `provisional` counter, its
  `provisional_rows()` accessor (`:37`, `:64`), and the rustdoc at `:22`, `:25`,
  `:148`, `:160` — e.g. `spooling_rows()`.
- `index_conformance`: the `provisional(seed, batch)` fixture
  (`fixtures.rs:205` area) — e.g. `spooling(seed, batch)` — and the case
  `a_provisional_spool_row_becomes_written_when_it_completes`
  (`device_state.rs:324`, listed at `mod.rs:35` and `:98`) — e.g.
  `a_spooling_row_becomes_spooled_when_its_file_completes`. Case meaning and
  coverage are untouched.
- Remaining hits of "provisional" / "pending spool" in
  `spooled_container.rs:28`, `sync/reconcile.rs`, the interruption suites, and
  rustdoc across `index.rs` / the spool steps.

The type `PendingUpload`, the `pending_uploads` table, and
`record_pending_upload` / `pending_uploads()` / `drop_pending_upload` keep
their names — they are the row's *storage* names and renaming them is a
separate decision this task does not take (see Out of scope).

**Plus one stray the refine rounds flagged for a joint fix:** the assertion
message `"nothing was left behind"` in `sync_conformance/import.rs:45` and
`freeze_conformance/import.rs:66` asserts that **surfaced findings** are empty,
but the concept documents assign "leave behind" to a different idea — what an
interrupted run leaves for the next one to clean up. Reword both messages
identically (e.g. `"nothing was surfaced"`); they were deliberately left
symmetric so they must change together.

No file under `docs/spec/` or `docs/concepts/` changes: the canon already says
spool, and registering the *pending row* noun in the concept documents belongs
to the upcoming docs pass, not here.

Conventions per `CLAUDE.md`: English throughout, Conventional Commits,
self-contained commit/PR text, `make check` as the gate.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The state enum is `SpoolState { Spooling, Spooled }` in
      `device_state/spool_state.rs` and no identifier or module named
      `PendingSpoolState` / `pending_spool_state` remains under `backend/`
      (check gates: `pub enum SpoolState` present, `PendingSpoolState` and
      `pending_spool` absent, `make check` compiles every use site).
- [x] The port operation is `Index::mark_spooled`, placed with the same
      update-not-upsert contract `complete_pending_spool` had, and
      `complete_pending_spool` is gone from `backend/` (check gates:
      `fn mark_spooled` present in `index.rs`, `complete_pending_spool`
      absent); the renamed conformance case still round-trips
      Spooling → Spooled, a second completion changes nothing, and completing
      a Container with no row changes nothing.
- [x] The SQLite adapter stores `"spooling"` / `"spooled"` — the literals
      `"writing"` / `"written"` are gone from `coffret-sqlite-index` — and
      `SCHEMA_VERSION` is 4, so an Index file from an older build is refused
      as `UnsupportedSchema` rather than misread (check gates on both).
- [x] The nouns "provisional" and "pending spool" appear nowhere under
      `backend/` (case-insensitive check gate); prose and test names say
      *pending row*, qualified by state where it matters.
- [x] The assertion message `nothing was left behind` appears nowhere under
      `backend/` (check gate); both import conformance cases carry the same
      new wording.
- [x] `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps
      --document-private-items` is clean, so every rustdoc link that named the
      old identifiers was updated rather than left dangling.

## Out of scope

- **Renaming `PendingUpload`, the `pending_uploads` table, or the
  `record_pending_upload` / `pending_uploads()` / `drop_pending_upload`
  operations.** Whether the storage names should also say "pending row" is a
  separate decision; this task only removes the two nouns that had no anchor
  at all.
- **Registering *pending row* (or any vocabulary) in `docs/concepts/` /
  `docs/spec/`** — that is the docs pass's job, queued separately.
- **Any behavior change** beyond the schema-version refusal of older Index
  files: no logic edits in the spool steps, reconcile, upload, or commit.
- **Error-type restructuring** of any kind.
