---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, schema-design, spec-alignment, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/1124-0338-answer-folder-listings-from-per-folder-totals
created_at: 2026-11-23T18:38:16Z
updated_at: 2026-11-23T19:47:50Z
---

# perf(index): answer a folder's listing from per-folder totals instead of reading its whole subtree

## Overview

Opening a folder in the explorer costs time in proportion to every Entry under it, at any depth. `OpenLibrary::list` (`backend/crates/apps/coffret-device/src/browse/list.rs`) reads `entries_under(folder)`, `present_under(folder)` and `containers_under(folder)` for the whole subtree, only to count, for each child folder, how many files stand under it, how many this device has, their size and the newest `mtime`. A Library of scanned books reaches 100,000 to 1,000,000 Entries, so the root listing, or any folder high up, reads all of them, and the explorer shows `loading…` each time. The same whole-Library reads stand behind three other routes:

- `GET /api/library/totals` (`browse/totals.rs`) reads every Entry, every present record and every Container. It is asked again whenever the root listing changes, and during a fill on every advance of the `/api/work` poll.
- `GET /api/folders` (`browse/folders.rs`) reads every Entry to derive the folder list, and is asked again on every sync, freeze, Refresh and catch-up.

Make these cost no more than the number of rows they answer with, times log N, by keeping per-folder totals in the Index. The design decisions already made for this (follow them; record any further judgement and its reason in the PR description):

1. **A per-folder totals table in `coffret-sqlite-index`.** One row per folder that has an Entry under it at any depth (the Library root included, as its own row), holding at least: the number of Entries under it at every depth, how many of them this device has (by the same present materialization records `list` reads today, spec EP-10), their total size, the size of the present ones (which `totals` reports as `size_present`), and the newest `mtime`. Add whatever else the listing needs (for example the number of Containers, if `totals` is to read it from here rather than from a `COUNT(*)` on `containers`).
2. **Kept up to date in Rust, not by SQLite triggers.** Every write of `entries` and of the present records (commit apply, rename/relocation, removal, Snapshot restore, materialization records being written, changed or dropped) updates the totals of every folder above the path it touches, in the same transaction as the write.
3. **Leaks are caught by a conformance test.** Add a test, run over the index conformance suite or the SQLite index's own tests across the write paths (commit, rename, removal, restore, fetch/present changes), that recounts every folder's totals from `entries` and the present records and compares them with the table.
4. **O(N) is what must go; O(1) is not required.** A listing enumerates its child folders and its own files by skipping through the `path` primary key (seek to the first path after the folder's prefix, take the child's name, then seek past everything under that child), not by reading the subtree, and reads each child folder's figures from the totals table.
5. **`mtime` after a removal** is recomputed from an index on the parent folder and `mtime` (add the column and index the recomputation needs, for example a `parent` column on `entries` with an index on `(parent, mtime)`, combined with the child folders' rows of the totals table), not by rescanning the subtree.
6. **`/api/library/totals` and the root listing** answer from the root's row of the totals table; **`/api/folders`** answers from the totals table's rows (every folder with an Entry under it), in EP-3 order.
7. **The totals are a local derivation.** They go into neither Storage nor the Index Snapshot, and do not change any FM-16 encoding. They are built when the catalog is built (including restore from a Snapshot and replay from the Journal) and by the schema migration: bump the Index layout, and have the upgrade build the table from the rows the file already holds — keeping the catalog and the device-local group, not discarding them. Explain the new layout in the `SCHEMA_VERSION` doc comment the way earlier layouts are explained.

Write the behaviour into the spec: a rule (in the loopback-access spec, beside the listing routes, or wherever the listing and totals routes are specified) that the cost of `GET /api/list`, `GET /api/library/totals` and `GET /api/folders` does not grow with the number of Entries below the folder beyond what the answer itself holds (rows answered, times log N), and the response of `/api/list` as it is (child folders with their counts, size, `mtime` and whether they are present throughout; the folder's own files; `mapped`; `held`; the empty mapped top-level folders at the root), if no rule specifies it yet. Cite the new rule IDs from the code where the code implements them. Do not change what any of these routes answer: the same figures, the same order, the same refusals. Existing route and conformance tests pin that and must keep passing unchanged.

The listing route reads two more things per folder opened that also read the whole subtree, and they are part of this change: `OpenLibrary::added_locally` (`backend/crates/apps/coffret-device/src/add/added_locally.rs`), which `GET /api/list` calls after the listing and which reads `entries_under(folder)` only to keep the folder's own files and its child folders' names; and `OpenLibrary::remote_in` (`backend/crates/apps/coffret-device/src/browse/remote_in.rs`), which the read-ahead of covers started for each listing (`read_fronts_of`) and `directly_present` call, and which reads `entries_absent_under(folder)` only to keep the Entries standing directly in the folder. Answer both from what stands directly in the folder (the per-folder totals and the parent index), with the same answers as before.

Do not touch the other whole-Library reads (commit checks, delete and move planning, sync scan, catch-up counts, `/api/beside`, `/api/filter`, the reader's held-parcel reads, the Keyring); later changes handle them. Change nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] A test recounts every folder's totals from `entries` and the present records after commits, renames, removals, a restore and changes to present records, and finds them equal to the totals table
- [x] `OpenLibrary::list`, `totals`, `folders`, `added_locally` and `remote_in` no longer call `entries_under`, `entries_absent_under`, `present_under` or `containers_under` (a `grep` over `backend/crates/apps/coffret-device/src/browse/{list,totals,folders,remote_in}.rs` and `backend/crates/apps/coffret-device/src/add/added_locally.rs` finds none)
- [x] The Index upgrade from the previous layout builds the totals table from the rows the file holds, and a test opens a previous-layout file and finds its catalog and device-local rows kept and its totals equal to a recount
- [x] The spec has a rule on the cost of the listing, totals and folders routes, and `make check`'s spec-citation and rule-ID checks pass

### Before merge (verified outside the check command)

- [x] The PR description lists the design judgements made beyond the decisions in this task, with their reasons
