---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "cd frontend && pnpm install && pnpm -r build && pnpm -r typecheck && pnpm -r test && pnpm -r lint"
assignee: null
branch: task/0819-0319-typescript-format-implementation
created_at: 2026-08-19T03:19:25Z
updated_at: 2026-08-19T05:02:00Z
---

# feat(frontend): implement the coffret format in TypeScript

## Overview

Build a second, independent implementation of the coffret storage format
in TypeScript, as a domain package the browser viewer can later reuse for
in-browser decryption. The immediate goal is interoperability
verification: proving that the public specification — not the Rust code —
is the format's source of truth. A follow-up task adds the Rust ⇄ TS
fixture-exchange harness and CI job; this task delivers the TS
implementation itself with unit-level compatibility pinned by golden
vectors.

Read `docs/spec/format/README.md` (`FM` rules) and
`docs/spec/key-derivation/README.md` (`KD` rules) first; every rule named
below is normative there. The Rust reference implementation lives in
`backend/crates/domain/coffret-format` and
`backend/crates/domain/coffret-model` — consult it to resolve
ambiguity, but implement from the spec. If the two disagree, or the spec
turns out to underspecify something a second implementation needs, follow
the spec where it speaks and record every such discrepancy or gap in the
PR description as a candidate for a deliberate spec or implementation
fix — do not silently pick a side.

Implementation shape:

- **New package `frontend/packages/domain/format/`, named
  `@coffret/format`** — the first package in the `domain` layer
  (`pnpm-workspace.yaml` already globs `packages/*/*`). Mirror
  `frontend/packages/apps/web`'s conventions: `package.json` with
  `build` (`tsc -b`), `typecheck`, `test` (`vitest run`), `lint`
  scripts, flat eslint config, strict TypeScript. Tests run under
  vitest.
- **Bytes in, bytes out.** The package performs no file, network, or DOM
  I/O — every public function takes and returns `Uint8Array` (plus plain
  data objects), mirroring the domain-crate principle on the Rust side,
  so the code runs unchanged in Node (vitest, the coming interop
  harness) and in the browser.
- **Dependencies**: `@noble/ciphers` (XChaCha20-Poly1305),
  `@noble/hashes` (HKDF-SHA-256), `hash-wasm` (Argon2id, WASM), and a
  CBOR codec (`cbor-x` suggested). The codec must decode any valid CBOR
  encoding of the schemas (the Rust side writes via `ciborium`); where
  the spec leaves encoding freedom, the TS encoder may make its own
  valid choices — byte-identical output with Rust is not a goal,
  mutual decodability is.

Functional scope, mapped to the spec:

- **Model vocabulary**: TS counterparts of `MasterKey`, `ContainerKey`,
  `ContainerId`, `MasterKeyEpoch` (u64 via `bigint`, first epoch 1),
  `KeyEnvelope`, `ControlObjectKind` (Journal / Keyring / IndexSnapshot),
  and generation / replica index / replica count values. Keep key
  material out of accidental logging (no `toString`/JSON leakage of raw
  bytes).
- **Purpose keys** per KD-3/KD-4: HKDF-SHA-256, IKM = Master Key,
  zero-length salt, 32-byte output, the four v1 info strings exactly as
  KD-4 spells them. Port the golden-vector test from the Rust side
  (`backend/crates/domain/coffret-format/src/purpose_key.rs`, test
  `derivation_matches_the_pinned_vectors`): same fixed Master Key (bytes
  `0x00..0x1f`), same four expected 32-byte outputs.
- **Container v1 encode/decode** per FM-1 to FM-10: header, chunked
  AEAD content stream with nonce/AD binding, meta section as CBOR
  padded with zeros to the next Padmé bucket (padded meta ciphertext
  length recorded in the header; decoder reads one CBOR item and rejects
  any non-zero padding byte), Padmé bucket function per the FM-4
  formula, opaque object naming per FM-3.
