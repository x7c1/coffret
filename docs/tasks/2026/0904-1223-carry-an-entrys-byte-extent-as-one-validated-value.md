---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rq 'pub struct EntryExtent' backend/crates/domain/coffret-model/src && grep -rq 'pub extent: EntryExtent' backend/crates/domain/coffret-model/src && ! grep -rq 'pub offset: u64' backend/crates/domain/coffret-model/src && ! grep -rq 'pub size: u64' backend/crates/domain/coffret-model/src/entry_metadata.rs && grep -rq 'pub struct CiphertextLenClaim' backend/crates/domain/coffret-model/src && ! grep -rq 'entry.offset + entry.size' backend/crates/domain/coffret-usecase/src/fetch && ! grep -rq 'offset += plan.size' backend/crates/domain/coffret-usecase/src && ! grep -rq 'fn entry_table' backend/crates/domain/coffret-usecase/src/freeze && ! grep -rqE 'as usize' backend/crates/domain/coffret-usecase/src/fetch/range_read.rs && grep -rq 'an_extent_past_the_end_of_the_address_space_cannot_exist' backend/crates/domain/coffret-model/src && grep -rq 'a_zero_length_extent_is_an_extent' backend/crates/domain/coffret-model/src && grep -rq 'an_extent_answers_its_end_range_and_what_it_contains' backend/crates/domain/coffret-model/src && grep -rq 'an_entry_extent_past_the_end_of_the_address_space_is_rejected' backend/crates/domain/coffret-format/src/meta && grep -rq 'an_entry_extent_past_the_end_of_the_address_space_is_rejected' backend/crates/domain/coffret-format/src/control/journal_record && grep -rq 'an_entry_extent_past_the_end_of_the_address_space_is_rejected' backend/crates/domain/coffret-format/src/control/index_snapshot && grep -rq 'a_row_whose_extent_passes_the_end_of_the_address_space_makes_the_catalog_unreadable' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'the_table_a_pack_records_is_the_one_its_encoder_wrote' backend/crates/domain/coffret-usecase/src && grep -q 'address space' docs/spec/format/README.md"
assignee: null
branch: task/0904-1223-carry-an-entrys-byte-extent-as-one-validated-value
created_at: 2026-09-04T12:23:42Z
updated_at: 2026-09-04T15:30:08Z
---

# refactor(model): carry an Entry's byte extent as one validated value

## Overview

An Entry's place in its Container's plaintext stream is two raw `u64`
fields on `EntryMetadata` (`backend/crates/domain/coffret-model/src/entry_metadata.rs`):
`offset` and `size`. The one condition a single Entry's extent has —
`offset + size` does not overflow — is checked in two places and nowhere
else: the meta-section decoder walks the table with `checked_add`
(`backend/crates/domain/coffret-format/src/meta/decode.rs`, 51-60) and
`Layout::plan` re-assigns every offset with `checked_add`
(`backend/crates/domain/coffret-format/src/layout.rs`, 55-58). A Journal
record's additions, an Index Snapshot's entries
(`backend/crates/domain/coffret-format/src/control/wire_catalog_entry.rs`),
and every catalog row (`backend/crates/gateway/coffret-sqlite-index/src/rows.rs`,
111-114) construct `EntryMetadata` from two bare integers with no check at
all, so an extent whose end lies past `u64::MAX` reaches the Index and is
handed to the fetch — which then does the arithmetic itself, unchecked, at
six sites:

- `backend/crates/domain/coffret-usecase/src/fetch/range_read.rs:73` and
  `:138` — `entry.offset..entry.offset + entry.size`, computed twice;
- `range_read.rs:173` — `position..position + plaintext.len() as u64`;
- `range_read.rs:178-180` — `(from - piece.start) as usize`,
  `(to - from) as usize`, `plaintext[offset..offset + len]`, where an
  `as usize` on a value derived from a `u64` extent truncates on a 32-bit
  target;
- `backend/crates/domain/coffret-usecase/src/fetch/placement.rs:124` —
  `self.entry.offset + self.entry.size` (`end()`, which `scatter.rs`
  compares against its stream position).

The writer side computes the same table twice: `freeze/spool.rs`'s
`entry_table` (`backend/crates/domain/coffret-usecase/src/freeze/spool.rs`,
165-184) walks the plans with an unchecked `offset += plan.size` to build
the Journal record's entry table, while the encoder it just drove
(`ContainerWriter::begin` → `Layout::plan`) assigned the very same offsets
with `checked_add` and wrote them into the meta section. Two computations
of one table are two chances for the record and the Container to disagree.

`ContainerSummary.ciphertext_len` is a different kind of value: it is what
a Journal record *claims* the ciphertext length is, and nothing verifies it
before it is used — its only consumers are a log line
(`backend/crates/domain/coffret-usecase/src/fetch/container.rs:83`) and the
upload's own measured length (`upload/run.rs:44`, which is the writer's own
number). The type does not say it is a claim.

