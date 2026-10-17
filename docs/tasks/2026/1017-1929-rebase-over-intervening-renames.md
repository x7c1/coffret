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
branch: task/1017-1929-rebase-over-intervening-renames
created_at: 2026-10-17T10:29:28Z
updated_at: 2026-10-17T11:36:18Z
---

# feat(commit): rebase pending changes over intervening renames and surface the conflicts they cause

## Overview

When a commit loses its slot, the writer refreshes the head, rebases its batch onto it and retries (CP-4); two writes to one Entry Path become an explicit conflict, never last-write-wins (CP-7, EP-7). Journal records can now carry `renames` and the catalog applies them, but rebase does not know about them yet: a batch prepared against a name that another device has since renamed would commit against a stale name, or collide silently. This change teaches rebase the rename rules. Nothing in the product writes renames yet; tests build rename records and rename batches directly, so the rules are in place before the rename operation ships.

### Rules (specify in CP-4 / CP-7 / EP-7, implement in the commit usecase)

A pending change remembers the current name it was prepared against. On rebase, every record between the batch's base and the new head is examined, and its renames are applied to the pending change by (Container ID, entry number):

1. **Pending addition vs intervening rename onto its path** — a rename that gave another Entry the path the addition wants is a conflict (EP-7), as an intervening addition at that path already is.
2. **Pending update (replacing Container C at its Entry's path) vs intervening rename of that Entry** — the update follows the rename: it is committed as the replacement of the same Entry at its new current path. The new Container's captured path stays the name the update was prepared under (it is what its meta section records).
3. **Pending rename vs an intervening update that replaced the renamed Entry's Container** — if the name the rename was prepared against now names the update's new Entry (D, m), the rename is retargeted to (D, m) and committed; otherwise conflict.
4. **Pending rename vs intervening rename of the same Entry** — conflict.
5. **Pending rename vs an intervening addition or rename that now holds the destination path** — conflict.
6. **Pending removal / delete rebuild vs intervening rename of an Entry the rebuild keeps** — the rebuilt Pack is kept (no re-upload); the record carries the kept Entry under its renamed current path.
7. **Pending removal vs intervening rename of a removed Entry** — the removal still applies (it names the Container), and nothing else changes.
8. **Folder renames are not extended**: a pending rename enumerates exactly the Entries it was prepared with; Entries added under the old folder by intervening commits stay where they are. Pending additions are never remapped by prefix.

Conflicts are typed (which pending element, which intervening record and rule), and the batch is not committed; existing conflict surfacing (CLI and server) carries them. Every rule above is tested with the batch prepared, a rival commit applied first, and the rebase run — and, where both orders are possible, in both commit orders.

### Concepts

The Journal and Catalog concepts (and the Library concept where it describes concurrent writers) state the rename rebase rules at the concept level; keep "planned" for the rename operation itself.

### Out of scope

The rename operation (CLI), fetch by entry number, relocating local files, local rename detection.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] CP-4 / CP-7 / EP-7 state the eight rules above (spec checks pass).
- [x] A commit-usecase test per rule 1–8 builds the pending batch, commits a rival record first, and checks the rebase outcome: the follow/retarget results land in the committed record (paths, captured path, entry numbers), and each conflict is a typed refusal with nothing committed.
- [x] Rules 2, 3 and 6 are also verified with the two commits in the opposite order, reaching the same catalog.
- [x] The rebased record for rule 6 references the already-uploaded Pack (no second upload, checked with the in-memory store's put counter).
