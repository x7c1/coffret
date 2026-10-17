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
branch: task/1017-2046-fetch-entries-by-number
created_at: 2026-10-17T11:46:41Z
updated_at: 2026-10-17T12:23:42Z
---

# feat(fetch): read an Entry by its entry number, and let rewrites carry renamed Entries

## Overview

The catalog now names every Entry by (Container ID, entry number), applies renames, and keeps each Entry's captured path apart from its current path. Two readers still look an Entry up in a Container's entry table by its **current** path, which a rename makes wrong:

- **Fetch.** `fetch/scatter.rs` (`outline.entry_at(target.path())`, ~line 59) and `fetch/parcel_read/read_entry.rs` (~line 71) find the Entry inside the Container by the catalog's current path. After a rename the meta section still records the captured path, so the lookup misses (or, after a swap, hits the wrong Entry).
- **Read-modify-replace** (`read_modify_replace/carrying.rs:144`): `outline.entries() != self.table` compares the whole meta table with the catalog's rows, so any renamed Entry in a Pack makes every rebuild (a delete's Pack rebuild, PK-10) fail with `Unverified::Disagrees`.

### Changes

1. **Fetch by number.** Every Container read (whole fetch, parcel/range read, the explorer's page reads through `coffret-device`/`coffret-server`) locates the Entry in the entry table by its entry number, never by path, and places the bytes at the catalog's **current** path. The read is still verified against the catalog's hash for that Entry (EP-11); a number outside the table, or a hash that disagrees, is a typed refusal, not a silent fallback to path lookup. Read-ahead caches (the front/outline memory) key Entries by (Container, number).
2. **Rewrites carry renamed Entries.** The read-modify-replace verification compares each catalog row with the meta entry at the same entry number on what a rename cannot change — captured path (`captured_path`), extent and hash — and no longer compares current paths. A rewritten Pack records each kept Entry under its **current** path as its new captured path (rename decision: a rewrite captures the name current at the rewrite). Revise PK-10's "A carried Entry keeps … its Entry Path" accordingly.
3. **Spec and concepts.** EP-11 / PK-10 (and any FM/PK rule that says "looked up by path") say Entries are read by entry number and placed at the current path. Entry and Container concepts drop "fetch does not use entry numbers yet".

### Tests

- Whole fetch and range (parcel) read of a renamed Entry inside a Pack and inside a one-file Container return the right bytes at the current path; a swap (two Entries exchanging names in one record) returns each Entry's own bytes at its new name.
- Multiple zero-length Entries in one Pack, one of them renamed, are each fetched correctly.
- A delete whose Pack rebuild keeps a renamed Entry succeeds; the rebuilt Pack's meta records the kept Entry's current path as its captured path, and the catalog's `captured_path` disappears for it (equal to path).
- A catalog row whose hash disagrees with the meta entry at its number is refused with a typed error.

### Out of scope

Relocating local files, the rename operation (CLI), local rename detection.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Fetch (whole and parcel range) reads Entries by entry number and places them at the current path; the rename, swap and zero-length cases above are covered by tests.
- [x] Read-modify-replace verification compares captured path, extent and hash by entry number, and a delete rebuilding a Pack with a renamed kept Entry succeeds and records its current path as the new captured path (test).
- [x] A hash disagreement or an out-of-range entry number is a typed refusal (tests).
- [x] EP-11 / PK-10 and the Entry and Container concepts describe reading by entry number (spec checks pass).
