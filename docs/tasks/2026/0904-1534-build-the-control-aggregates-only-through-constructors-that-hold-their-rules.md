---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && ! grep -rqE '^\\s*pub (generation|prev|additions|removals|checkpoint|adopted_from|containers|entries|container|head_generation|journal_generation|master_key_epoch|keyring|next_commit_slot|snapshot_slot): ' backend/crates/domain/coffret-model/src/journal_record* backend/crates/domain/coffret-model/src/index_checkpoint* backend/crates/domain/coffret-model/src/snapshot_content* backend/crates/domain/coffret-model/src/keyring_mapping* backend/crates/domain/coffret-model/src/container_addition* && ! grep -rq 'fn canonical(mut self)' backend/crates/domain/coffret-model/src && ! grep -rq 'require_strictly_increasing' backend/crates/domain/coffret-format/src/control/journal_record backend/crates/domain/coffret-format/src/control/index_snapshot backend/crates/domain/coffret-format/src/control/keyring && ! grep -rqE 'sort_by|\\.sort\\(\\)' backend/crates/domain/coffret-format/src/control/journal_record/encode.rs backend/crates/domain/coffret-format/src/control/index_snapshot/encode.rs backend/crates/domain/coffret-format/src/control/keyring/encode.rs && grep -rqE '^\\s+generation: Generation,$' backend/crates/domain/coffret-format/src/control/index_snapshot/decode.rs && ! grep -rq 'stands_at != record.generation' backend/crates/domain/coffret-usecase/src/commit && grep -rq 'a_journal_record_names_its_predecessor_or_cannot_exist' backend/crates/domain/coffret-model/src && grep -rq 'a_journal_record_out_of_canonical_order_cannot_exist' backend/crates/domain/coffret-model/src && grep -rq 'an_addition_with_no_entries_cannot_exist' backend/crates/domain/coffret-model/src && grep -rq 'an_addition_whose_entries_do_not_tile_cannot_exist' backend/crates/domain/coffret-model/src && grep -rq 'a_checkpoint_whose_journal_is_ahead_of_its_head_cannot_exist' backend/crates/domain/coffret-model/src && grep -rq 'a_snapshot_out_of_canonical_order_cannot_exist' backend/crates/domain/coffret-model/src && grep -rq 'a_snapshot_whose_entry_names_no_container_cannot_exist' backend/crates/domain/coffret-model/src && grep -rq 'a_keyring_mapping_naming_a_container_twice_cannot_exist' backend/crates/domain/coffret-model/src && grep -rq 'canonical_sorts_and_then_holds_to_the_same_rule' backend/crates/domain/coffret-model/src && grep -rq 'an_addition_with_no_entries_is_rejected' backend/crates/domain/coffret-format/src/control/journal_record && grep -rq 'an_addition_whose_entries_do_not_tile_is_rejected' backend/crates/domain/coffret-format/src/control/journal_record && grep -rq 'a_snapshot_that_checkpoints_another_head_is_rejected' backend/crates/domain/coffret-format/src/control/index_snapshot && grep -rq 'a_checkpoint_whose_journal_is_ahead_of_its_head_is_rejected' backend/crates/domain/coffret-format/src/control/index_snapshot && grep -rq 'a_snapshot_that_checkpoints_another_head_is_not_adopted' backend/crates/domain/coffret-usecase/src && grep -rq 'a_checkpoint_row_whose_journal_is_ahead_of_its_head_makes_the_catalog_unreadable' backend/crates/gateway/coffret-sqlite-index/tests"
assignee: null
branch: task/0904-1534-build-the-control-aggregates-only-through-constructors-that-hold-their-rules
created_at: 2026-09-04T15:34:43Z
updated_at: 2026-09-04T17:48:01Z
---

# refactor(model): build the control aggregates only through constructors that hold their rules

## Overview

The five control aggregates in `coffret-model` — `JournalRecord`,
`IndexCheckpoint`, `SnapshotContent`, `KeyringMapping`, and
`ContainerAddition` — are plain structs with every field `pub`, so any
caller can write a literal that breaks a rule the reader depends on. The
rules exist, but each lives somewhere else, and several live in more than
one place:

- **Canonical order** (`additions` / `removals` by Container ID,
  `containers` by ID, `entries` by Entry Path bytes, a Keyring `mapping`
  by ID; FM-15, FM-16, FM-17, EP-3) is checked by the three decoders
  through `control/canonical_order.rs`, re-imposed by the three encoders
  (`sort_by_key` / `sort` in `journal_record/encode.rs`,
  `index_snapshot/encode.rs`, `keyring/encode.rs`), produced by the SQLite
  gateway's `ORDER BY` (`library_state.rs::snapshot`), and offered once more
  by `SnapshotContent::canonical`, which no production code calls. Four
  statements of one rule; the model type, which is what every one of them
  hands around, states none of it.
