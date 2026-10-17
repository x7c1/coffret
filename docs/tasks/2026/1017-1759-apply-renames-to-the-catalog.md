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
branch: task/1017-1759-apply-renames-to-the-catalog
created_at: 2026-10-17T08:59:15Z
updated_at: 2026-10-17T10:19:22Z
---

# feat(index): apply a Journal record's renames to the catalog, atomically and only when every state rule holds

## Overview

Journal records can now carry `renames` (FM-15) and the Index stores each Entry's entry number, but nothing applies a rename yet: replay ignores the vocabulary. This change makes every place a record is applied to the catalog — `Index::apply` in both implementations (`InMemoryIndex` in `coffret-usecase` and the SQLite Index in `coffret-sqlite-index`), and therefore catch-up and restore — honour `renames`, so a device that replays a record carrying renames ends with the Entries at their new current paths. Nothing writes renames yet; tests build records directly.

### Rules to implement (FM-15 state rules, RV-5)

- A record is applied as one unit: removals taken out, then all of its renames applied at once, then its additions added. If any state rule fails, the whole record is refused and the catalog is left exactly as before (SQLite: one transaction rolled back; in-memory: no partial mutation). The refusal is a typed error naming which rule failed and which element (by its position in `renames`).
- Each rename element names a Container that is current immediately before the record and is not among the same record's removals, and an entry number inside that Container's entry table.
- The element's `path` differs from the Entry's current path.
- The renamed Entry is not a derived Entry (`derived_from` present or its current path under `.coffret-derived/`): refuse (derived Entries are not renamed in this version).
- After removals, renames and additions, Entry Path uniqueness (EP-5) holds across the whole catalog; renames apply simultaneously, so swapping two names in one record is valid, and two elements may not land on the same path.
- A rename changes the Entry's current path only. Its captured path stays the Container's record, so the catalog's `captured_path` for that Entry is present exactly when the new current path differs from it (renaming back to the captured path drops it; renaming away from it adds it).
- Device-local state that follows a path (materialization records, pending rows keyed by path, held parcels if keyed by path — check each) must not silently point at a stale path after a rename is applied. Relocating local files is a later change; here, decide per table whether it is re-keyed with the Entry, left keyed by (Container, entry number), or explicitly marked as awaiting relocation, and test that a later sync does not mistake an applied rename for a local deletion plus a new file. If a table cannot be handled without the relocation work, keep it consistent (no orphan rows) and say so in the PR.

### Tests

- Index conformance (run against both implementations): a single-file rename, a rename inside a Pack, a folder's worth of renames in one record, a swap, rename back to the captured path, rename away from it; and refusals with the catalog unchanged for each failed rule: Container not current, Container removed in the same record, entry number out of range, unchanged path, derived Entry, destination collides with an existing Entry, two renames to one path, rename colliding with the same record's addition.
- Replay: a new device replaying the full Journal and one adopting a schema-3 Snapshot followed by later records with renames reach the same catalog (current paths and captured paths).
- Concepts: Catalog and Journal concept docs drop "planned" for applying renames; Catalog gains the collocation "rename (an Entry, changing its current Entry Path)". The Index concept mentions that the Index keeps entry numbers.

### Out of scope

Writing renames (the CLI operation), rebase rules for concurrent commits, fetch by entry number, relocating local files, local rename detection.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Index conformance cases for each successful rename shape above pass against both the in-memory and the SQLite Index, with current and captured paths checked.
- [x] Each state-rule violation above is refused with a typed error naming the rule and the element, and the catalog (and device-local tables) is unchanged afterwards, in both implementations.
- [x] Full-Journal replay and Snapshot + later records reach the same catalog when the later records carry renames (test).
- [x] Device-local state after an applied rename is consistent: a sync immediately afterwards reports no local deletion and adds no new file for the renamed Entry when its local file is untouched (test), or the PR states which table awaits the relocation work and a test pins that it holds no orphan rows.
