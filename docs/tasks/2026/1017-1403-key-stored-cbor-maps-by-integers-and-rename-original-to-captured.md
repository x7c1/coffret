---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! git grep -n -E "original_(path|mtime|btime)|original(Path|Mtime|Btime)|ORIGINAL_(PATH|MTIME|BTIME)" -- backend frontend docs/spec docs/concepts scripts Makefile'
assignee: null
branch: task/1017-1403-key-stored-cbor-maps-by-integers-and-rename-original-to-captured
created_at: 2026-10-17T05:03:46Z
updated_at: 2026-10-17T06:36:01Z
---

# feat(format)!: key every stored CBOR map by integers and rename the `original_*` fields to `captured_*`

## Overview

Every encrypted payload coffret puts in Storage is a CBOR map keyed by field-name text strings: the Container meta section (FM-9), the Journal record (FM-15), the Index Snapshot (FM-16) and the Keyring replica (FM-17), and every map nested in them (meta entries, `derived_from`, additions and their entries, Snapshot containers and entries, Keyring `mapping` elements, …). Two consequences have become costly:

- A field's name is its wire spelling, so renaming a field is a format change, and an immutable Container keeps the old spelling forever.
- The key strings are a large share of the Index Snapshot: of the measured 94-byte fixed cost per Entry, the six keys every entry map repeats (`path`, `mtime`, `offset`, `size`, `hash`, `container`) take about 38 bytes, against 6 bytes as small integers.

Text keys bought little: whoever salvages a Library already needs coffret's specification for key derivation, the fixed headers, AEAD and Padmé, and integer keys keep CBOR self-describing (types and structure stay readable, unknown keys can still be skipped). Every existing Library is being discarded, so this is the cheapest moment to change the format, and no reader for the old spelling is kept.