- **`prev == generation − 1`** (FM-15) is checked by the Journal record
  decoder only; the writer in `commit/journal.rs` assembles both fields by
  hand.
- **`journal_generation ≤ head_generation`** (CK-1) is checked nowhere: the
  Snapshot decoder, the SQLite `checkpoint` row reader (`rows.rs`), and the
  in-memory Index each build an `IndexCheckpoint` from raw values.
- **An addition's entry table** is non-empty (FM-10) and tiles the
  Container's plaintext stream from offset 0 (FM-9). The meta-section
  decoder checks both for a Container; the Journal record decoder checks
  neither, so a record whose addition carries no entries, or entries with a
  gap, applies to the Index.
- **A Snapshot's entries name a Container the Snapshot lists** (FM-16) is
  checked by the decoder (`DanglingContainerIndex`) and by the encoder
  (`SnapshotEntryWithoutContainer`), and by nothing between them.
- **A Keyring mapping names each Container once** (FM-17) is implied by
  the decoder's strictly-increasing check; `KeyringMapping::new` and
  `Default` accept duplicates, and the writer in `commit/keyring.rs`
  concatenates the held mapping with the batch's additions without looking.
- **A Snapshot checkpoints the head it is named for** (CK-10: an ordinary
  Snapshot's `head_generation` is the generation in its object name) is
  checked only on the CK-11 sibling read-back in `commit/settle.rs`
  (`stands_at != record.generation`); the catch-up adoption path in
  `commit/catch_up.rs` calls `decode_index_snapshot(payload, kind)` — which,
  unlike `decode_journal_record(payload, generation)`, is not told the
  name's generation — and restores whatever the payload claims, so the
  Index's checkpoint and its recorded starting point can disagree.

Make each aggregate the one place its own rules are stated, and make the
Snapshot decoder the one place the name ⇔ payload generation rule is stated.

### 1. The aggregates (`coffret-model`)

Every one of the five gets private fields, accessor methods for what the
fields exposed (`generation()`, `prev()`, `additions()`, `removals()`,
`checkpoint()`, `containers()`, `entries()`, `head_generation()`, …), and a
**strict** constructor that returns `Result<Self>` and refuses anything the
rules below exclude. Struct literals become impossible outside the module.

- **`ContainerAddition::new(container, entries)`** — `entries` is non-empty
  (FM-10) and tiles from offset 0 without gaps or overlaps (FM-9), which
  `EntryExtent::end()` makes a walk of `extent.offset() == expected` /
  `expected = extent.end()`. Entry Paths within one addition are distinct
  (EP-6's per-record uniqueness stays in `commit/candidate.rs`, which checks
  it against the whole Index).
- **`IndexCheckpoint::new(master_key_epoch, head_generation,
  journal_generation, next_commit_slot, keyring)`** — `journal_generation ≤
  head_generation` (CK-1). `JournalRecord::checkpoint()` keeps building the
  equal-generation case and cannot fail.
- **`JournalRecord::new(generation, prev, master_key_epoch, keyring,
  next_commit_slot, snapshot_slot, additions, removals)`** — `prev` is
  `Some(generation − 1)`, or `None` exactly at generation 0 (FM-15);
  `additions` strictly increasing by Container ID and `removals` strictly
  increasing by ID (FM-15). (Whether an ID may appear in both is not a spec
  rule; do not add one.)
- **`SnapshotContent::new(checkpoint, adopted_from, containers, entries)`**
  — `containers` strictly increasing by ID, `entries` strictly increasing
  by the canonical bytes of the Entry Path (FM-16, EP-3), and every entry's
  `container_id` names an element of `containers` (FM-16). `adopted_from`
  is the Index's own provenance and takes part in no rule (CK-7). The
  catch-up path's struct update (`SnapshotContent { adopted_from: Some(..),
  ..content }`) becomes a method that sets the provenance on a value that
  already holds.
- **`KeyringMapping::new(entries)`** — strictly increasing by Container ID
  (FM-17). `Default` (an empty mapping) stays.
- **`canonical(..)`** on `JournalRecord`, `SnapshotContent`, and
  `KeyringMapping`: the same arguments as `new`, sorts the collections
  first, then calls `new` — for this implementation's own writers, whose
  inputs arrive in scan or spool order (`commit/journal.rs` collects
  `batch.additions` in spooled order; `commit/keyring.rs` appends the
  batch's envelopes after the held mapping; the in-memory Index and the
  SQLite gateway iterate maps and rows). Sorting cannot make a duplicate
  disappear, so `canonical` refuses exactly what `new` refuses after the
  sort. The old sort-only `SnapshotContent::canonical(self)` goes.
- **Errors**: new `coffret_model::Error` variants that name *why*, carrying
  the collection and index (a `&'static str` naming `additions` /
  `removals` / `containers` / `entries` / `mapping`, and the offending
  position) for order and duplicates; the two generations for a
  predecessor or checkpoint mismatch; the entry index for a gap or an
  overlap in an addition; the entry index and the Container ID for a
  dangling Snapshot entry. None of these carries an Entry Path, so their
  `Redacted` renderings may carry every field.

