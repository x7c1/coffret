---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'RecoveryCodeEpochOutOfRange' backend/crates/domain/coffret-format/src/error.rs && grep -q 'StoredMasterKeyEpochOutOfRange' backend/crates/domain/coffret-format/src/error.rs && grep -rq 'a_recovery_code_epoch_past_the_formats_integer_range_is_refused' backend/crates/domain/coffret-format/src/recovery_code && grep -rq 'a_stored_master_key_epoch_past_the_formats_integer_range_is_refused' backend/crates/domain/coffret-format/src/stored_master_key && grep -q 'epoch past the integer range the format admits' frontend/packages/domain/format/src/recoveryCode/recoveryCode.test.ts && grep -q 'epoch past the integer range the format admits' frontend/packages/domain/format/src/storedMasterKey/storedMasterKey.test.ts && ! grep -q 'detail: error.to_string()' backend/crates/domain/coffret-format/src/meta/decode.rs && grep -rq 'a_meta_integer_past_the_formats_integer_range_names_its_field' backend/crates/domain/coffret-format/src/meta && grep -rq 'refuses to write an integer past the integer range the format admits' frontend/packages/domain/format/src"
assignee: null
branch: task/0905-1453-name-the-epoch-and-meta-refusals-the-format-states-and-guard-the-typescript-writers
created_at: 2026-09-05T15:28:47Z
updated_at: 2026-09-05T16:50:16Z
---

# fix(format): name the epoch and meta refusals the format states and guard the TypeScript writers

## Overview

FM-19 bounds every integer format v1 carries below 2^63, and the change
that introduced it left three seams where a reader's refusal is not stated
as well as its siblings', plus one asymmetry between the two
implementations. Close them:

- **The epoch's eight bytes.** FM-19 says the Master Key epoch is bounded
  wherever it is spelled — the eight bytes a Recovery Code carries (KD-11)
  and the eight a stored Master Key holds (KD-9), as well as the payload
  field. The control header's generation, the other eight-byte number the
  format reads, gets a format-level refusal
  (`Error::ControlHeaderGenerationOutOfRange { generation }` in
  `control/header.rs`, with the rationale "the format states this rule, so
  the format names the refusal"). The two epoch readers do not:
  `recovery_code/parse.rs` and `stored_master_key/unlock.rs` call
  `MasterKeyEpoch::new(...)?` and pass the model's
  `EpochOutOfRange { epoch }` through as `Error::Model`, so a caller matching
  on the format's vocabulary sees one refusal for a header and a different
  kind for a Recovery Code. Neither side has a test for an epoch at or past
  the bound in either carrier; the only epoch tests are "epoch 0 is refused"
  and "the bound round-trips".
- **The meta section's field name.** `wire_uint.rs`'s `WireUint` states the
  bound once for the serde-derived `WireMeta` / `WireMetaEntry` /
  `WireCatalogEntry`, but a refusal reaches `MalformedMeta { detail }` as
  ciborium's own rendering of a custom error —
  `Semantic(None, "expected an unsigned integer below 2^63, found 9223372036854775808")`
  — which names neither the field nor the entry, and wraps the message in a
  `Debug` spelling with escaped quotes. Every other malformed-meta detail
  goes through the same `detail: error.to_string()` in `meta/decode.rs`, so
  the wrapping is pre-existing; the missing field name is new with FM-19.
- **The TypeScript writers.** The Rust side checks the bound on the way out
  too (`WireUint`'s `Serialize`, and the model types that cannot hold a
  larger number), so no object it writes is one its own reader refuses. The
  TypeScript encoders write raw `bigint`s — `encodeExtent`'s `offset` /
  `size`, `meta.ts`'s `pad_len`, `wireContainer.ts`'s `ciphertext_len`, and
  `writeU64BE` for the header's generation — with no such guard.
  `Generation` and `MasterKeyEpoch` are bounded at construction, so the
  header is covered in practice; the CBOR fields are not.

### 1. Epoch refusals named by the format (`coffret-format`)

- Add `Error::RecoveryCodeEpochOutOfRange { epoch: u64 }` and
  `Error::StoredMasterKeyEpochOutOfRange { epoch: u64 }` beside the existing
  Recovery Code and stored Master Key variants in `error.rs`, each covering
  every number `MasterKeyEpoch::new` refuses — 0 and anything past the bound
  — since KD-11 already lists epoch 0 among a Recovery Code's rejections and
  the eight bytes spell no epoch in either case. Docs and `Display` in the
  family of `ControlHeaderGenerationOutOfRange` (the number travels; it is
  the format's arithmetic, not Library content). Map the model's refusal
  with an explicit `map_err` at the two read sites, dropping the model error
  the way `header.rs` does (the variant restates the whole of what the model
  refused for; a cause would be a second spelling).
- Update the existing epoch-0 tests (`recovery_code/tests.rs::epoch_zero_is_rejected`,
  and the stored Master Key equivalent if one exists) to the new variants,
  and add `a_recovery_code_epoch_past_the_formats_integer_range_is_refused`
  (payload bytes with epoch 2^63; `MAX_FORMAT_INTEGER` round-trips, as the
  existing round-trip test already shows) and
  `a_stored_master_key_epoch_past_the_formats_integer_range_is_refused`
  (seal a plaintext whose epoch bytes spell 2^63 the way the existing
  epoch-0 case does, or craft it the same way the TypeScript test
  `rejects a stored epoch below one` does).
- TypeScript keeps `epoch_out_of_range` for both carriers: the TypeScript
  side already reports the header's generation with the model-level
  `generation_out_of_range`, so a per-carrier code would be a new
  asymmetry, not parity. Add two tests titled with the phrase "epoch past
  the integer range the format admits" (`recoveryCode.test.ts`,
  `storedMasterKey.test.ts`) exercising 2^63 through the real decoders.
- `control/payload/decode.rs` keeps its `MasterKeyEpoch::new(epoch)?` for
  the payload field: there the bound is already stated by the format's own
  `as_bounded_uint`, and the model's epoch-0 refusal is the one rule the
  format's reader does not state, so the passthrough is a mechanically
  identical failure. Say so in a comment only if the asymmetry with the two
  carriers would otherwise puzzle a reader.

### 2. `MalformedMeta` names the field (`coffret-format`)

- A `WireUint` refusal must reach the caller as
  `MalformedMeta { detail }` whose detail names the field (`pad_len`,
  `offset`, `size`, `schema`) and the value, in the shape the payload
  readers already use ("`<key>` is an unsigned integer below 2^63, found
  …"). Two routes; pick the one that keeps the bound stated once:
  - keep `WireUint` and have the serde field carry its name into the error
    (a `deserialize_with` per field, or a small const-generic-free wrapper
    per field name), or
  - deserialize the wire structs with plain `u64` fields and apply one
    bounded, field-naming conversion as each wire struct becomes a domain
    value (`WireMeta` → `Meta`, `WireMetaEntry::to_metadata`,
    `WireCatalogEntry`'s conversion), mirroring the serialize-side guard in
    the encoders. If this route wins, `wire_uint.rs` goes away and its doc's
    reasoning moves to the helper.
  The control payload's `WireCatalogEntry` reports through the payload's own
  malformed variant (`MalformedJournalRecord` / `MalformedIndexSnapshot`),
  naming the field the same way.
- Unwrap ciborium's error in `meta/decode.rs` so `MalformedMeta { detail }`
  carries the message, not `Semantic(None, "…")`: match
  `ciborium::de::Error::Semantic(_, message)` and take the message; keep
  the other variants' `to_string()` (they are ciborium's own syntax and I/O
  errors and have no inner message to prefer). This is what the check
  command's `! grep 'detail: error.to_string()'` gate stands for.
- Test `a_meta_integer_past_the_formats_integer_range_names_its_field` in
  `meta/rejection_tests.rs`: a `pad_len` of 2^63 yields a `MalformedMeta`
  whose detail contains `pad_len` and the number and does not contain
  `Semantic`; the existing
  `a_meta_integer_past_the_formats_integer_range_is_malformed` stays.

### 3. TypeScript writers refuse what their reader refuses (`frontend/packages/domain/format`)

- One helper beside `asUint` in `internal/cbor.ts` — a bounded unsigned
  writer that takes the map, key, value, and the caller's encode-failure
  code (`meta_encode_failed` / `control_payload_encode_failed`, whichever
  the caller already uses) and refuses a `bigint` past `MAX_FORMAT_INTEGER`
  with a message naming the key and the value. Route `encodeExtent`,
  `meta.ts`'s `pad_len` and `schema`, and `wireContainer.ts`'s
  `ciphertext_len` through it. `writeU64BE` for the header generation needs
  no guard beyond `Generation`'s own bound; leave it.
- One test per encoder family titled with the phrase "refuses to write an
  integer past the integer range the format admits" (at least
  `meta.test.ts` for an extent or `pad_len`, and one control-object test for
  `ciphertext_len`), asserting the encode-failure code.

### Out of scope

- The "address space" vocabulary (a separate documentation task defines the
  term in FM-9).
- Any spec text: FM-19 already states every rule this task enforces.
- Renaming `Error::ExtentPastTheAddressSpace`.
- The interop exchange: it carries only objects both sides accept;
  refusals are proven by each side's unit tests.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A Recovery Code or a stored Master Key whose epoch bytes spell 0 or a
      number at or past 2^63 is refused with a format-level variant naming
      the carrier and the number, with tests on both sides
- [x] A meta-section integer past the bound is `MalformedMeta` whose detail
      names the field and the value without ciborium's `Semantic(…)`
      wrapping; the catalog-entry maps in control payloads report the same
      way
- [x] The TypeScript encoders refuse to write a CBOR unsigned integer past
      the bound, with tests
- [x] `make check` (backend fmt / build / test / clippy, frontend, interop) is
      green