Make the extent one validated value, make the format layer the only place a
table's offsets are assigned, and let the type say what is verified and what
is merely claimed.

### 1. The types (`coffret-model`)

- **`EntryExtent`** (its own module, `entry_extent.rs`): `offset` and
  `size`, both `u64`, private, with a fallible constructor
  `EntryExtent::new(offset, size) -> Result<Self>` that refuses
  `offset + size` overflowing `u64` with a new
  `Error::ExtentPastTheAddressSpace { offset, size }` (name it for *why*;
  both values are plain integers and carry no Library content, so the
  `Redacted` rendering may carry them). Zero-length extents are valid
  (`sync/spool.rs` and `Layout::plan` already rely on that). Provide the
  accessors the callers need and nothing more: `offset()`, `size()`,
  `end()` (exclusive), `range() -> Range<u64>`, `contains(position)`, and
  an infallible `from_start(size)` for the extent at offset zero (which
  cannot overflow). Whether to add a `following(size) -> Result<Self>` for
  the tiling walk in `Layout::plan` is the implementer's call; if it is
  added, `Layout::plan` is its only production caller.
- **`EntryMetadata.offset` / `.size` → `EntryMetadata.extent: EntryExtent`.**
  The struct stays a plain public-field record: it has no cross-field
  condition of its own once the leaf is validated. `EntryLocation::extent()`
  returns the `EntryExtent` (by value or reference) instead of a `(u64, u64)`
  tuple; its two callers in `index_conformance/paths.rs` compare extents.
- **`CiphertextLenClaim`** (its own module): a newtype around `u64` for
  `ContainerSummary.ciphertext_len`, whose name and doc say that it is what
  the record claimed and that nothing has checked it against the object —
  a fetch verifies the ciphertext by its hash and its authenticated chunks,
  not by this number. Accessor `get()`; no validation, because every `u64`
  is a possible claim. The upload's `len` (`upload/run.rs:44`) is the
  writer's own measurement and reads the claim's value through the accessor
  where it is the same value.

### 2. Every constructor of an extent goes through `EntryExtent::new`

- **Meta section** (`meta/wire_meta_entry.rs::to_metadata`): the refusal
  maps to `coffret_format::Error::StreamTooLong` (the variant the table
  walk already uses for the same overflow, so one Container yields one
  error whichever check catches it). The tiling walk in `meta/decode.rs`
  keeps checking contiguity but reads `entry.extent.offset()` and
  `entry.extent.end()` instead of adding.
- **Journal record and Index Snapshot entries**
  (`control/wire_catalog_entry.rs::to_metadata`): the same mapping. This is
  the first time either payload refuses an overflowing extent; the tiling
  of a Journal addition's table as a whole (FM-9's contiguity, FM-10's
  non-emptiness) is an aggregate rule and stays out of this change.
- **Catalog rows** (`rows.rs`): `EntryExtent::new` through
  `unreadable_model`, so a row that overflows makes the catalog
  `UnreadableCatalog` like a malformed path does. The signed / unsigned
  bit-pattern cast (`to_integer` / `from_integer`) is a separate change and
  stays as it is.
- **Writers**: `sync/spool.rs` uses `EntryExtent::from_start(size)`;
  `format/encode.rs` and `format/entry_plan.rs::to_metadata` take the
  extent `Layout::plan` assigns (see 3); the interop generator's
  `entry(path, offset, size)` helper builds through `new` and unwraps with a
  message saying the literal is the generator's own.

### 3. One table, assigned once