- **Control-object framing** per FM-11 to FM-13: the 44-byte `CFCTL`
  header with AD = full header, per-kind purpose keys, mandatory
  `master_key_epoch` in the payload, and object names per FM-12
  (`jrn-`, `idx-`, `key-` forms; name/header disagreement rejected;
  Journal and Index Snapshot use replica index 0, count 1). Note FM-13:
  generation is a Library-wide counter that never restarts at a
  rotation. Per-kind payload schemas are NOT in scope — payloads are
  caller-supplied CBOR bytes plus the epoch, as on the Rust side.
- **Key Envelope** per FM-14: 72 bytes, XChaCha20-Poly1305 under the
  container-wrap purpose key, AD = the 16-byte Container ID.
- **Stored Master Key** per KD-5 to KD-7 and the KD-9 byte layout:
  magic `CFMK1`, version 0x01, recorded Argon2id parameters bound as
  associated data (downgrade detection), recorded parameters drive the
  derivation on unlock, big-endian integers, reader rejections as KD-9
  lists them. Use cheap Argon2id parameters in tests (mirror the Rust
  tests' reasoning) while shipping OWASP-band defaults as named
  constants.

Every test that verifies a specific FM/KD rule cites the rule ID in a
comment above the test, as the Rust tests do.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Purpose-key derivation reproduces the Rust golden vectors: for the
      fixed Master Key `0x00..0x1f`, a vitest test asserts the exact
      32 bytes for each of the four v1 info strings (KD-3, KD-4).
- [x] Container v1 encode→decode round-trips containers of varying Entry
      counts; the meta section's ciphertext length equals the Padmé
      bucket of the CBOR length plus the AEAD tag length, and a decoder
      rejects a meta section with a non-zero byte after the CBOR item
      (FM-2, FM-9).
- [x] The Padmé bucket function agrees with the Rust implementation's
      pinned cases (port representative input/bucket pairs from
      `backend/crates/domain/coffret-format/src/padme.rs` tests) (FM-4).
- [x] A Key Envelope is exactly 72 bytes; wrap→unwrap round-trips the
      Container Key; unwrapping with a different Container ID as AD
      fails (FM-14).
- [x] Control objects of all three kinds round-trip through the framing;
      tampering with any header field fails decryption; an object whose
      name-encoded kind, generation, or replica position disagrees with
      its header is rejected; Journal and Index Snapshot names carry
      replica index 0, count 1; a payload missing `master_key_epoch` is
      rejected (FM-11, FM-12, FM-13).
- [x] The stored Master Key form round-trips under the correct
      Passphrase, fails under a wrong one, and fails when the recorded
      Argon2id parameters, salt, or nonce are edited; the recorded
      parameters drive the derivation, so a form written at another cost
      still unlocks; unknown magic or version and length mismatches are
      rejected (KD-5 to KD-7, KD-9).
- [x] `@coffret/format` builds, typechecks, tests, and lints as part of
      the workspace (`pnpm -r` from `frontend/`), with no Node-only or
      DOM APIs in `src/` (pure `Uint8Array` interfaces).

### Manual / on-hardware (verified by a human before merge)

- [x] The public API shape supports the follow-up interop harness
      without reshaping: encode functions accept caller-supplied inputs
      sufficient to reproduce a fixture (keys, IDs, nonces where the
      spec allows injection, payload bytes), and decode functions accept
      externally produced bytes.
- [x] The dependency choices (`@noble/*`, `hash-wasm`, CBOR codec) are
      acceptable for the future in-browser decryption path (bundle
      size, WASM loading story).

## Out of scope

- The Rust ⇄ TS fixture-exchange harness, the `coffret-interop` bin, and
  the CI job — the follow-up task, stacked on this branch.
- Per-kind control-object payload schemas (Journal record fields,
  Keyring mapping and `set_digest`, Index Snapshot contents).
- Any file, network, or DOM I/O; storage adapters; viewer integration.
- Recovery Code encoding, device custody (`DK`), and rotation (`MR`)
  flows.
- Changes to the Rust implementation or the spec (discrepancies are
  recorded in the PR description, not fixed here).
