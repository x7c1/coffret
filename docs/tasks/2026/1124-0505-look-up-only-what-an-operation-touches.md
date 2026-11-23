---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, spec-alignment, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/1124-0505-look-up-only-what-an-operation-touches
created_at: 2026-11-23T20:05:16Z
updated_at: 2026-11-23T20:48:27Z
---

# perf: look up what a commit, a deletion, a move or a sync touches instead of reading the whole Library

## Overview

Several everyday operations read every Entry or every Container of the Library — `entries_under(None)` and `containers_under(None)` — and then keep a handful. A Library of scanned books holds 100,000 to 1,000,000 Entries, so each of them costs time in proportion to the whole Library, however little it touches. `containers_under(None)` is no cheaper than reading the Entries: it joins every Entry and takes the distinct Containers (`backend/crates/gateway/coffret-sqlite-index/src/library_state/lookup.rs`), so it grows with the Entries, not with the Containers.

Apply one rule to every site below: **an operation reads only what it touches** — a point lookup by path (`entry_at`), a lookup by Container id, a range under the folder the operation is about (`entries_under(Some(folder))`, `entries_in`), `EXISTS … LIMIT 1` for "is there any", `COUNT(*)` for a count — never the whole Library to keep a few rows. The indexes these lookups need are there (the `entries.path` primary key, `entries_by_container`, `local_entries_by_entry`, and the `parent` index on `entries`); add a narrow Index port method where none answers the question, with a default implementation on the trait where the in-memory index can keep reading everything. Line numbers below are from when the sites were listed; confirm each before changing it.

- **Commit checks.** `domain/coffret-usecase/src/commit/candidate.rs` reads every Entry into a set of paths (around line 41) and calls `containers_under(None)` (around lines 102 and 157); `commit/rebase/check_contested.rs` does the same (around line 39). Look up each path the commit adds or renames to with `entry_at`, and each Container it names by id.
- **Deletion and move.** `delete/plan.rs` reads every Entry and every Container even for one file (around lines 142 and 151). The move's confirmation and execution do the same (`rename/plan.rs` around line 106, `rename/rules.rs` around line 32), and so does the sync's rename detection (`rename/refuse_detected.rs` around line 39). Read the Containers a deletion touches with `entries_in`, and the Entries a move carries with `entries_under(Some(from))`.
- **`mapped` alone.** The drop, the import, the freeze and the fill call `library.list(folder)` whole only to read `mapped` (`coffret-server/src/routes/upload/mod.rs` around line 276, `upload/import/mod.rs` around line 106, `routes/freeze.rs` around line 113, `fill/run.rs` around line 59). Answer `mapped` from the mappings alone.
- **The sync's scan.** `sync/scan/mod.rs` calls `containers_under(None)` (around line 96), and `sync/scan/deletions.rs` reads the whole mapping prefix with `present_under` and then throws away what lies outside the scope (around lines 94–108). Read per folder in scope.
- **The next Keyring generation.** `commit/keyring/replicate.rs` lists the current Containers with `containers_under(None)` (around line 165). Read the `containers` table directly. What is written to Storage does not change.
- **Settle.** `sync/settle/complete.rs` reads every Entry when pending rows remain (around line 77). `entries_in` answers it.
- **Catch-up and Refresh.** `catch_up/run.rs` reads every Entry twice only to count them (around lines 58 and 80). Count them.
- **The remote-head poll.** `catch_up/remote_head.rs` calls `containers_under(None)` on every poll (every three minutes, and whenever the page comes to the front) (around line 71).

Do not change what any operation decides, refuses or writes: the same checks, the same refusals (spec rules such as EP-5, EP-6, OC-*, PK-*, KL-*), the same Storage writes. Existing tests pin that and must keep passing unchanged. Where a site turns out to need a whole-Library read for correctness, keep it, and say why in a comment and in your report. Where a rule in `docs/spec/` describes how one of these operations reads, keep it true; do not add or remove behavioural rules.

Do not touch the reader and explorer paths (page reads and held parcels, the fill's "is anything missing", `/api/beside`, `/api/filter`, the cover read-ahead), the listing routes, the Keyring's Storage format, or the Index Snapshot policy; other changes handle them. Change nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] No `entries_under(None)` or `containers_under(None)` call is left at the sites listed in the Overview, outside tests (a `grep` over those files finds none), except a site kept with a comment saying why it needs the whole Library
- [x] Each new Index port method has a conformance case run against both the in-memory and the SQLite index

### Before merge (verified outside the check command)

- [x] The PR description lists each site, what it reads now, and any site kept as it was with the reason
