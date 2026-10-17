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
branch: task/1017-1638-carry-renames-and-snapshot-entry-numbers
created_at: 2026-10-17T07:38:30Z
updated_at: 2026-10-17T08:38:44Z
---

# feat(format): carry renames in Journal records and entry numbers in Index Snapshots

## Overview

The catalog is the authority for Entry Paths: a rename changes an Entry's current path in the catalog and never rewrites, re-encrypts or re-uploads its Container. To get there the catalog needs a way to name an Entry that does not depend on its name, and the Journal needs a vocabulary that carries a rename. This task lands both in the format, in the specification and in the Rust and TypeScript codecs. Applying renames to the Index, fetching by entry number, rebasing and the local follow-up are later changes; nothing in this task writes a `renames` element yet except tests and fixtures.

### Entry numbers

- **FM-9.** A Container's entry table is an ordered array whose order never changes once written. An Entry's **entry number** is its 0-based position in that table: a u32, scoped to one Container, assigned gap-free in the order the table was written (a Pack in its freeze's Entry Path order, a one-file Container always 0). `(Container ID, entry number)` names exactly one Entry. A reader refuses a reference to a number outside the table. State this in FM-9 without adding any field to the meta section.
- **FM-15 additions** already keep the order of the Container's entry table, so an addition's entries carry their numbers implicitly by position. Say so in FM-15; add no field.
- **FM-16 Snapshot entries** are in canonical Entry Path order, so the position is lost there. Add `entry` (the entry number, u32) to each Snapshot entry beside `container`, register its field number, and bump the Snapshot `schema` to 3. A reader that meets a Snapshot of schema 2 does not use it for entry numbers: it rebuilds the catalog by replaying every Journal record from `head-0` (records are never pruned yet, so this is exact). It never guesses a number from path order. Test both paths.

### `renames` in the Journal record

- **FM-15** gains a fourth vocabulary beside `additions`, `removals` and the Keyring commitment: `renames`, an array (absent or empty when the record renames nothing). Each element is a map of `container` (16-byte Container ID), `entry` (entry number), `path` (the new current Entry Path, NFC per EP-1). The old name is not carried: it is the catalog's. Register the new map kind and its field numbers in `docs/spec/format/field-numbers.json` (append rows only; the guard enforces it).
- **Order and shape rules, enforced by the decoder** (refuse rather than repair, like FM-15's existing orders): elements are ordered by (Container ID as 16 raw bytes, entry number) ascending; no two elements share (Container ID, entry number); `path` is a valid Entry Path (EP-1, EP-2); `path` does not lie under the reserved `.coffret-derived/` component, and the renamed Entry is not one (derived Entries are not renamed by this version — their follow-up is decided when derived Entries are implemented).
- **State rules, specified here and enforced where the record is applied (a later change):** each element names a Container that is current immediately before the record and is not among the same record's `removals`, and an entry number inside its table; its `path` differs from the Entry's current path; the state after applying the record — removals taken out, all renames applied at once, additions added — satisfies Entry Path uniqueness (EP-5), so a swap of two names is one valid record. A record's own additions write their own `path` and are never targets of its renames. Renaming an Entry back to its captured path makes `captured_path` disappear from the catalog by FM-15's present-iff-differs rule.
- Bump the Journal record `schema` to 3, and keep readers accepting schema 2 records (they simply carry no `renames`) — those exist in Libraries created since the field-number format.
- **RV.** Replay applies `renames` (state the rule; the implementation lands with the Index change).
- **Concepts.** The Journal concept lists four vocabularies; Catalog and Entry concepts describe the entry number as the name-independent reference. Keep "planned" where behaviour (applying, fetch by number, the rename operation) is not implemented yet.

### Codecs and fixtures

Rust (`coffret-format`: `control/journal_record/`, `control/index_snapshot/`, `control/wire_catalog_entry.rs`, the field-number registry module) and TypeScript (`frontend/packages/domain/format/src/control/`) encode and decode `renames` and the Snapshot `entry`, with round-trip and rejection tests on both sides, and the interop fixtures carry a record with renames and a schema-3 Snapshot so `make interop` checks both implementations agree.

### Out of scope

Applying renames to the SQLite Index, fetch by entry number, rebase rules, local relocation, the rename command, local rename detection, restore/salvage wording beyond RV's replay rule.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] FM-9 defines the entry number and FM-15 states that additions carry it by position; FM-15 defines `renames` with its element map, order, shape rules and state rules; FM-16 defines Snapshot `entry` and schema 3 (spec checks pass in `make check`).
- [x] The field-number registry gains rows for `renames`, the rename element map and the Snapshot `entry`, and the registry guard accepts them as appended rows.
- [x] Rust and TypeScript round-trip a Journal record carrying `renames` and a schema-3 Snapshot carrying `entry`, and refuse: out-of-order elements, a duplicate (Container, entry), an invalid or non-NFC `path`, a `path` under `.coffret-derived/`, a Snapshot entry without `entry` at schema 3 (tests on both sides).
- [x] A schema-2 Journal record still decodes (no `renames`), and a schema-2 Snapshot is not used for entry numbers: the reader falls back to replaying every Journal record (tests).
- [x] The interop fixtures include a record with renames and a schema-3 Snapshot, and `make interop` verifies both implementations against them.