### 2. Decoders hand the fields to the constructors

`journal_record/decode.rs`, `index_snapshot/decode.rs`, and
`keyring/decode.rs` stop calling `require_strictly_increasing` and stop
checking `prev` themselves: they collect the wire fields and call the strict
constructor, mapping the model's refusal onto the format crate's existing
vocabulary where a variant already exists (`ControlPayloadOutOfOrder {
array, index }` for order and duplicates, keeping its `array` names;
`JournalRecordPrevMismatch`; `DanglingContainerIndex` for a Snapshot entry
whose index is out of range — that one stays a wire-level check since the
index is a wire artefact, but the ID-level rule is the constructor's) and
adding a variant where none exists (an addition with no entries or a
non-tiling table, a checkpoint whose Journal generation is ahead of its
head). `control/canonical_order.rs` and its tests go with them, or shrink to
whatever the wire layer still needs. The existing rejection tests for order
and `prev` keep their names and keep passing.

**`decode_index_snapshot(payload, kind, generation)`** takes the name's
generation, symmetrical with `decode_journal_record(payload, generation)`,
and holds the rule that differs by kind: an ordinary Snapshot's
`head_generation` equals the name's generation (CK-10); an activation
Snapshot's `head_generation` equals the name's generation too (FM-13's one
head chain) and its `base_head_generation` names a strictly earlier head
(FM-16; the comparison of `activation_slot` with that head's
`next_commit_slot` stays with the caller, as FM-16 says). A mismatch is a
new format error naming both generations. Then:

- `commit/settle.rs` drops its own `stands_at != record.generation` check
  and the `ControlObjectFault::CheckpointsAnotherHead` variant it fed —
  the decoder's error arrives through the existing `Unopenable` path — or
  keeps the variant only if the decoder's error is mapped onto it; either
  way the check is written once.
- `commit/catch_up.rs::adoptable` passes the candidate's generation, so a
  Snapshot that checkpoints another head is not adopted, and the
  `commit_conformance` (or `commit/adversarial_store_tests.rs`) suite gains
  `a_snapshot_that_checkpoints_another_head_is_not_adopted`: a Storage
  holding `idx-N` whose payload says `head_generation = N − 1` makes the
  catch-up refuse with the corrupt-control-object error rather than restore
  it.
- The interop verifier (`apps/coffret-interop/src/verify/check_control_object.rs`)
  and `commit_conformance/library.rs` pass the generation they already have.

### 3. Encoders, gateways, writers

- The three encoders stop sorting; they serialize the collections in the
  order the type guarantees.
- SQLite: `library_state.rs::snapshot` keeps its `ORDER BY` (that is how
  rows arrive sorted) and builds through `SnapshotContent::new` via
  `unreadable_model`; `rows.rs::checkpoint` builds through
  `IndexCheckpoint::new` via `unreadable_model`, so a `checkpoint` row whose
  `journal_generation` exceeds `head_generation` is `UnreadableCatalog`.
  Writes (`restore`, `write_checkpoint`) read through the accessors.
- The in-memory Index (`in_memory_index/state.rs`) builds through
  `canonical` / `new` (its `BTreeMap`s already iterate in order).
- `commit/journal.rs` builds the record through `JournalRecord::canonical`
  (it computes `generation` and `prev` from the head; the constructor now
  confirms they agree); `commit/keyring.rs::next_generation` through
  `KeyringMapping::canonical`, so a duplicate Container — which the current
  concatenation could produce if a batch re-added a Container the held
  mapping still lists — is refused at the writer with a `CommitError` rather
  than written for every reader to reject; `spooled_container.rs` through
  `ContainerAddition::new` (the encoder's table tiles by construction, so
  map the impossible refusal to the commit error vocabulary rather than
  `expect`).
- The interop generator (`generate/control_payloads.rs`) builds its
  fixtures through the constructors, unwrapping with a message that says the
  literal is the generator's own, as its `entry` helper does.
- Every test fixture (about 35 literal sites across `coffret-format`
  testing modules, `coffret-usecase` conformance fixtures,
  `coffret-sqlite-index/tests`, `coffret-device` browse tests) moves to the
  constructors; a fixture that deliberately built an out-of-order or
  dangling value to test a reader now does so at the wire level (a tampered
  CBOR map), which is where those tests already tamper.

### 4. Tests that fix the rules

Use these names (the check command greps for them, anchored to directories):

- `coffret-model`: `a_journal_record_names_its_predecessor_or_cannot_exist`
  (`prev` absent at 0, present and equal to `generation − 1` otherwise,
  each mismatch refused), `a_journal_record_out_of_canonical_order_cannot_exist`
  (additions reversed; removals with a repeat), `an_addition_with_no_entries_cannot_exist`,
  `an_addition_whose_entries_do_not_tile_cannot_exist` (a gap, an overlap,
  a table not starting at 0), `a_checkpoint_whose_journal_is_ahead_of_its_head_cannot_exist`,
  `a_snapshot_out_of_canonical_order_cannot_exist` (containers reversed;
  entries with a repeated path), `a_snapshot_whose_entry_names_no_container_cannot_exist`,
  `a_keyring_mapping_naming_a_container_twice_cannot_exist`,
  `canonical_sorts_and_then_holds_to_the_same_rule` (an unsorted but
  valid input becomes the same value `new` accepts sorted; an unsorted input
  with a duplicate is refused by `canonical` too).
- `coffret-format`: `an_addition_with_no_entries_is_rejected` and
  `an_addition_whose_entries_do_not_tile_is_rejected` in
  `control/journal_record/rejection_tests.rs`;
  `a_snapshot_that_checkpoints_another_head_is_rejected` and
  `a_checkpoint_whose_journal_is_ahead_of_its_head_is_rejected` in
  `control/index_snapshot/rejection_tests.rs` (tamper the wire fields the
  way the existing cases do).
- `coffret-usecase`: `a_snapshot_that_checkpoints_another_head_is_not_adopted`
  (section 2).
- `coffret-sqlite-index`, `tests/`:
  `a_checkpoint_row_whose_journal_is_ahead_of_its_head_makes_the_catalog_unreadable`
  (write the row with `rusqlite` the way the malformed-path and extent cases
  do).

### Out of scope

- The Index port's `apply` refusing a record whose `prev` is not the
  catalog's current head: the catch-up walk applies heads in generation
  order and the decoder now guarantees each record's `prev`; an Index-level
  sequencing check is a separate port contract.
- The `activation_slot` ⇔ base head `next_commit_slot` comparison (FM-16
  assigns it to the caller; activation is not produced in production yet).
- `ContainerSummary`, `EntryMetadata`, `EntryLocation`, and `Mapping` stay
  plain records: they hold no cross-field rule once their leaves are
  validated.
- The SQLite signed / unsigned integer round-trip and the provider
  `ObjectInfo` values — the next change.
- The TypeScript format implementation: its decoders already verify order
  and dangling indices; nothing exchanged by the interop suite changes.
  `make interop` (inside `make check`) must stay green.
- The storage format: every rule here is one FM-9 / FM-10 / FM-15 / FM-16 /
  FM-17 / CK-1 / CK-10 already states, so refusing a violation on read
  narrows nothing the spec allowed; no spec text changes unless a rule is
  found to be unstated, in which case add it as a sub-bullet of the rule it
  belongs to.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The five aggregates have private fields, accessors, and strict
      constructors; no struct literal of them exists outside `coffret-model`
- [x] `canonical` sorts then holds to the same rule as `new`; the sort-only
      `SnapshotContent::canonical` is gone; the three encoders no longer sort
- [x] The three decoders check order, `prev`, non-empty / tiling additions,
      and the checkpoint's generation relation through the constructors, and
      `require_strictly_increasing` is no longer called by them
- [x] `decode_index_snapshot` takes the name's generation and refuses a
      Snapshot that checkpoints another head; `commit/settle.rs` no longer
      re-checks it; the catch-up does not adopt such a Snapshot
- [x] A `checkpoint` row whose Journal generation is ahead of its head makes
      the catalog `UnreadableCatalog`
- [x] The tests named in section 4 exist under the directories the check
      command greps
- [x] `make check` (backend fmt / build / test / clippy, frontend, interop) is
      green
