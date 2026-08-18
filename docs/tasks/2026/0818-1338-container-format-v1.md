---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: "cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings && test \"$(cargo tree -p coffret-model -e normal --prefix none | wc -l)\" -eq 1"
assignee: null
branch: task/0818-1338-container-format-v1
created_at: 2026-08-18T13:38:33Z
updated_at: 2026-08-18T14:55:00Z
---

# feat(backend): add coffret-model and coffret-format crates implementing Container v1

## Overview

Implement the Container v1 byte format — the first piece of the encryption
layer. The normative rules are `FM-1` through `FM-10` in
`docs/spec/format/README.md` (chunked XChaCha20-Poly1305 framing,
deterministic nonces, header AD binding, encrypted CBOR meta section, Padmé
padding, at-least-one-entry). Read that file first; `docs/spec/key-derivation/README.md`
gives surrounding context but is NOT implemented by this task.

Create two library crates under new workspace categories
(`backend/Cargo.toml` already declares `members = ["crates/*/*"]`, so new
category directories are picked up automatically):

- `backend/crates/domain/coffret-model` — the core domain types:
  `ContainerId`, container kind (one-file / pack), entry metadata (entry
  path as an opaque string for now, offset/size, mtime, BLAKE3-256 content
  hash, optional derived-from and mime), plus a `ContainerKey` newtype for
  the 256-bit key material. **This crate must have zero third-party
  dependencies** (the check command enforces this via `cargo tree`). Do not
  derive serde traits on these types; byte-format concerns stay in
  coffret-format.
- `backend/crates/domain/coffret-format` — pure encode/decode for Container
  v1, depending on `coffret-model` and the crypto/serialization crates it
  needs (suggested: RustCrypto `chacha20poly1305` for the AEAD, `blake3`
  for hashes, `ciborium` or `minicbor` for CBOR, `getrandom`/`rand` for
  `ContainerId` generation). No file or network I/O anywhere in this crate:
  the API takes and returns bytes/readers over in-memory data. Streaming
  (chunk-at-a-time) processing is the intended shape — decoding
  authenticates each chunk before releasing its plaintext (FM-1, FM-5) —
  but the public API may be buffer-based in this task as long as chunk
  processing is incremental internally.

Both crates follow the existing workspace conventions: dependencies
declared in `[workspace.dependencies]` in `backend/Cargo.toml` and
referenced with `workspace = true`; each crate exposes `pub enum Error` and
`pub type Result<T>` at its root; keep one public type per module.

Implementation notes:

- Encoding input: an ordered list of entries (metadata + plaintext bytes),
  the container kind, a `ContainerKey`, a `ContainerId`, and a chunk size
  (default 1 MiB, caller-overridable per FM-6). Output: the full container
  byte stream per FM-2 plus the Storage object name per FM-3
  (`<32 lowercase hex>.cfrt`).
- Decoding input: container bytes plus the `ContainerKey`. Header
  validation (magic, version, reserved bytes) must reject before any
  decryption is attempted (FM-2). Decode returns the entry metadata and
  plaintext contents with padding stripped via the meta section's
  `pad_len` (FM-4, FM-9).
- The Padmé bucket function is specified in FM-4 (round up to a multiple
  of 2^(E−S), E = ⌊log₂ L⌋, S = ⌊log₂ E⌋ + 1; streams with E − S ≤ 0 are
  unpadded). Implement it as a pure function with its own unit tests.
- Nonce construction and the AD are FM-7 and FM-8 exactly; the meta
  section's CBOR map and field names are FM-9 (`schema`, `kind`,
  `pad_len`, `entries`; per entry `path`, `offset`, `size`, `mtime`,
  `hash`, optional `derived_from`, `mime`). Unknown map fields must be
  ignored on decode (forward-open maps).
- Every test that verifies a specific FM rule cites the rule ID as plain
  text in a comment above the test, e.g. `// FM-7: chunk reordering fails
  authentication.`

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A multi-entry container round-trips: encode then decode returns
      byte-identical entry contents and identical metadata (path, mtime,
      hash, kind), verified by a unit test in coffret-format (FM-2, FM-5,
      FM-9).
- [x] Tampering with each header field (format version, Container ID,
      chunk size, meta section length) makes decode fail, and chunk
      reordering, truncation, extension, and dropping the final chunk each
      fail authentication — one test per case (FM-7, FM-8).
- [x] Decoding bytes with an unknown magic or format version is rejected
      with a distinct error before any decryption is attempted (FM-2).
- [x] The Padmé bucket function has unit tests pinning padded sizes for
      known input lengths, including the E − S ≤ 0 unpadded regime, and a
      round-trip test confirms `pad_len` padding is stripped on decode
      (FM-4).
- [x] A container encoded with a non-default chunk size round-trips, with
      the decoder honoring the header's recorded value (FM-6).
- [x] Encoding with an empty entry list is rejected, and decoding a meta
      section whose entry table is empty is rejected (FM-10).
- [x] The Storage object name for a container is the Container ID as 32
      lowercase hex characters followed by `.cfrt`, verified by a unit
      test (FM-3).
- [x] `coffret-model` has zero third-party dependencies — enforced by the
      `cargo tree -p coffret-model` gate appended to the check command.

### Manual / on-hardware (verified by a human before merge)

- [ ] The public API of coffret-format is a workable foundation for the
      follow-up work (key derivation, Key Envelopes, control-object
      framing) without reshaping the crate boundaries.

## Out of scope

- Key derivation (`KD` rules): HKDF purpose keys, Argon2id, the stored
  Master Key form. A follow-up task.
- Key Envelope wrap/unwrap (FM-14) and control-object framing and naming
  (FM-11 to FM-13). A follow-up task.
- Migrating `Form: test` entries out of `docs/spec/format/README.md` into
  test comments — deliberately deferred until the format implementation is
  complete across both tasks; do not edit the spec register in this task.
- Any usecase/gateway/wire crates, and any wiring into `coffret-server`.
- Entry Path canonicalization (`EP` rules) — `path` is carried as an
  opaque string here.
