---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && awk '/\\*\\*FM-11\\./,/\\*\\*FM-12\\./' docs/spec/format/README.md | grep -q 'Padmé'"
assignee: null
branch: task/0823-1612-pad-control-object-payloads
created_at: 2026-08-23T07:12:00Z
updated_at: 2026-08-23T08:24:54Z
---

# feat(backend): pad control-object payloads to a Padmé bucket, like the meta section

## Overview

A Container's plaintext stream and its meta section are both padded to a
Padmé bucket before encryption (FM-4, FM-9), so the provider sees only a
bucketed approximation of how many Entries a Container holds. Control objects
are not: FM-11 encrypts the CBOR payload as it is
(`backend/crates/domain/coffret-format/src/control/payload/`,
`control/encode.rs`, `control/decode.rs`, and their TypeScript twins under
`frontend/packages/domain/format/src/control/`). An Index Snapshot's payload
lists every current Entry at roughly 100 bytes each, a Journal record's
additions list every Entry a batch added, and a Keyring's mapping has one
element per Container — so each of their ciphertext sizes hands the provider
a precise count that the meta-section padding was introduced to blur.

Apply the meta section's rule to control-object payloads: the plaintext that
is encrypted is the CBOR map followed by zero bytes up to the next Padmé
bucket boundary, for every kind — Journal, Keyring, Index Snapshot, and
activation Index Snapshot alike. Padmé keeps the overhead under about 12% and
small in the typical case, and the repository already has the function
(`padme::padded_len` in `coffret-format`, `paddedLength` in the TS package).

### What changes

1. **Spec — FM-11.** State that the payload plaintext is the CBOR map followed
   by zero padding up to the next Padmé bucket boundary (FM-4), that a
   decoder reads one CBOR item and rejects any non-zero byte after it, and
   why: so the ciphertext length of a Journal record, Keyring, or Index
   Snapshot is not a proxy for the number of Entries or Containers it lists,
   matching what FM-9 already does for the meta section. Keep FM-11's header
   layout and everything else unchanged. The `check_command` greps the FM-11
   rule for `Padmé` — make sure the word lands inside the FM-11 entry.
2. **Rust.** `control/payload.rs` (or wherever the payload is serialized and
   parsed — follow the module's existing split) pads on encode and, on decode,
   parses one CBOR item and rejects trailing non-zero bytes with a dedicated
   error in `coffret-format`'s `Error`, named like the meta section's
   `NonZeroMetaPadding` (e.g. `NonZeroControlPadding`) — no equality on error
   types, tests match the variant. Round-trip tests assert the encoded
   plaintext length is a Padmé bucket for a few payload sizes (including one
   below the bucket threshold where Padmé leaves it unpadded, and one that
   grows across a bucket boundary), and the rejection tests flip one padding
   byte at every position and expect the new error.
3. **TypeScript.** Mirror the same in `control/encode.ts` / `decode.ts` /
   `payload.ts` (and `errors.ts` for the new code), with the same tests.
4. **Interop.** Regenerate the interop fixtures and manifests with the repo's
   tooling so both implementations exchange padded control objects. The
   manifest carries only values the format does not derive, so do not add a
   length to it; have both verifiers enforce the bucket on decode instead,
   which makes an unpadded object fail the exchange.
5. **Concept document.** `docs/concepts/storage-object/README.md` says control
   objects' type and update frequency are accepted leakage; add, in the same
   bullet family and in the concept-doc house style (`docs/concepts/README.md`
   conventions), that their size is padded like a Container's meta section
   so it gives only a bucketed approximation of what they list (spec: FM-11).
   Nothing else in the concept documents changes.

Documentation, comments, commit message, and PR description are in English.
Keep spec rule IDs stable (FM-11 keeps its ID); `docs/spec/README.md` is the
register's conventions.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Encoding a control payload of any kind yields a plaintext whose length is
      the Padmé bucket of its CBOR length, and a payload whose CBOR length is
      below the unpadded threshold is stored unpadded — both asserted in Rust
      and TypeScript round-trip tests.
- [x] Decoding reads one CBOR item and rejects a payload with any non-zero
      byte after it, with a dedicated error variant / error code; the
      rejection test flips each padding byte in turn. Error types gain no
      `PartialEq`; tests match the variant.
- [x] The rule holds for all four kinds (Journal, Keyring, Index Snapshot,
      activation Index Snapshot): a round-trip test covers each kind.
- [x] The interop fixture set exchanges padded control objects in both
      directions (`make interop`); both verifiers enforce the bucket length on
      decode, so an unpadded control object fails the exchange. (The manifest
      deliberately states no derived lengths — its convention is to carry only
      values the format does not compute — which is why the enforcement lives
      in the decoders rather than in a recorded number.)
- [x] FM-11 in `docs/spec/format/README.md` states the Padmé padding and the
      non-zero-trailing-byte rejection (grep-gated in `check_command`); the
      Storage Object concept document states the bucketed-size consequence.
- [x] `make check` passes: fmt, build, tests, clippy, TS typecheck/test/lint,
      deps, interop.

## Out of scope

- Using the header's reserved byte (offset 7) as a payload-encoding marker for
  future compression — a separate decision; this task keeps it zero.
- The Index Snapshot and Journal record payload *contents* (entry tables,
  checkpoint fields) — their own task; this task pads whatever CBOR the
  payload holds.
- Container padding (FM-4 / FM-9) — unchanged.
