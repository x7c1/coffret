---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1018-0234-name-what-restore-and-salvage-recover
created_at: 2026-10-17T17:34:59Z
updated_at: 2026-10-17T17:50:43Z
---

# docs(recovery): say what names a restore and a salvage recover now that Entries can be renamed

## Overview

Renames are now written by `coffret rename` and by sync's detection of a local rename. They are applied on replay (RV-5) and carried by Index Snapshots. A Pack rewrite records the name current at the rewrite as its captured path. Two places still say nothing about names under rename:

- **RV-4 (salvage).** A Container records each Entry's captured path. Renames live only in the Journal. So salvage can only present the names the Containers were written under — and it must never present a current name it cannot prove. RV-4 does not say this.
- **The concept documents.** These were edited by several changes in turn. They may still carry wording that predates renames being written: "planned", "not yet", "nothing writes renames", or the word *move* for a rename or relocation.

### 1. Spec

- **RV-4.** Add a sub-bullet saying that salvage presents each Entry under the captured path its Container's meta section records (FM-9).
  - Since renames are carried only by Journal records and Index Snapshots, salvage cannot know a current name and never presents one as current.
  - A Pack rebuilt after a rename records the name current at the rebuild (PK-10), so salvage of that Pack shows that name.
  - Keep "salvage is not a restore".
- **RV-1 and RV-5.** Make sure they say a restore recovers each Entry's current path, renames included. Say it once, where it belongs, and link from the other.

### 2. Concepts

Audit every concept document under `docs/concepts/` and the spec READMEs for wording that no longer matches what is implemented. In particular:

- **Status statements.** Rename "planned", "not yet written", or "the explorer will …". The explorer's Rename and Move-to actions are still not offered and stay out of scope; say so where relevant.
- **Salvage descriptions.** Library, Storage, Journal, Catalog, Pack, Index Snapshot and the concepts README should agree with RV-4's name rule.
- **Vocabulary.** *rename* for the Library operation, *relocate* for a device's file following one, *discard* for the desktop's trash, *re-key* for a record. Never *move* for any of them.

Fix what is stale. Do not restate rules already stated elsewhere.

### 3. Tests

RV-4 is `(Form: test)`. Add a test, at the Container reading level, that pins the name rule salvage depends on:

- After an Entry is renamed in the Library, reading its Container's meta section still yields the captured path, not the current one.
- After a Pack rebuild that keeps a renamed Entry, the rebuilt Container's meta records the current name.

If equivalent tests already exist, cite them from the spec instead of duplicating them.

### Out of scope

A salvage operation itself (none exists), the explorer's Rename / Move-to actions, changes to behaviour.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] RV-4 states that salvage presents captured paths and never a current name, and how a rebuilt Pack's names relate. RV-1 or RV-5 states that a restore recovers current paths, renames included.
- [x] A test pins that a renamed Entry's Container still reads under its captured path, and that a rebuilt Pack records the current name (or the spec cites existing tests that do).
- [x] No concept document or spec README describes rename as planned or unwritten, or uses *move* for a rename, a relocation or a discard (spec checks pass).
