---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings && test \"$(cargo tree -p coffret-model -e normal --prefix none | wc -l)\" -eq 1"
assignee: null
branch: task/0818-1554-key-derivation-and-control-framing
created_at: 2026-08-18T15:54:40Z
updated_at: 2026-08-18T19:55:00Z
---

# feat(backend): implement key derivation, Key Envelopes, and control-object framing

## Overview

Complete the encryption layer's key side: the `KD` rules
(`docs/spec/key-derivation/README.md`) and the remaining `FM` rules —
Key Envelopes (FM-14) and control-object framing and naming (FM-11 to
FM-13) in `docs/spec/format/README.md`. Read both spec files first; every
rule named below is normative there. Container v1 encode/decode (FM-1 to
FM-10) is already implemented in `backend/crates/domain/coffret-format`
and `coffret-model` — build on it, do not restructure it.

This task also makes one deliberate format change: **pad the meta
section**. Today the plaintext Container header records the meta section's
exact ciphertext length, which hands the storage provider a precise proxy
for the Entry count and total Entry Path length while the content stream
is size-blurred by Padmé (FM-4). Close the asymmetry:

- The meta section plaintext becomes the CBOR map followed by zero padding
  up to the next Padmé bucket boundary (reusing the existing `padded_len`
  function). CBOR is self-delimiting, so no extra length field is needed:
  the decoder reads one CBOR item and then verifies that every remaining
  plaintext byte is zero, rejecting the object otherwise.
- Amend `docs/spec/format/README.md` in the same commit as the code so
  spec and implementation never diverge silently: extend FM-9 with the
  padding-and-verification clause above, and adjust FM-2's description of
  the meta section length field to say it records the padded ciphertext
  length. Update any existing test whose citation or assertions this
  changes.

Implementation shape (follow the existing conventions: workspace deps in
`backend/Cargo.toml` referenced with `workspace = true`; `pub enum Error`
+ `Result` alias per crate; one public type per module; directory modules
for anything that will grow):

- **`coffret-model`** (must stay free of third-party dependencies — the
  check command enforces it): add the key-material and control vocabulary
  as plain newtypes. Expected: `MasterKey` (256-bit, redacted `Debug`, no
  `PartialEq`/`Display`, mirroring `ContainerKey`), `MasterKeyEpoch`
  (u64, first epoch 1), `KeyEnvelope` (the 72-byte form of FM-14),
  `ControlObjectKind` (Journal / Keyring / IndexSnapshot), and newtypes
  for generation and replica index/count as FM-11 needs them.
- **`coffret-format`**: new modules for key derivation and control
  framing; suggested new workspace deps: `hkdf`, `sha2`, `argon2`.
  - Purpose keys per KD-3/KD-4: HKDF-SHA-256, IKM = the Master Key,
    zero-length salt, 32-byte output, the four v1 info strings exactly as
    KD-4 spells them. Generalize the internal `aead::Cipher` constructor
    to accept any 32-byte key (it is `pub(crate)`; currently it takes
    `&ContainerKey`).
  - Container Key generation per KD-2 (CSPRNG, alongside the existing
    `generate_container_id`).
  - Key Envelope wrap/unwrap per FM-14: XChaCha20-Poly1305 under the
    container-wrap purpose key, fresh random 24-byte nonce, AD = the
    16-byte Container ID.
  - Control-object framing per FM-11 to FM-13: encode/decode of the
    44-byte `CFCTL` header with AD = full header and a random nonce; the
    payload is caller-supplied CBOR bytes plus the mandatory
    `master_key_epoch` (FM-13) — the per-kind payload schemas (Journal,
    Keyring, Index Snapshot fields) are NOT this task's scope. Object
    names and the name/header cross-check per FM-12 (`jrn-`, `idx-`,
    `key-` forms; mismatch is rejected).
  - The stored Master Key form per KD-5 to KD-7: Argon2id over the
    Passphrase with a per-device random salt, parameters recorded in the
    stored form and bound as AEAD associated data (downgrade detection),
    wrapping the Master Key and its epoch; self-contained bytes in/bytes
    out — no file I/O in this crate. Initial Argon2id parameters are
    chosen from the current OWASP-recommended band and pinned as named
    constants with a comment stating the source band.
- Every test verifying a specific FM/KD rule cites the rule ID in a
  comment above the test, as the existing tests do.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Purpose-key derivation pins golden vectors: for a fixed Master Key,
      a unit test asserts the exact derived key bytes for each of the four
      v1 info strings, so any drift in HKDF parameters (salt, info,
      length) breaks the test (KD-3, KD-4).
- [x] A payload sealed under one purpose key fails to open under any
      other purpose key or under a Container Key — domain separation is
      observable (KD-4).
- [x] A Key Envelope is exactly 72 bytes; wrap→unwrap round-trips the
      Container Key; unwrapping with a different Container ID as AD fails
      (FM-14).
- [x] Control objects of all three kinds round-trip through the framing;
      tampering with each header field fails decryption; an unknown magic
      or version is rejected before decryption (FM-11).
- [x] Object-name generation and parsing round-trip for `jrn-`, `idx-`,
      and `key-` forms, and an object whose name-encoded kind, generation,
      or replica position disagrees with its header is rejected; Journal
      and Index Snapshot names carry replica index 0, count 1 (FM-12).
- [x] A control-object payload missing `master_key_epoch` is rejected;
      the epoch round-trips (FM-13).
- [x] The stored Master Key form round-trips under the correct
      Passphrase, fails under a wrong one, and fails when its recorded
      Argon2id parameters are tampered with (KD-5, KD-7); the parameters
      recorded in the stored form drive the derivation, so a stored form
      written with non-default parameters still unlocks (KD-6).
- [x] The meta section's ciphertext length is `padded_len(cbor_len) +
      TAG_LEN` for containers of varying entry counts, a decoder rejects
      non-zero bytes after the CBOR item, and existing Container
      round-trips still pass with padded meta (amended FM-9).
- [x] `docs/spec/format/README.md` FM-9 states the meta-padding rule in
      the same PR (grep-verifiable: the FM-9 entry mentions padding).
- [x] `coffret-model` still has zero third-party dependencies (the
      `cargo tree` gate in the check command).

### Manual / on-hardware (verified by a human before merge)

- [x] The Argon2id initial parameters land in the OWASP-recommended band
      current at review time.
- [x] The public API shape supports the storage layer that follows
      (upload/fetch of Containers and control objects, Keyring assembly)
      without reshaping crate boundaries.

## Out of scope

- Per-kind control-object payload schemas (Journal record fields, Keyring
  mapping and `set_digest`, Index Snapshot contents) and their rules.
- Any file or network I/O: key-file persistence and Storage adapters are
  gateway work.
- Recovery Code encoding (Bech32 or word list), device custody states
  (`DK` rules), and Master Key rotation flows (`MR` rules).
- Key zeroization on drop: `coffret-model` stays dependency-free in this
  task; whether to grant a `zeroize` exception or move key types to a
  crate that may take dependencies is a separate decision to record
  before the device-custody work.
- Migrating `Form: test` register entries into test comments — still
  deferred; do not delete register entries. Amending FM-2/FM-9 for meta
  padding is required by this task and is the only spec edit in scope.