`Layout::plan` is the only place an offset is assigned. Make the format
writer hand the table it wrote back to its caller — `ContainerWriter` (and
`encode`'s `EncodedContainer`, if it does not already) expose the
`Vec<EntryMetadata>` the meta section records, with the extents the layout
assigned — and delete `freeze/spool.rs::entry_table`: the `SpooledContainer`'s
`entries` are what the encoder wrote, taken from the writer after `finish`,
not recomputed from the plans. `EntryPlan::to_metadata(offset)` becomes
`to_metadata(extent)` or is folded into the layout, whichever leaves one
assignment site; `EntryPlan.size` stays a `u64` (a plan has no offset yet,
which the plan's own doc already explains). Add a usecase test
`the_table_a_pack_records_is_the_one_its_encoder_wrote` (beside the freeze
conformance cases, or as a unit test in `freeze/spool.rs`) that opens the
spooled Pack's meta section and asserts its entry table equals the
`SpooledContainer.entries` the run reported — extents included.

### 4. The fetch stops doing arithmetic on extents

- `range_read.rs`: `outline.chunks_covering(entry.extent.range())`; the
  wanted range is the same call, made once. The piece-walking in
  `write_entry` (`position`, `piece`, `from`, `to`, and the slice into
  `plaintext`) is rewritten so that no `as usize` is applied to a value
  derived from a `u64`: the two slice bounds are differences that are by
  construction at most `plaintext.len()`, so `usize::try_from` with the
  overflow treated as the impossible case it is (an explicit error, not a
  silent cast), or a helper on `EntryExtent` / a small local type that
  intersects an extent with a piece and yields `usize` bounds. `position`
  advances with `checked_add` against a `FormatError::StreamTooLong`, or an
  equivalent in the fetch's own vocabulary, rather than `+`.
- `placement.rs`: `start()` / `end()` read the extent; `scatter.rs` is
  unchanged except where it named `offset` / `size`.
- The `debug!` lines that log `bytes = asked.end - asked.start` are fine —
  that is a `Range` the outline validated.

### 5. Spec

`docs/spec/format/README.md`, FM-9: add a sub-bullet stating that an
entry's `offset` and `size` describe a range inside the plaintext stream's
64-bit address space — `offset + size` does not overflow — and that a
reader refuses a meta section, a Journal record, or an Index Snapshot whose
entry violates it, the same way it refuses a table that does not tile (the
check command greps the spec for the words "address space"). No concept
document changes; do not edit `docs/concepts/`.

### 6. Tests that fix the rule

Use these names (the check command greps for them, anchored to directories):

- `coffret-model`: `an_extent_past_the_end_of_the_address_space_cannot_exist`
  (`u64::MAX` / 1, `u64::MAX - 1` / 2, and `1` / `u64::MAX` refused with the
  new variant carrying both values), `a_zero_length_extent_is_an_extent`
  (offset `n`, size 0: `end() == n`, `range()` empty, `contains(n)` false),
  `an_extent_answers_its_end_range_and_what_it_contains`.
- `coffret-format`: `an_entry_extent_past_the_end_of_the_address_space_is_rejected`
  in `meta/rejection_tests.rs`, `control/journal_record/rejection_tests.rs`,
  and `control/index_snapshot/rejection_tests.rs` (tamper `offset` to
  `u64::MAX` the way the path cases tamper the path; expect `StreamTooLong`
  for the meta section, and whichever variant the control decoders map to —
  the same one for both).
- `coffret-sqlite-index`, `tests/`:
  `a_row_whose_extent_passes_the_end_of_the_address_space_makes_the_catalog_unreadable`
  (write the row with `rusqlite` the way the malformed-path case does).
- `coffret-usecase`: `the_table_a_pack_records_is_the_one_its_encoder_wrote`
  (section 3).

Existing tests that built an `EntryMetadata` with literal `offset` / `size`
(18 files) move to `EntryExtent::new(..).expect(..)` or a fixture helper
beside the crate's existing `entry_path` helper; where a fixture deliberately
built an overlapping or non-contiguous table to test a reader, that intent
is unchanged — only overflow is impossible now.

### Out of scope

- `ContainerAddition`'s aggregate rules (non-empty table, contiguous tiling
  from zero, no duplicate paths) and the other aggregate constructors —
  the next change.
- The SQLite signed / unsigned round-trip and the provider `ObjectInfo`
  values — the change after that.
- `pad_len`'s relation to the Padmé bucket (`meta/mod.rs`): a Container-level
  value, not an Entry's extent; unchanged here.
- The TypeScript format implementation: it already refuses an overflowing
  table with `stream_too_long` (bigint arithmetic) and the interop suite
  exchanges no overflowing extent, so nothing on that side changes; `make
  interop` (inside `make check`) must stay green.
- The storage format: a Container, record, or Snapshot that a conforming
  writer produced never carries an overflowing extent (its own `checked_add`
  would have refused it), so refusing one on read narrows nothing the spec
  allowed.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `EntryMetadata` carries `extent: EntryExtent` and no raw `offset` /
      `size`; `EntryExtent::new` refuses `offset + size` overflow with
      `Error::ExtentPastTheAddressSpace`, and zero-length extents are valid
- [x] `ContainerSummary.ciphertext_len` is a `CiphertextLenClaim` whose name
      and doc say it is unverified
- [x] A meta section, a Journal record, an Index Snapshot, and a catalog row
      whose entry extent overflows are refused (`StreamTooLong` /
      `UnreadableCatalog`), with the tests named in section 6
- [x] The fetch does no unchecked `+` on an extent and no `as usize` on a
      value derived from one; `range_read.rs` computes the wanted range once
      through `EntryExtent::range`
- [x] `freeze/spool.rs::entry_table` is gone; the Journal record's entry
      table is the one the encoder wrote, proven by
      `the_table_a_pack_records_is_the_one_its_encoder_wrote`
- [x] FM-9 carries the address-space sub-bullet
- [x] `make check` (backend fmt / build / test / clippy, frontend, interop) is
      green
