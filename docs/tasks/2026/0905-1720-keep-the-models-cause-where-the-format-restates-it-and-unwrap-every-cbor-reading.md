---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && ! grep -q 'map_err(|_|' backend/crates/domain/coffret-format/src/control/header.rs && ! grep -q 'map_err(|_|' backend/crates/domain/coffret-format/src/recovery_code/parse.rs && ! grep -q 'map_err(|_|' backend/crates/domain/coffret-format/src/stored_master_key/unlock.rs && ! grep -q 'malformed(error.to_string())' backend/crates/domain/coffret-format/src/control/cbor/mod.rs && ! grep -q 'detail: error.to_string()' backend/crates/domain/coffret-format/src/control/payload/decode.rs && ! grep -q 'detail: error.to_string()' backend/crates/domain/coffret-format/src/control/payload/encode.rs && ! grep -q 'fn not_this_map' backend/crates/domain/coffret-format/src/meta/decode.rs && grep -rq 'names_the_fault_without_the_wrapping' backend/crates/domain/coffret-format/src/control"
assignee: null
branch: task/0905-1720-keep-the-models-cause-where-the-format-restates-it-and-unwrap-every-cbor-reading
created_at: 2026-09-05T17:19:07Z
updated_at: 2026-09-05T17:59:57Z
---

# fix(format): keep the model's cause where the format restates it and unwrap every CBOR reading

## Overview

Two small leftovers from the last two format changes, both inside
`coffret-format`, both about how a refusal from below reaches the caller.

- **Three sites drop the model's error blind.** `control/header.rs`
  (`Generation::new(number).map_err(|_| Error::ControlHeaderGenerationOutOfRange { generation: number })`),
  `recovery_code/parse.rs` and `stored_master_key/unlock.rs` (the same shape
  around `MasterKeyEpoch::new` and the two epoch variants) restate the
  model's one refusal as the format's own — correct today, because
  `Generation::new` and `MasterKeyEpoch::new` each fail for exactly one
  reason. But `|_|` restates *whatever* the model refuses for, so a rule the
  model gains later (a second reason to refuse a generation) would be
  reported as "past the largest integer the format admits" with no way to
  tell. The crate already has the right shape in `stream_extent.rs::refusal`:
  `match` the variant this layer restates, and pass anything else through as
  `Error::Model(other)` — "a rule this layer has not been told about, and
  passing it through says so".
- **Three CBOR readings still hand ciborium's rendering to the caller.**
  `meta/decode.rs::not_this_map` takes `ciborium::de::Error::Semantic(_, message)`
  apart so `MalformedMeta { detail }` carries the message instead of
  `Semantic(None, "…")` (ciborium's `Display` is its `Debug` spelling). The
  same reading in `control/cbor/mod.rs::read_body`
  (`malformed(error.to_string())`), `control/payload/decode.rs::read_padded_map`
  and `control/payload/encode.rs` (`Error::MalformedControlPayload { detail: error.to_string() }`)
  still passes the wrapped form, so a control payload that is not CBOR, or
  whose body is not the map a schema expects, reports its fault inside a
  `Debug` wrapper with escaped quotes, while a meta section reports it
  plainly.

### 1. Restate one refusal, pass the rest through

- At each of the three sites, replace `map_err(|_| …)` with a `match` on the
  model's error: the variant this layer restates
  (`GenerationOutOfRange { .. }` → `ControlHeaderGenerationOutOfRange { generation: number }`;
  `EpochOutOfRange { .. }` → the carrier's epoch variant) and
  `other => Error::Model(other)`. Follow `stream_extent.rs::refusal` for the
  shape and its doc for the reason; a small per-site `fn` beside the reader
  (or one shared helper if the three collapse cleanly — judge which reads
  better, but do not invent a trait or a macro for three call sites) keeps
  the read site one line.
- No new tests: the passthrough arm is unreachable while the model has one
  reason to refuse, and a test that could not fail proves nothing. The
  existing refusal tests (`a_header_generation_past_the_formats_integer_range_is_refused`,
  `a_recovery_code_epoch_past_the_formats_integer_range_is_refused`,
  `a_stored_master_key_epoch_past_the_formats_integer_range_is_refused`,
  the two epoch-0 cases) keep proving the restated arm.

### 2. One place takes a CBOR reading apart

- Move `not_this_map`'s logic to one crate-level helper (a sibling of
  `bounded_uint.rs`, e.g. `cbor_reading.rs`, named for what it does) that
  takes `ciborium::de::Error<E>` and the carrier's `malformed` constructor
  and returns the format's error with the `Semantic` message unwrapped and
  the other variants rendered as they are. `meta/decode.rs`,
  `control/cbor/mod.rs::read_body`, `control/payload/decode.rs::read_padded_map`,
  and `control/payload/encode.rs`'s read-back all go through it, each passing
  its own constructor (`meta::malformed`, the schema's `malformed`, a
  `MalformedControlPayload` constructor). The doc that explains why the
  message is preferred moves with the logic; `meta/decode.rs` keeps nothing
  but the call.
- `control/cbor/mod.rs::deserialization_failed` (which unwraps
  `ciborium::value::Error::Custom`, a different error type) stays where it
  is unless folding it into the same module reads better; if it moves,
  its two callers move with it.
- Test `a_malformed_payload_body_names_the_fault_without_the_wrapping` in
  the control rejection tests (beside `a_body_that_is_not_a_map_is_rejected`
  in `control/journal_record/rejection_tests.rs`, or in
  `control/rejection_tests.rs` for the `read_padded_map` path — one is
  enough, pick the path that reaches ciborium's `Semantic` variant): the
  detail contains ciborium's message and does not contain `Semantic(`.

### Out of scope

- The serialization side (`meta/encode.rs`, `control/cbor::serialization_failed`,
  `control/payload/mod.rs`): a writer's own failure to encode is not a
  malformed object, and those details are read by developers, not matched
  on.
- Any spec text, the TypeScript side, and the concept documents.
- Renaming the variants involved.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `control/header.rs`, `recovery_code/parse.rs`, and
      `stored_master_key/unlock.rs` restate exactly the model refusal the
      format names and pass any other through as `Error::Model`
- [x] Every ciborium reading that reports a malformed meta section or control
      payload goes through one helper that unwraps the `Semantic` message,
      with a control-side test proving the detail carries no `Semantic(`
      wrapper
- [x] `make check` (backend fmt / build / test / clippy, frontend, interop) is
      green
