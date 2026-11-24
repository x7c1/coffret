---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/1124-1805-close-the-gaps-in-the-tests-of-the-index-reads
created_at: 2026-11-24T09:04:26Z
updated_at: 2026-11-24T10:11:55Z
---

# test: close the gaps in the tests of the Index's bounded reads

## Overview

The Index port has reads bounded to what an operation touches — `present_outside`, `present_in`, the lookup of Containers by ID — and a sync's scan reads through them. Their tests leave gaps through which a wrong read could pass. Apply one rule: **a bounded read of the Index is tested where it is used, against the index that serves the Library.** Line numbers and names are from when the sites were listed; confirm each before changing it.

- **The test wrappers of the Index do not pass `present_outside` and `present_in` on.** The wrappers that stand in front of another index in tests — `sync_conformance/watching_index.rs`, `sync_conformance/refusing_index.rs`, `src/refusing_index.rs`, `commit_conformance/rival_index.rs`, `freeze_conformance/truncating_index.rs`, `tests/place_faults.rs` under `backend/crates/domain/coffret-usecase/`, and `backend/crates/apps/coffret-device/src/reservations/tests.rs` — forward `present_under` to the index they wrap and leave `present_outside` and `present_in` to the trait's defaults. The defaults call the forwarded `present_under`, so the answers are right, but a conformance suite run over the SQLite index through such a wrapper never runs the SQLite index's own SQL for those two reads. Forward both in every wrapper that forwards `present_under`, with whatever the wrapper does to `present_under` (watching, refusing, truncating) done to them in the same way. Search for other wrappers of the same shape (an `impl Index for` that holds another index and forwards to it) and treat them alike; say in your report which you found. If a wrapper overrides other reads that have gained narrow siblings in the same way, forward those too.
- **`present_outside`'s conformance case has no file bearing an excluded folder's name.** `the_present_rows_outside_some_folders_are_the_remainder` (`backend/crates/domain/coffret-usecase/src/index_conformance/device_state/local_entries.rs`) does not hold a file standing directly at the Library root under the very name of an excluded top-level folder. Both indexes leave that record out, as `present_under` takes it in for the folder; add it to the case so the boundary is pinned for both. If the two indexes turn out to differ, the one that matches `present_under`'s boundary is right: fix the other and say so.
- **No sync conformance case runs the scan's read of that one file.** A sync's scan (`sync/scan/deletions.rs`) reads the file standing at a top-level mapping's own prefix by its path (`Read::At`), when a scope of folders holds both the Library root and that mapping's folder; only a unit test of which reads are made covers it. Add a case to the sync conformance suite that runs a scan of such a scope end to end over both indexes, with such a file recorded present and gone from disk, and checks that the deletion is reported once. Follow how the neighbouring cases set mappings, scopes and local records up. If no arrangement a real scan can reach makes that read, say so with the reason rather than building a state the product cannot be in.
- **The lookup of Containers by ID has no query-plan test.** `backend/crates/gateway/coffret-sqlite-index/src/library_state/lookup/containers_among.rs` reads `SELECT * FROM containers WHERE id = ?1`; LA-48 (`docs/spec/loopback-access/README.md`) says a fetch looks its Container up by ID, and its Form note calls that part prose, "honored by construction". Move the SQL into a constant and add a query-plan test in the way the neighbouring plan tests do (the shared `query::plan` test helper; see the plan tests for the beside lookups and for `ANY_REMOTE_IN` / `HELD_PARCELS_OF`), citing LA-48 as those do. Then LA-48's Form note can say test for that part: reword the Form note so it names exactly what is now tested and what remains prose (the reading of every held parcel), changing nothing else in the rule.

Change no behaviour of the product: this change adds and widens tests, forwards calls in test wrappers, moves one SQL string into a constant, and rewords one Form note. Change nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] Every test wrapper of the Index that forwards `present_under` forwards `present_outside` and `present_in` too
- [x] The conformance case for `present_outside` holds a root file named as an excluded folder, and passes against both indexes
- [x] A sync conformance case covers a scan whose scope holds the root and a top-level mapping's folder with a file at the mapping's own prefix, against both indexes (or the report says why no real scan reaches it)
- [x] A query-plan test covers the lookup of Containers by ID and cites LA-48, and LA-48's Form note says test for it

### Before merge (verified outside the check command)

- [x] The PR description lists the wrappers changed, the cases added, and anything the new tests showed about the code
