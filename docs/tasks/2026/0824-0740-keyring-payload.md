---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q '\\*\\*FM-17\\.\\*\\*' docs/spec/format/README.md"
assignee: null
branch: task/0824-0740-keyring-payload
created_at: 2026-08-24T07:40:15Z
updated_at: 2026-08-24T09:17:38Z
---

# feat(backend): encode the Keyring payload and its set_digest

## Overview

The Keyring is the last control object whose payload has no wire form: the
framing (FM-11) and the replica names (FM-12) exist, `coffret-format`
encodes and decodes Journal records (FM-15) and Index Snapshots (FM-16),
and `control/mod.rs` itself notes "The Keyring's is not written yet." The
upload pipeline cannot pre-replicate Key Envelopes (the step before every
commit) until a Keyring replica can be turned into bytes and back, and
until `set_digest` — the value KL-1 validates, FM-12 puts in the replica
name, and FM-15 commits — is computed by one shared definition. This task
defines the payload in the spec register and implements it in both format
implementations plus the interop harness, exactly as FM-15/FM-16 were done.
The write path (uploading R replicas, read-back verification, KL states)
is the pipeline's and stays out.

### Spec — one new format rule

Add to `docs/spec/format/README.md` as **FM-17** (the next free ID), with
the register's conventions (`docs/spec/README.md`; `*(Form: test)*`; field
names as spelled here; unknown fields ignored on read; every array in a
stated canonical order so one state has one encoding):

- **FM-17 — Keyring payload** (kind `0x02`). A CBOR map with: `schema`
  (= 1) and `mapping` — an array in Container ID order, each element a map
  of `id` (the 16-byte Container ID) and exactly one of `envelope` (the
  72-byte Key Envelope, FM-14) or `key_lost` (= true, the explicit
  key-lost marker of KL-7). An element carrying both or neither is
  rejected, as is a `mapping` out of ID order or with a duplicate ID.
  `master_key_epoch` is the framing's (FM-13) and is not repeated inside;
  the replica index and count ride in the FM-11 header, so every replica
  of one generation carries an identical payload (KL-6) and differs only
  in header and nonce.
  - `set_digest` is **not a payload field**: it is the BLAKE3-256 of the
    canonical CBOR encoding of `mapping` alone (this rule fixes that
    encoding — the array as it appears in the payload, in ID order),
    carried in the replica's name (FM-12) and in the commitment tuple
    (CP-10, KL-3). Putting it inside the payload would make the digest
    cover itself; leaving it out keeps one definition shared by the name,
    the commitment, and KL-1's validity check. A reader computes it from
    the decoded `mapping` and compares it against the name it fetched
    under.

State why the order is canonical (same mapping, same bytes, same
`set_digest` whichever device writes it — KL-1 depends on that).

### Implementation

- **Rust.** Under `coffret-format/src/control/`, a `keyring/` directory
  module split like `journal_record/` (encode, decode, `set_digest`,
  round-trip tests, rejection tests), producing / consuming the CBOR map
  body that `ControlPayload` frames. The in-memory value is a new
  `KeyringMapping` (or equivalently named) type in `coffret-model`:
  an ordered list of (Container ID, `KeyEnvelope` | key-lost), reusing
  `coffret_model::{ContainerId, KeyEnvelope}`; follow the workspace's
  one-type-per-module convention. The encoder owns canonical ordering;
  the decoder verifies it and rejects out-of-order, duplicate, or
  both/neither elements with dedicated cause-named variants (no
  `PartialEq`; tests match variants). `set_digest` is a function of the
  mapping exposed from the same module, so the name builder
  (`ControlObjectName::keyring_replica`) and future KL-1 validation share
  it; return it as the lowercase-hex string the name grammar expects.
- **TypeScript.** Mirror in `frontend/packages/domain/format/src/control/`
  (`keyring.ts` + tests) with the same error codes in `errors.ts`.
- **Interop.** Extend the fixture set and manifest so a Keyring replica
  whose mapping holds several envelopes and one key-lost marker is
  exchanged in both directions; each verifier decodes the payload,
  re-computes `set_digest`, and compares both the fields and the digest
  the manifest states.
- **Size.** A test encodes a synthetic mapping of 10,000 Containers and
  pins the payload cost per Container (the design figure is ≈100 bytes —
  16-byte ID + 72-byte envelope + CBOR framing); pin the measured value
  the way the Index Snapshot size test pins its per-Entry cost.
- **Docs.** `docs/concepts/keyring/README.md` gets at most a one-sentence
  pointer to FM-17 if the concept-doc litmus test says so; otherwise leave
  it.

Documentation, comments, commit message, and PR description are in
English. Keep existing rule IDs stable; `FM-17` must be the ID used (the
`check_command` greps for it).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] FM-17 exists in `docs/spec/format/README.md` with the field list,
      the canonical order, the reader's rejections, and the `set_digest`
      definition (grep-gated on the rule ID in `check_command`).
- [x] Rust and TypeScript each round-trip a Keyring payload whose mapping
      holds envelopes and a key-lost marker; an element with both
      `envelope` and `key_lost`, with neither, a mapping out of ID order,
      and a duplicate ID are each rejected with dedicated variants /
      codes.
- [x] Encoding is canonical: two equal mappings built in different
      in-memory orders produce identical bytes and identical
      `set_digest`; the Rust and TypeScript `set_digest` of the interop
      fixture agree with the manifest.
- [x] The interop fixture set includes the Keyring replica and
      `make interop` exchanges it in both directions with field-level and
      digest comparison.
- [x] The synthetic 10,000-Container mapping encodes at or under the
      pinned bytes-per-Container bound.
- [x] `coffret-model` gains no third-party dependency (`make deps`);
      error types derive no `PartialEq`; tests match variants;
      `make check` passes.

## Out of scope

- The Keyring write path: uploading replicas, read-back verification,
  KL-1/KL-2 state evaluation, repair — these belong to the upload
  pipeline.
- Padding-parameter changes: the payload is padded by the existing FM-11
  rule like every control payload.
- The Journal commit and catch-up flows.
