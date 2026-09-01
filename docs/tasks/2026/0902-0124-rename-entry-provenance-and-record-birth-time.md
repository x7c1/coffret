---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'original_path' docs/spec/format/README.md && grep -q 'original_btime' docs/spec/format/README.md && grep -rq 'original_btime' frontend/packages/domain/format/src"
assignee: null
branch: task/0902-0124-rename-entry-provenance-and-record-birth-time
created_at: 2026-09-01T16:24:00Z
updated_at: 2026-09-01T18:55:20Z
---

# feat(format): rename entry provenance fields and record birth time

## Overview

A Container's entry table (FM-9, `docs/spec/format/README.md`) records each
Entry's `path` and `mtime` under names that read like current attributes of
the Entry. They are not: a Container is an immutable object — rewriting its
meta section would mean re-encrypting and re-uploading it — so these values
are facts captured once, when the Container was written, while the current
Library state is defined by the Journal and its checkpoint (EP-11 already
verifies a fetched Entry against "what the current catalog records for it").
Ahead of introducing catalog-level renames, make that provenance explicit in
the vocabulary, and capture one more creation-time fact while capture is
still free:

1. **Rename the meta-section entry keys to provenance names.** In the meta
   section's entry map: `path` → `original_path`, `mtime` → `original_mtime`,
   and inside `derived_from`: `path` → `original_path`. The meaning is "the
   Entry Path / modification time at the moment this Container was written" —
   not the first name the Entry ever had: a content update writes a new
   Container, so successive Containers for one Entry may carry different
   `original_path` values. **Journal record and Index Snapshot entries keep
   `path` and `mtime`** — they are the catalog's durable form, where an
   addition's values establish the current state. FM-15's "each element
   exactly FM-9's entry map" sharing therefore loosens to: the same map,
   except the provenance keys are spelled `original_*` in the meta section
   and `path` / `mtime` / `btime` in records and Snapshots.
2. **Record birth time as an optional field.** Add `original_btime` to the
   meta entry map and `btime` to record / Snapshot entries: a signed count of
   whole seconds from the Unix epoch like `mtime`, captured from the local
   file at Container creation where the platform reports one
   (`std::fs::Metadata::created()` — an `Err` simply means the field is
   absent), omitted otherwise. Birth time is a capture-only fact: unlike a
   name, it cannot be recovered later once the original file is gone.
   Restoring it onto fetched files is deliberately out of scope — on Linux
   there is no API to set it, and on APFS setting an older mtime already
   drags the visible creation time down to it.
3. **Pin `mime` as a hint.** FM-9's optional `mime` is a creation-time guess;
   state in the rule that no reader treats it as authoritative (the server's
   extension table is the single place that decides openability).

There is no backward compatibility to preserve: nothing reads the old keys
after this change, `schema` stays 1 with the rewritten rule text, and every
fixture is regenerated. Existing verification Libraries are throwaway data
and get recreated, the same treatment the Entry Path NFC change gave them.

Concretely:

1. **Spec, `docs/spec/format/README.md`.** Rewrite FM-9's entry-map wording:
   the renamed keys with their provenance meaning (one sentence on "as of
   this Container's creation, not the Entry's first-ever name"), the new
   optional `original_btime` (whole seconds, signed, absent when the
   filesystem reports none), and the `mime` hint sentence. Rewrite FM-15's
   `entries` clause (and FM-16's reference to it) to spell the catalog
   vocabulary — `path`, `mtime`, optional `btime`, with `offset` / `size` /
   `hash` / `mime` / `derived_from` shared with FM-9 — and why the two
   spellings differ. Check `docs/spec/README.md`'s Mechanisms table for
   wording that names the old keys.
2. **Rust, `backend/crates/domain/coffret-format`.** Split the shared wire
   entry (`src/meta/wire_entry.rs`, currently reused by
   `src/control/journal_record/` and `src/control/index_snapshot/`) into the
   meta spelling and the catalog spelling; the domain `Entry` model gains an
   optional birth time next to `mtime` (extend `Mtime`'s home module or add a
   sibling — follow the one-type-per-module layout). Decode rejects a meta
   entry missing `original_path` exactly as it rejects a missing `path`
   today.
3. **Capture, `backend/crates/domain/coffret-usecase`.** Where the walk reads
   the local modification time (`src/local_mtime.rs`), also read the birth
   time when the platform provides it, and carry it through spool / freeze /
   sync so committed record entries hold it. The conformance suites' injected
   filesystems gain a case with a birth time and a case without.
4. **Index, `backend/crates/gateway/coffret-sqlite-index`.** Add a nullable
   `btime` column to the entries schema (`src/library_state.rs`), populate it
   on replay, return it on queries, and cover it in the index conformance
   suite. The Index is a device-local cache, so this is a schema change with
   no migration — a rebuilt Index replays it from records.
5. **TypeScript, `frontend/packages/domain/format/src`.** Mirror the split in
   `meta.ts` and the control codecs, the model types, and the unit tests.
6. **Interop, `backend/crates/apps/coffret-interop`.** Regenerate the fixture
   exchange under the new keys in both directions, including at least one
   Container and one Journal record fixture with a birth time present and one
   with it absent.
7. **Concepts.** Update `docs/concepts/container/README.md` (and the Entry
   concept beneath it) where they list the entry table's fields or lean on
   "no external catalog is needed": self-description now names content as of
   the Container's creation, and the Journal remains the authority for the
   current state — keep meaning and guarantees there, byte forms in the spec.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] FM-9 in `docs/spec/format/README.md` defines `original_path`,
      `original_mtime`, and optional `original_btime` with their
      as-of-creation meaning and pins `mime` as a non-authoritative hint
      (the `grep -q 'original_path'` / `grep -q 'original_btime'` gates in
      `check_command` match nothing today).
- [x] `coffret-format` unit tests cover: the meta section round-trips the
      `original_*` keys and rejects an entry map missing `original_path`;
      Journal record and Index Snapshot payloads round-trip `path` / `mtime`
      and an optional `btime`, both present and absent; encodings stay
      deterministic.
- [x] `@coffret/format` unit tests mirror that matrix, and the interop
      exchange carries regenerated fixtures under the new keys in both
      directions, with birth time present in at least one fixture and absent
      in at least one (the `grep -rq 'original_btime'` gate on the
      TypeScript package matches nothing today).
- [x] The freeze / sync conformance suites show a walked file with a birth
      time ending up as a committed record entry carrying `btime`, and one
      without staying absent; the index conformance suite shows `btime`
      surviving replay and queries.

## Out of scope

- Catalog-level renames (the `renames` record vocabulary) and any rewording
  of EP rules — this task only renames the meta keys and adds capture.
- Restoring birth time onto fetched files, and any timestamp-correction
  vocabulary in the catalog.
- Reading the old key names anywhere, or migrating existing stored data —
  existing verification Libraries are recreated instead.
- Explorer UI and server API changes.
