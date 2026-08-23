---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q '\*\*FM-15\.\*\*' docs/spec/format/README.md && grep -q '\*\*FM-16\.\*\*' docs/spec/format/README.md"
assignee: null
branch: task/0823-1935-control-payload-schemas
created_at: 2026-08-23T10:30:14Z
updated_at: 2026-08-23T12:36:23Z
---

# feat(backend): encode Journal records and Index Snapshots as control-object payloads

## Overview

Control objects have a framing (FM-11: header, padded CBOR payload, AEAD) and
a single framing-owned payload field (`master_key_epoch`, FM-13); the
payloads' own fields have never been specified or implemented. The Index
port (`coffret_usecase::Index`) now speaks the domain values a Journal record
and an Index Snapshot carry — `JournalRecord`, `SnapshotContent`,
`ContainerAddition` in `coffret-usecase`, over `ContainerSummary`,
`EntryLocation`, `IndexCheckpoint`, `KeyringCommitment` in `coffret-model` —
but nothing turns them into bytes or back. This task defines the two payload
schemas in the spec register, implements them in both format implementations
(Rust `coffret-format`, TypeScript `@coffret/format`), and exchanges them
through the interop harness, so that the commit and catch-up flows can be
built on a fixed wire form. The Keyring payload is not part of this task.

### Spec — two new format rules

Add to `docs/spec/format/README.md`, as the next free IDs, with the
register's conventions (`docs/spec/README.md`; `*(Form: test)*`; CBOR map
field names as spelled here; integers as CBOR integers, hashes and IDs as byte
strings, names and paths as text strings; unknown fields ignored on read, as
FM-9's meta section does; every array in a stated canonical order so one
state has one encoding):

