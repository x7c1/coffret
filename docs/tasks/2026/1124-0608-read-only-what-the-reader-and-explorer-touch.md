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
branch: task/1124-0608-read-only-what-the-reader-and-explorer-touch
created_at: 2026-11-23T21:08:03Z
updated_at: 2026-11-23T22:00:17Z
---

# perf: read only what a page read, the reader and the explorer touch

## Overview

Reading a page and moving through the explorer still read every Entry or Container of the Library, or of a whole subtree, to keep a few rows. With 100,000 to 1,000,000 Entries in a Library of scanned books, each of these costs time in proportion to the whole Library or subtree, however little it touches. Apply one rule to every site below: **read only what the operation touches** — a lookup by id or by key, a range bounded by `LIMIT 1`, or `EXISTS … LIMIT 1` for "is there any" — never a whole Library or subtree to keep a few rows. Reuse the Index port methods already on `main` where they answer the question: `containers_among`, `containers`, `entry_count`, `folder_contents`, `library_summary` and `folders`. Add a narrow port method where none answers it, with a default implementation on the trait that the in-memory index can keep, and a conformance case run against both indexes. Line numbers below are from when the sites were listed; confirm each before changing it.

- **A page read with held parcels.** Whenever a parcel is held, each page read from Storage calls `containers_under(None)` (`domain/coffret-usecase/src/fetch/entry_run/mod.rs` around line 138, `let_go.rs` around lines 33–41). So does the listing cache when it misses (`fetch_memory/listing.rs`), where it still asks the whole Library. Look up only the held parcels' Containers by id.
- **`held_parcels()`.** It reads every row, then filters in memory (`parcel_read/read_entry.rs` around line 75). Look up by Container id.
- **The fill's "is anything missing".** `fill_unless_whole` reads every missing Entry under the folder only to see whether there are none (`coffret-server/src/routes/file.rs` around line 314, through `entries_absent_under`). Ask `EXISTS … LIMIT 1`.
- **`/api/beside` without `under`.** It reads every Entry (`coffret-device/src/browse/beside.rs` around line 37). Find the previous and next Entry through the `path` index with `LIMIT 1`, keeping the same order (EP-3), the same answer, and the same handling of what the reader skips.
- **The cover read-ahead after each listing.** It reads every Container of the subtree with `containers_under(folder)` (`domain/coffret-usecase/src/fetch/fronts_ahead.rs` around lines 84–89). Look the Containers up by id: the ids of the folder's own remote files, which `remote_in` already answers.
- **The explorer's `treeRows`.** Inside its loop over child folders it calls `listing.folders.some(...)`, which is O(k²) in a folder's k children (`frontend/packages/apps/web/src/tree.ts` around line 265). Build a set once per listing.

Change nothing that any of these reads answers or decides: the same pages, the same rows and order, the same refusals and findings. The existing tests pin that and must keep passing unchanged. Where a site turns out to need the wider read for correctness, keep it, and say why in a comment and in your report. Where a rule in `docs/spec/` describes one of these reads, keep it true. Do not add or remove behavioural rules.

Do not touch `/api/filter`, the Keyring's Storage format or the Index Snapshot policy; later changes handle them. Change nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] Outside tests and trait defaults, no `containers_under(None)`, `entries_under(None)` or whole-table `held_parcels()` read is left at the sites listed in the Overview (a `grep` over those files finds none), except a site kept with a comment saying why
- [x] Each new Index port method has a conformance case run against both the in-memory and the SQLite index
- [x] A web test pins `treeRows`' output for a folder with many child folders

### Before merge (verified outside the check command)

- [x] The PR description lists each site, what it reads now, and any site kept as it was with the reason
