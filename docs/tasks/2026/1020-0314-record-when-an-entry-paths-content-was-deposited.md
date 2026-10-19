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
branch: task/1020-0314-record-when-an-entry-paths-content-was-deposited
created_at: 2026-10-19T18:14:54Z
updated_at: 2026-10-19T19:40:51Z
---

# feat(format): record when an Entry Path's current content was deposited (`deposited_at`)

## Overview

A future "recently added books" view needs to know when content entered the Library, and that cannot be recovered later for content stored before it is recorded. Today the only times are an Entry's `mtime` and optional `btime`, which describe the original file. Journal records and Index Snapshots carry no time, a Container ID is random (FM-3), and the commit order a Journal's generations give is lost once a checkpoint is adopted and records are pruned.

An Entry is not the right unit to stamp: replacing content and rewriting a Pack both create a new Entry (Entry and Entry Path concepts), and only a rename keeps one. What is wanted is when the content now at an Entry Path entered the Library.

### Decisions (already made; implement them)

1. **Name and definition.** The concept is the *deposit time* of an Entry Path's current content, `deposited_at`: when the content now at that Entry Path was deposited into the Library. A *deposit* is putting content into the Library. Today adding (`add`) and replacing (`update`) are deposits, and each sets a new time. Adding again after a delete is a deposit too. A restore (RV-1) is not a deposit: it rebuilds the Library state from the checkpoint and Journal, and recovers each Entry's `deposited_at` from them exactly as it recovers the current path. Salvage (RV-4) has no records and so no deposit time; content salvaged and put back into a Library is deposited anew. Any future way of putting content in is a deposit as well, with no change to the definition.
2. **Operations that put no content in carry it forward.** Rename, a Pack rewrite (PK-10), absorbing one-file Entries into a Pack (PK-13), and repack / compaction keep the deposit time: a new Entry that carries an existing Entry's content copies that Entry's `deposited_at`.
3. **Catalog layer only.** Add `deposited_at` to the Journal record addition entry (FM-15) and the Index Snapshot entry (FM-16), and to the device's catalog (`coffret-sqlite-index` schema and the other Index implementation). Do **not** add it to the Container meta section (FM-9): that holds what was captured when the Container was written (`captured_*`), and a rewritten Container would mix provenance with a current value. It sits beside `path`, which is also a current value.
4. **Clock and form.** The device that builds the Journal record adding the content stamps it with its own clock, as Unix seconds like `mtime`. No correction for skew between devices. Decide (and state in CP-4) whether a rebased batch keeps its first stamp or restamps: prefer keeping the batch's stamp, so a retry does not move the time, unless the code makes that unnatural.
5. **Required.** The field is required in every addition entry and snapshot entry a writer produces and a reader accepts. Assign new field numbers in `docs/spec/format/field-numbers.json` (the guard checks them) and bump the schema the way the integer-key change did, so a build refuses an older Library rather than reading entries without a deposit time. Existing Libraries are recreated, not migrated.
6. **Not `mtime`.** Spec and concepts say plainly that the deposit time is a separate axis from the original file's `mtime` / `btime`. Avoid "modified" / "updated" / "uploaded" wording for it. Japanese copies of concepts (generated elsewhere) will call it 預け入れ日時.
7. **No UI yet.** Listing / sorting / filtering by deposit time is out of scope. Expose `deposited_at` in the listing API's file rows (RFC 3339 like `mtime`) so a later UI can use it, but do not change the explorer's display.

### Where it touches

- Spec: FM-15, FM-16, `field-numbers.json`, CP-4 (rebase), PK-10 / PK-13 (carrying forward), RV-1 (a restore recovers `deposited_at`; it is not a deposit) and RV-4 (salvage has none), and whatever rule describes `update`. The interop generator and verifier (`coffret-interop`) and the TypeScript format implementation, if it reads these payloads.
- Concepts: Library, Entry / Entry Path, Journal, Catalog, Index Snapshot — add the deposit vocabulary.
- Code: record building in commit, the catalog apply path, snapshot writing and adoption, rename (unchanged time), Pack rewrite and absorb (carry), sync add and update (new time), restore (recovered from the checkpoint and records).

### Tests

- Add and update stamp a new deposit time; rename, Pack rewrite, absorb and repack keep it (both Index implementations).
- A restore / Index rebuild from Storage recovers every Entry's `deposited_at` unchanged.
- A snapshot round-trip and a catch-up from records give the same `deposited_at` for every Entry.
- A record or snapshot entry without `deposited_at` is refused.
- The interop fixtures carry it and the cross-language checks pass.
- The listing API returns it.

### Out of scope

Explorer UI for deposit time, manual correction of times, `mtime` handling.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Every addition and snapshot entry carries a required `deposited_at`, with new field numbers and a schema bump (tests, field-number guard).
- [x] Deposits set a new time and non-deposits carry it forward, as listed above (tests).
- [x] Spec and concepts define the deposit time and keep it separate from `mtime` / `btime` (spec checks pass).
- [x] The listing API exposes `deposited_at` (tests).

### Before merge

- [ ] Recreate the agent's development Library under the new schema, add a fixture book, rename a page, delete another page of the same Pack (forcing a rewrite), update a third, and confirm the deposit times behave as listed, on both devices.
