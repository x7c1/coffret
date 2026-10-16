---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1016-1643-carry-each-entrys-original-path-in-the-catalog
created_at: 2026-10-16T07:43:33Z
updated_at: 2026-10-16T10:33:25Z
---

# feat(catalog): carry each Entry's original path in the catalog, the Journal record and the Index Snapshot

## Overview

The catalog names an Entry by its current Entry Path (`path`), and a Container's
meta section records the path the Entry had when the Container was written
(FM-9's `original_path`). Today the two are always equal, because nothing can
rename an Entry yet. Rename (the catalog is the authority for Entry paths, a
rename travels in the Journal, `original_path` stays as provenance) will make them
differ, and once they differ a device can only learn an Entry's original path
by fetching and decrypting the Container's meta section. Carry it in the
catalog instead, before rename lands, so every device can list both names from
the catalog alone.

- **Format (FM-15, FM-16).** Each entry map in a Journal record's `additions`
  and in an Index Snapshot's `entries` gains `original_path`: the Entry Path
  FM-9 records in the owning Container's meta section for that entry. Adding
  a field increments `schema` (FM-15's forward-open rule): writers write the
  new `schema`; readers accept the old one and, for a map that lacks
  `original_path`, take it to equal `path` — true of every record and Snapshot
  written so far, since no rename existed. Writers write the field only where
  it differs from `path`, so an unrenamed Entry costs nothing more in a
  Snapshot, and a reader refuses a map carrying it equal to `path` (one state,
  one encoding). Say that in a sub-bullet. Keep rule
  IDs stable. Update the FM-15 sub-bullet on the `original_` prefix so it
  explains why one map now carries both spellings (`path` is what the Library
  holds now, `original_path` is the Container's provenance), instead of saying
  the prefix is dropped.
- **Writers.** A commit fills `original_path` from the Container it adds (the
  entry table it just wrote), not from the batch's current paths. A checkpoint
  copies it from the catalog.
- **Catalog (SQLite Index).** The `entries` table gains an `original_path`
  column (canonical Entry Path, `COLLATE BINARY`, NOT NULL). Bump the schema
  version. Fill existing rows with their current `path` in place (an explicit
  transactional upgrade in the style of the existing device-state upgrade),
  rather than discarding the catalog and making every device catch up from
  Storage again. A replay and a Snapshot load fill the column from the record
  / Snapshot.
- **Model and ports.** The model's catalog Entry (and whatever the usecase
  hands the catalog) carries `original_path` as an Entry Path type, not a
  string. Name it consistently with FM-9's `original_path` and the concept
  docs.
- **Where it shows.** Nothing in the explorer or CLI needs to display it yet.
  Where a JSON answer already describes an Entry in full (e.g. a CLI `--json`
  listing), decide whether to include it; if included, document it.
- **Concepts.** The Catalog concept (`docs/concepts/catalog/`, and the Index,
  Index Snapshot and Journal concepts where they list an Entry's fields) says the catalog keeps each Entry's original path beside
  its current one, and why.

Out of scope: rename itself (`renames`), any UI that shows the original path.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A Journal record's and an Index Snapshot's entry maps carry
      `original_path` exactly where it differs from `path`; round-trip tests
      cover both forms, a map without it (including one of the previous
      `schema`) decodes with `original_path` equal to `path`, and a map
      carrying it equal to `path` is refused (Rust and TypeScript).
- [x] The Snapshot's per-Entry size budget is unchanged
      (`index_snapshot/size_tests.rs`).
- [x] A commit writes `original_path` from the Container's own entry table; a
      test shows the record's value matches the meta section's.
- [x] The SQLite catalog has an `original_path` column; a test upgrades a file
      at the previous schema version holding Entries and finds every row's
      `original_path` equal to its `path`, with device-local state preserved.
- [x] Replay of a record and load of a Snapshot fill the column; a test reads
      it back through the Index port.
- [x] FM-15 and FM-16 carry the new field and the fallback rule, rule IDs
      unchanged (`make check` runs `spec-rule-ids` and `spec-citations`).

### Before merge (verified outside the check command)

- [ ] On the agent's MinIO Library, a device running this build upgrades its
      existing catalog without a catch-up from Storage, and a new drop commits
      a record whose entries carry `original_path`.
