---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'FM-19' docs/spec/format/README.md && ! grep -rq '64-bit address space\\|64-bit plaintext address space' docs/spec docs/concepts backend/crates frontend/packages/domain/format/src && ! grep -q 'pub const fn new(generation: u64) -> Self' backend/crates/domain/coffret-model/src/generation.rs && ! grep -q 'pub const fn new(len: u64) -> Self' backend/crates/domain/coffret-model/src/ciphertext_len_claim.rs && grep -rq 'a_generation_past_the_formats_integer_range_is_refused' backend/crates/domain/coffret-model/src && grep -rq 'an_extent_ending_past_the_formats_integer_range_is_refused' backend/crates/domain/coffret-model/src && grep -rq 'a_header_generation_past_the_formats_integer_range_is_refused' backend/crates/domain/coffret-format/src && grep -rq 'a_payload_integer_past_the_formats_integer_range_is_malformed' backend/crates/domain/coffret-format/src && grep -rq 'a_meta_integer_past_the_formats_integer_range_is_malformed' backend/crates/domain/coffret-format/src && grep -rq 'past the integer range the format admits' frontend/packages/domain/format/src && ! grep -rq 'a_value_past_the_catalogs_integer_range_is_refused_on_write' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'an_observed_size_past_the_catalogs_integer_range_is_refused_on_write' backend/crates/gateway/coffret-sqlite-index/tests"
assignee: null
branch: task/0905-0421-bound-every-unsigned-integer-the-format-carries-below-two-to-the-63
created_at: 2026-09-05T04:22:47Z
updated_at: 2026-09-05T08:32:37Z
---

# fix(format): bound every unsigned integer the format carries below 2^63

## Overview

The format spec today lets every unsigned integer it carries use the full
64-bit range: a control header's `generation` (FM-11), and every CBOR
unsigned integer in a meta section or a control payload — `schema`, `pad_len`,
`offset`, `size`, `prev`, `head_generation`, `journal_generation`,
`keyring_generation`, `base_head_generation`, `master_key_epoch`,
`ciphertext_len`, `container` (FM-9, FM-13, FM-15, FM-16, FM-17). FM-9 even
spells that out for extents: "the plaintext stream's own 64-bit address
space: `offset + size` does not overflow".