- **FM-15 — Journal record payload.** A CBOR map with: `schema` (= 1);
  `prev` (the previous control head's generation; omitted at generation 0);
  `next_commit_slot` (the adapter token only — CP-15: a minted identifier
  where the Storage mints them, absent where the name is the slot);
  `snapshot_slot` (same form, CK-10); `keyring_generation`,
  `keyring_replica_count`, `keyring_set_digest` (CP-10, KL-3);
  `additions` — an array in Container ID order, each element a map of `id`
  (16-byte Container ID), `kind` (`one-file` / `pack`, PK-15),
  `ciphertext_hash` (BLAKE3-256 of the stored object), `ciphertext_len`,
  optional `object_ref` (the provider's object identifier, a cache so a
  device catching up can fetch the Container without a listing), and
  `entries` — the Container's entry table, each element exactly FM-9's entry
  map (`path`, `offset`, `size`, `mtime`, `hash`, optional `mime`,
  optional `derived_from`) — CP-11; `removals` — an array of 16-byte
  Container IDs in ID order (CP-14). `master_key_epoch` is the framing's
  (FM-13) and is not repeated inside.
- **FM-16 — Index Snapshot payload** (ordinary, kind `0x03`, and activation,
  kind `0x04`, share it): a CBOR map with `schema` (= 1); the checkpoint —
  `head_generation`, `journal_generation`, `next_commit_slot` (adapter
  token only), `keyring_generation`, `keyring_replica_count`,
  `keyring_set_digest` (CK-1 to CK-3); `containers` — an array in Container
  ID order, each element `id`, `kind`, `ciphertext_hash`, `ciphertext_len`,
  optional `object_ref`; `entries` — an array in Entry Path byte order
  (EP-3), each element FM-9's entry map plus `container`, the index of the
  owning element in `containers` (so the 16-byte ID is not repeated per
  Entry). An activation Snapshot additionally carries `base_head_generation`
  and `activation_slot` (MR-2); an ordinary Snapshot carries neither and is
  rejected if it does, an activation Snapshot is rejected if it lacks either.
  A Snapshot carries no device state (CK-7) — the Index's record of which
  Snapshot it adopted (`SnapshotContent::adopted_from`) is never encoded.

State in both rules why the order is canonical (same state, same bytes;
prefix ranges over `entries` by binary search) and that a reader verifies the
array orders and the `container` indexes and rejects an out-of-order or
dangling payload rather than repairing it.

### Implementation

- **Where the wire types live.** `coffret-format` depends on `coffret-model`
  only and `coffret-usecase` on `coffret-model` only; `SnapshotContent`,
  `JournalRecord`, `ContainerAddition` currently live in `coffret-usecase`.
  Preferred resolution: move those three value types into `coffret-model`
  (they are plain data over model types — what a control object carries is
  format vocabulary), keep re-exports in `coffret-usecase` so the `Index`
  port and its suite are unchanged, and let `coffret-format` encode and
  decode them directly. `SnapshotContent::adopted_from` stays on the type
  (it is the Index's provenance) and the encoder ignores it; the decoder
  yields `None`. If moving proves wrong for a reason you can state, the
  alternative is wire structs in `coffret-format` with conversions in
  `coffret-usecase` (which would then depend on `coffret-format`) — say
  which you chose and why in the PR. `JournalRecord` gains the fields the
  wire needs that it lacks (`prev`, `snapshot_slot`); `ContainerAddition`
  gains optional `object_ref` if `ContainerSummary` does not already carry
  it.
- **Rust.** Under `coffret-format/src/control/`, a `journal_record/` and an
  `index_snapshot/` module (directory modules, split like `payload/`:
  encode, decode, round-trip tests, rejection tests, shared test helpers),
  producing / consuming the CBOR map body that `ControlPayload` frames
  (`ControlPayload::new(epoch, body)`; the framing owns `master_key_epoch`
  and rejects a body that repeats it). Canonical ordering is the encoder's
  job; the decoder verifies it. Errors: dedicated variants in
  `coffret-format`'s `Error` named for *why* (malformed field, order
  violation, dangling `container` index, activation fields on an ordinary
  Snapshot and vice versa), structured causes, no `PartialEq`; tests match
  variants. Reuse the meta section's entry-map codec for the `entries`
  elements rather than writing a second one.
- **TypeScript.** Mirror in `frontend/packages/domain/format/src/control/`
  with the same error codes in `errors.ts` and the same tests.
- **Interop.** Extend the fixture set and manifest so a Journal record with
  additions (including entries and a removal), an ordinary Index Snapshot
  with several Containers and path-sorted Entries, and an activation
  Snapshot are exchanged in both directions; the verifiers decode the
  payload fields and compare them to what the manifest states (CBOR map
  order is the writer's freedom, as `check_cbor_map` already handles).
- **Size.** A test encodes a synthetic Snapshot of 10,000 Entries with
  realistic paths (`books/<title>/page-NNN.png`, `albums/<year>/IMG_NNNN.jpg`)
  and asserts the payload is at most 120 bytes per Entry before padding — the
  per-Entry ceiling this schema targets; if the real number is lower, pin
  the lower bound instead and say so.
- **Docs.** `docs/concepts/journal/` and `docs/concepts/index-snapshot/` need
  at most a one-sentence pointer to the new rules if the concept-doc litmus
  test (`docs/concepts/README.md`) says so; otherwise leave them.

Documentation, comments, commit message, and PR description are in English.
Keep existing rule IDs stable; `FM-15` and `FM-16` must be the IDs used
(the `check_command` greps for them).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] FM-15 and FM-16 exist in `docs/spec/format/README.md` with the field
      lists above, the canonical orders, and the reader's rejections
      (grep-gated on the rule IDs in `check_command`).
- [x] Rust and TypeScript each encode a `JournalRecord` to a payload body and
      decode it back equal (round trip), for a record with additions carrying
      entries, a removal, both slots present, and both slots absent.
- [x] Rust and TypeScript each round-trip an ordinary Index Snapshot and an
      activation Snapshot; an ordinary Snapshot carrying activation fields and
      an activation Snapshot lacking them are rejected with dedicated
      variants / codes.
- [x] Encoding is canonical: two `SnapshotContent` values with the same
      content but different in-memory order produce identical bytes; a
      payload with `containers` out of ID order, `entries` out of path order,
      or a `container` index past the end is rejected on decode, not
      repaired.
- [x] `adopted_from` never appears in an encoded Snapshot (a grep of the CBOR
      keys in a test), and decoding yields `adopted_from: None`.
- [x] The interop fixture set includes the three new control objects and
      `make interop` exchanges them in both directions with field-level
      comparison against the manifest.
- [x] The synthetic 10,000-Entry Snapshot encodes at or under the pinned
      bytes-per-Entry bound.
- [x] `coffret-model` gains no third-party dependency (`make deps`); error
      types derive no `PartialEq`; tests match variants; `make check` passes.

## Out of scope

- The Keyring payload (`mapping`, `set_digest`) — with the Keyring write path.
- Compression of the Snapshot payload; the reserved header byte.
- The commit / catch-up flows that produce and consume these payloads.