In the same change, rename the provenance fields. `original_path`, `original_mtime` and `original_btime` (and the `original_path` inside `derived_from`) mean "the value captured when this Container was written", not "the first value the Entry had": a content update already writes a new Container under the then-current name, and Pack rewrites (repack, compaction, a delete's rebuild) will record the name current at the rewrite. The word "original" invites the wrong reading. Rename them to `captured_path`, `captured_mtime`, `captured_btime` everywhere they appear as words — specification, concepts, Rust and TypeScript identifiers, SQLite column names, refusal reasons and log fields. Their meaning does not change, and the catalog-layer `path` / `mtime` / `btime` stay as they are. These two changes land together because both break the stored format and both are paid for by the same discarded Libraries; splitting them would mean two incompatible formats in a row.

### Integer keys

1. **One registry is the source of truth.** Add a single machine-readable key registry file (JSON is suggested so both Rust and TypeScript read it without new tooling; choose its location so both workspaces can load it, e.g. under `docs/spec/format/`). Each row names the map it belongs to, the field name, and the integer key, and may carry `"retired": true`. Key spaces are per map kind. Give the most frequent keys 0–23 so they encode in one byte; within that, the assignment is yours — record the reasoning in the PR.
2. **The specification states the rule and points at the registry.** FM-9, FM-15, FM-16 and FM-17 keep naming fields by name in their prose, and say that each field is carried under the integer key the registry assigns it; the registry is normative. Do not keep a hand-copied third table in the spec unless a check keeps it identical to the registry.
3. **Both codecs read and write integers.** Rust (`backend/crates/domain/coffret-format/src/meta/`, `src/control/` including `cbor/fields.rs`, `journal_record/`, `index_snapshot/`, `keyring/`, `payload/`, `wire_catalog_entry.rs`, `wire_container.rs`) and TypeScript (`frontend/packages/domain/format/src/` including `internal/cbor.ts`, `internal/metaEntryMap.ts`, `internal/catalogEntryMap.ts`, `internal/entryFields.ts`, `control/*`). Each implementation declares its keys in code and has a test that loads the registry and asserts its declarations match it, map by map, in both directions (no key missing, none extra).
4. **Refusals and logs still speak names.** A malformed or missing field is reported by its field name (looked up from the key), as today.
5. **Forward-open stays.** An unknown integer key is ignored as an unknown text key is today. A text-string key is no longer any field's spelling: a map whose required fields are carried only under text keys is refused as malformed (missing field), with a test.
6. **Keyring `set_digest`.** The digest is over the deterministic CBOR encoding of `mapping` (RFC 8949 §4.2.1, keys ordered by their encoded bytes). With integer keys that order is the integer order; rewrite FM-17's sentence that says which key sorts first, and update both implementations and their fixtures.
7. **`schema` values.** Whether to bump each payload's `schema` number is your call — the old format is refused either way because its required fields are missing. Record the choice and why in the PR.
8. **Registry guard.** Add a check, wired into `make check`, that compares the registry against its version on the merge base with `origin/main` and fails if any existing row was removed or had its map, name or key changed. Adding rows and adding `"retired": true` are the only edits it allows. Give the check a test that builds throwaway git repositories (a temporary directory) holding each forbidden edit and each allowed one, and wire that test into `make check` too. Within one map, keys must be unique and a retired key must not be read or written (tests).
9. **Debugging aid.** Give `coffret-interop`'s manifest / verify output (or another debugging path that already prints decoded payloads) the field names from the registry, so a dumped payload is readable without looking the numbers up by hand.
10. **Fixtures and budgets.** Regenerate the interop fixtures (`make fixtures` / `make interop`). Re-measure the Index Snapshot per-Entry size in `control/index_snapshot/size_tests.rs`; lower `DESIGN_BUDGET` to what the new encoding warrants and put the before/after numbers in the PR.

### `captured_*` rename

- Spec: FM-9, FM-15, FM-16 and every rule that cites the fields (PK, EP, RV, derived-origin transition, …). Concepts: Entry, Container, Catalog and any other page that names them.
- SQLite Index (`backend/crates/gateway/coffret-sqlite-index/`): rename the column. An Index at an older layout is refused and rebuilt from Storage rather than migrated (the Storage it would be rebuilt from is in the old format anyway); migrations that only served layouts this change makes unreachable may be removed, and the remaining upgrade path must still be tested.
- Rust and TypeScript identifiers, error variants and messages, log fields, test names and fixtures.

### Out of scope

- The `renames` vocabulary, Snapshot `entry` numbers, and anything else of the rename feature: later tasks build on this format.
- Text enum values such as `kind`'s `one-file` / `pack`: they are values, not keys, and stay as they are.
- Device-local files that are not CBOR maps (stored Master Key, token cache): unchanged.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The key registry exists and is the single source the Rust and TypeScript key declarations are tested against, map by map, in both directions.
- [x] Meta section, Journal record, Index Snapshot and Keyring round-trip through Rust and TypeScript with integer keys, and the regenerated interop fixtures verify across both implementations (`make interop`).
- [x] A payload whose required fields are carried under text-string keys is refused as malformed, naming the missing field (tests in both implementations).
- [x] An unknown integer key is ignored (test), and a retired key is neither read nor written (test).
- [x] `set_digest` is computed over the integer-keyed deterministic encoding in both implementations, and FM-17 states the order accordingly.
- [x] The registry guard rejects a removed row and a changed key, name or map, and accepts an added row and an added `"retired": true`, each shown by a throwaway-repository test run from `make check`.
- [x] The Index Snapshot per-Entry size test passes under the lowered `DESIGN_BUDGET`.
- [x] No `original_path` / `original_mtime` / `original_btime` (or camel/upper-case forms) remains in `backend`, `frontend`, `docs/spec`, `docs/concepts`, `scripts` or the `Makefile` (grep gate in `check_command`).
- [x] An Index at the previous layout is refused and rebuilt rather than migrated (test).

### Before merge (verified outside the check command)

- [ ] On fresh agent MinIO Libraries (`coffret-agent`, `coffret-agent-2`, re-created with `init` / `join` after this change): a sync and a freeze commit, the second device catches up and fetches a page, and a payload dumped through the debugging aid shows field names.