The last change to the SQLite catalog (`IndexError::UnrepresentableValue`)
made the catalog refuse a value at or past `2^63` on write, because a
catalog column is a signed 64-bit integer. That left the register saying
one thing and one reader another: a Container or a Snapshot whose extent
starts at `2^63` is a well-formed object by the spec, and a device that
cannot catalog it. The two must agree, and the right place to make them
agree is the spec: no Library gets anywhere near `2^63` — a generation
counts commits, an epoch counts rotations, an offset lies inside an object
that every Storage caps at a few terabytes — so the range above it buys
nothing, while bounding it lets every implementation hold what the format
says in a signed 64-bit integer (a SQLite column, a Java `long`, a JSON
number's integer part) without reinterpreting a sign.

Add one format rule that bounds the integers, make the readers hold it at
the point where a wire integer becomes a value — the Rust and TypeScript
readers alike, since the two must accept the same objects — and let the
domain's leaf types carry the bound as an invariant so that nothing past
them ever has to refuse a value the format admits.

### 1. The spec (`docs/spec/format/README.md`)

- **Append `FM-19`** after FM-18 (rule IDs are appended, never renumbered —
  `docs/spec/README.md`). The statement: every unsigned integer format v1
  carries in 64 bits — the header's `generation` (FM-11) and every CBOR
  unsigned integer of a meta section (FM-9) or a control payload (FM-13,
  FM-15, FM-16, FM-17) — is below `2^63`; a reader rejects an object carrying
  a larger one as malformed, whether the value arrives in a header, a meta
  section, or a payload. `*(Form: test)*`. Sub-bullets, in the register's
  own voice (a rule states what and why, decisions stay in design records):
  - Why the bound is `2^63`: the value then fits the widest integer many
    hosts have — a signed 64-bit one — so an implementation keeping a
    catalog, or reading the format from a language without unsigned
    integers, holds what the format says without a reinterpretation; and
    nothing the format counts approaches it (a generation counts commits, an
    epoch counts rotations, an offset lies inside an object a Storage caps
    far below), so the range above buys no Library anything.
  - The bound covers positions as well as counts: an entry's end,
    `offset + size`, is a position in the plaintext stream and lies below
    `2^63` too (FM-9), so every extent has an end that is a position the
    format admits.
  - A control object's name spells its generation in decimal (FM-12); a name
    spelling a generation the format does not admit names no object, and a
    reader refuses it as it refuses a name with a leading zero.
  - Fields the format already spells in fewer bits (the header's replica
    index and count, FM-11) are bounded by their width and need nothing
    more; the rule is about the 64-bit ones.
- **Revise FM-9's extent sub-bullet**: replace the "64-bit address space"
  phrasing with the bound FM-19 states — `offset + size` is below `2^63`, so
  every entry has an end that is a position in the stream — keeping the rest
  of the bullet (a writer refuses a table that would need more; a reader
  rejects the entry wherever the entry map is carried). Do not touch the
  encoding preamble ("Multi-byte integers are big-endian throughout"): it is
  not a rule and carries no ID.
- Concept docs under `docs/concepts/` do not speak of the address space
  (grep `64-bit address` before assuming), so none needs a change; if one
  does, change the English source only — the Japanese mirrors are regenerated
  elsewhere.

### 2. The model's leaf types (`coffret-model`)

State the bound once, as one named constant the crate exports (the largest
integer the format admits, `2^63 - 1`), and have every leaf type refer to it
rather than to `i64::MAX`:

- **`Generation::new(u64)` becomes fallible** (`Result<Self>`), refusing a
  number past the bound with `Error::GenerationOutOfRange`; `next` refuses at
  the bound rather than at `u64::MAX`. `FIRST` stays a constant. The
  existing unit variant may grow the offending number — a generation is the
  format's own arithmetic, not Library content, so it belongs in the
  message — decide from what the `Display` needs to say. Every call site
  moves: production (`coffret-format` decoders and `header.rs`, the SQLite
  and in-memory Index, `coffret-usecase`'s control head and catch-up,
  `journal_record/new.rs`'s predecessor computation, the interop generator's
  literals — those take an `expect` naming why the literal is inside the
  bound, matching the crate's existing style) and tests (a fixture helper in
  the model's `testing.rs` or beside the existing fixture leaves keeps the
  test bodies short).
- **`MasterKeyEpoch::new`** refuses past the bound as it refuses zero;
  `next` refuses at the bound.
- **`EntryExtent::new` / `following`** refuse an end past the bound (the end
  is `offset + size`; it must itself be below `2^63`), still as
  `Error::ExtentPastTheAddressSpace` — the address space is now the one
  FM-19 bounds, and the type's doc, the variant's doc, and its `Display` say
  so instead of "64-bit". `from_start` stays infallible only if a `size` at
  or past the bound is refused somewhere before it; if that cannot be
  guaranteed at the type, make it fallible too.
- **`CiphertextLenClaim::new` becomes fallible**: its doc says "every `u64`
  is a possible claim, so there is nothing here to refuse", which FM-19 makes
  untrue. Give the refusal a named variant (or share one with the other
  plain counts if that reads better than a variant per type) and move the
  ~15 call sites.
- **`control_object_name/parse.rs`**: a name whose decimal generation is past
  the bound is a malformed name (FM-12's verdict), not a generation error
  passed through.
- Model tests: `a_generation_past_the_formats_integer_range_is_refused` (at
  exactly `2^63`; `2^63 - 1` is accepted and has no successor),
  `an_extent_ending_past_the_formats_integer_range_is_refused` (an extent
  ending at `2^63` is refused, one ending at `2^63 - 1` is not), and the
  matching cases for the epoch, the claim, and the object name.

### 3. The Rust readers (`coffret-format`, `coffret-interop`)

- **Control payloads**: `control/cbor/fields.rs::as_uint` is where every
  CBOR unsigned integer of a payload becomes a `u64`; bound it there, with a
  detail that says what was expected ("`<key>` is an unsigned integer below
  2^63, found …"), so `schema`, `prev`, the generations, `ciphertext_len`,
  and `container` are all covered by one check. `payload/decode.rs` reads
  `master_key_epoch` with its own copy of that conversion; make it use the
  same bounded reading (or the same helper) rather than a second spelling.
- **Control header**: `control/header.rs` builds `Generation::new(…)` from
  the 8 header bytes; a generation past the bound is a format refusal that
  names the header's generation (a new `Error` variant carrying the number,
  in the family of `ControlHeaderTooShort` / `ReservedNotZero`), not a
  `Model` passthrough — the format states this rule, so the format names
  the refusal.
- **Meta section**: `meta/wire_meta.rs` and `wire_meta_entry.rs` are
  serde-derived with `u64` fields, so ciborium accepts the full range. Bound
  `schema`, `pad_len`, `offset`, and `size` in one place (a bounded wire
  integer type with its own `Deserialize`, or one check as the wire struct
  becomes `Meta` — pick the one that keeps the bound stated once) and report
  it as `MalformedMeta`. Extents keep flowing through `stream_extent.rs`, so
  an entry whose end is past the bound stays `StreamTooLong`; that variant's
  `Display` and every doc that says "64-bit address space" say the FM-19
  bound instead.
- **Interop**: the fixture exchange carries only objects both sides accept,
  so no rejection fixture is added; the generator's and verifier's literals
  and `expect` messages move with the API. `make interop` (inside
  `make check`) must stay green.
- Format tests: `a_header_generation_past_the_formats_integer_range_is_refused`
  (in `header.rs`'s tests: generation bytes `80 00 00 00 00 00 00 00`),
  `a_payload_integer_past_the_formats_integer_range_is_malformed` (in
  `fields.rs`'s tests, a map with a `2^63` integer, naming the key), and
  `a_meta_integer_past_the_formats_integer_range_is_malformed` (in
  `meta/rejection_tests.rs`: a `pad_len` of `2^63` is `MalformedMeta`, and an
  entry whose extent ends at `2^63` is `StreamTooLong`).

### 4. The TypeScript readers (`frontend/packages/domain/format`)

The two implementations must accept and refuse the same objects, so mirror
the bound where the TypeScript side turns a wire integer into a value:

- `internal/cbor.ts::asUint` (behind `requiredUint` / `optionalUint`)
  refuses a value at or past `2^63` with the caller's code and a message
  saying what was expected — this covers every payload and meta integer.
- `internal/bytes.ts` gains the bound as one exported constant beside
  `U64_MAX` (which stays for the 8-byte fields' writer-side checks);
  `model/generation.ts` and `model/masterKeyEpoch.ts` refuse past it in `of`
  and `next`; `internal/entryFields.ts::decodeExtent` refuses an end past it
  as `stream_too_long`; `control/header.ts`'s generation goes through
  `Generation.of` and so is refused with `generation_out_of_range`.
- Tests beside the existing ones, titled with the phrase "past the integer
  range the format admits" so the check command finds them: one for `asUint`
  (through a payload decoder), one for the header generation, one for an
  extent ending at `2^63`, one each for `Generation.of` / `MasterKeyEpoch.of`
  at `2^63` (accepted at `2^63 - 1`, no successor there).

### 5. The SQLite catalog (`coffret-sqlite-index`, `coffret-usecase`)

- After sections 2–3, every format value the catalog stores is below `2^63`
  by construction; the one column that is not format-bounded is
  `local_entries.observed_size`, which comes from the device's own
  `metadata.len()`. Keep `rows/columns.rs::to_integer` fallible and keep
  `IndexError::UnrepresentableValue`, but rewrite the variant's doc: the
  format bounds its integers (FM-19) and refuses larger ones at their
  constructors, so this refusal is reachable only for a device-observed size
  — a file the local filesystem reports as `2^63` bytes or more — and its
  message names that. Drop the "signed 64-bit columns" sentence from the
  port's doc (it is the gateway's detail; the gateway's helper doc may keep
  it).
- Reshape the existing `a_value_past_the_catalogs_integer_range_is_refused_on_write`
  (its `2^63` extent can no longer be built) into
  `an_observed_size_past_the_catalogs_integer_range_is_refused_on_write`: a
  `LocalObservation` whose `size` is `2^63` is refused by the device-state
  write with `UnrepresentableValue { column: "observed_size", .. }`. Keep
  `a_refused_write_leaves_the_catalog_as_it_was` meaningful — if it relied
  on the extent case, drive it from the observed size the same way, or
  drop it with a comment saying the format now refuses first, whichever
  keeps a true statement.

### Out of scope

- The SQLite schema and its version: no stored representation changes.
- Signed integers (`original_mtime`, `original_btime`, `mtime`, `btime`):
  already 64-bit signed and unaffected.
- The `u16` / `u32` header fields (replica index and count, chunk size, meta
  section length): bounded by their width.
- A rejection fixture in the interop exchange: the exchange proves both
  sides accept the same well-formed objects; refusals are proven by each
  side's unit tests.
- Any other spec rule; migrating FM-19 to `Form: test` ownership is the
  register's ordinary flow and not this task.
- Documents outside `docs/spec` and `docs/concepts` that quote the old
  wording (earlier task files under `docs/tasks/` are history and stay as
  written).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `docs/spec/format/README.md` carries `FM-19` bounding every 64-bit
      unsigned integer the format carries below `2^63`, and FM-9's extent
      sub-bullet cites that bound instead of a "64-bit address space"
- [x] `Generation::new`, `MasterKeyEpoch::new`, `CiphertextLenClaim::new`,
      and `EntryExtent::new` / `following` refuse values past the bound,
      stated once as a named model constant
- [x] The Rust readers refuse a header generation, a payload integer, and a
      meta-section integer at or past `2^63` as format errors, and the tests
      named in sections 2–3 exist under the directories the check command
      greps
- [x] The TypeScript readers refuse the same values, with tests titled "past
      the integer range the format admits"
- [x] The SQLite write refusal is reachable only for `observed_size`, its doc
      says so, and the reshaped test exists
- [x] No spec, concept, backend, or frontend source says "64-bit address
      space" any more
- [x] `make check` (backend fmt / build / test / clippy, frontend, interop) is
      green
