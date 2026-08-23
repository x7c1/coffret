---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && make s3-store-it && ! grep -rnE 'jrn-[0-9]' --include='*.rs' --include='*.ts' --include='*.md' backend frontend docs/spec docs/concepts && grep -q 'coffret/v1/control/activation-snapshot' docs/spec/key-derivation/README.md"
assignee: null
branch: task/0822-2148-kind-neutral-head-names-and-name-bound-slots
created_at: 2026-08-22T12:48:00Z
updated_at: 2026-08-22T17:16:23Z
---

# feat(backend): name the control-head chain kind-neutrally and bind commit slots to their names

## Overview

The commit protocol serializes writers through one slot per head (CP-2, CP-3):
every authenticated control head carries a `next_commit_slot`, and of the
writers that start from that head exactly one conditional create succeeds —
whether the successor is an ordinary Journal record or the Index Snapshot that
activates a new Master Key epoch. On Google Drive this holds: the slot is a
file ID minted by `files.generateIds`, and two `files.create` calls naming the
same ID are mutually exclusive whatever name each passes. **On S3 it does not
hold.** The S3 adapter's slot is `CommitSlot::by_name()` — it carries nothing —
and `put_if_absent(slot, name, body)` keys the `If-None-Match: *` PUT on the
`name` argument (`backend/crates/gateway/s3-store/src/s3.rs:184`). Today FM-12
names the two successor kinds differently (`jrn-<g+1>.cfrt` versus
`idx-<g+1>.cfrt`), so a Journal writer and an activating writer starting from
the same head hit two different keys and both succeed. CP-3 is false on S3,
and the backend-swappability the `ObjectStore` port promises is false for the
head chain.

The fix is in the naming, and the port change that accompanies it is
hardening. Both land here, with the format and spec changes they need. No
Library data exists yet, so this is the cheapest moment to rename.

### What changes

1. **FM-12 — one kind-neutral name form for the head chain.** Journal records
   and activation Index Snapshots are both named `head-<generation>.cfrt`,
   where `<generation>` is the control-head generation (FM-13: successor =
   head + 1). Ordinary Index Snapshots stay `idx-<generation>.cfrt` with the
   generation of the head they checkpoint (CK-10). Keyring replicas stay
   `key-<generation>-<set_digest>-r<index>-of-<count>.cfrt`. The name now
   expresses the object's **role** — head chain, checkpoint, Keyring — and the
   authenticated FM-11 header byte carries its **kind**. Replace FM-12's
   "name-encoded kind must agree with the header" with an admission table:
   `head-` admits kind Journal or kind Activation Snapshot (below); `idx-`
   admits only Index Snapshot; `key-` only Keyring; every other pairing is
   rejected before decryption, as today. The Library's first head, `head-0`,
   is a Journal record — there is no epoch to activate before the first
   commit; say so in FM-13.
2. **FM-11 — a fourth kind byte, `0x04` Activation Snapshot**, with its own
   purpose key `coffret/v1/control/activation-snapshot` in the KD-4 registry.
   An ordinary Snapshot (`0x03`, `idx-`) and an activation Snapshot (`0x04`,
   `head-`) carry the same checkpoint content (CK-1 to CK-3) and the
   activation one additionally carries the fields activation needs (the base
   head generation and the activation slot). Giving activation its
   own kind and key means a misfiled or renamed object — an ordinary Snapshot
   presented under `head-N`, or the reverse — is rejected by the admission
   table and by the key, before any payload is read, and recovery and old-epoch
   cleanup (MR-3) can classify objects from the plaintext header without
   opening them. Add the variant to `ControlObjectKind`
   (`backend/crates/domain/coffret-model/src/control_object_kind.rs`), to
   `Purpose` (`backend/crates/domain/coffret-format/src/purpose.rs`), to the
   TypeScript mirror, and to the interop fixtures and manifest so both
   implementations exchange an activation-kind object.
3. **The format layer stops treating the name as the single source of kind.**
   `encode_request.rs` documents the name as "the single source of the kind,
   generation, and replica position", `encode.rs` takes `request.name.kind()`,
   and `decode.rs` rejects on `name.kind() != header.kind`. With `head-`
   admitting two kinds, a name no longer determines a kind. Give the encode
   request the kind explicitly (a `Head { generation, kind }` name variant whose
   `Display` drops the kind, or a separate kind argument — choose whichever
   keeps `ControlObjectName` honest: parsing a `head-` name must not invent a
   kind), turn decode's equality into the admission check, and mirror the same
   in `frontend/packages/domain/format/src/control/objectName.ts`,
   `encode.ts`, `decode.ts`. Round-trip and rejection tests on both sides
   cover every row of the admission table, including the rejected pairings.
4. **`ObjectStore` port — the slot is bound to a name when it is reserved.**
   `reserve_create(name: &str) -> CommitSlot` and
   `put_if_absent(slot: &CommitSlot, body: ByteStream)`; the name parameter
   leaves `put_if_absent`. On S3 the slot is the name; on Drive it is the
   minted ID plus the name the create will pass. Spending one slot under two
   names then has no spelling. The head decides its successor's name
   (`head-<g+1>`) at reservation time, and CK-10's `snapshot_slot` is reserved
   the same way under `idx-<g>`. Add to the port a way to read back what a
   slot holds — `CommitSlot` to `ObjectRef`, or a `read_slot` — because a
   writer that lost or got an uncertain answer must fetch the object *at the
   slot* and compare it to its candidate by identity (CP-4, CP-5, CK-11);
   on Drive names are not unique, so a by-name lookup is wrong there.
   Document in the port that `reserve_create` is idempotent per name on a
   name-keyed store and mints a fresh identifier on Drive — exclusion comes
   from the one reservation the head carries, never from two writers
   reserving the same name independently. Update both adapters and the
   conformance suite (`backend/crates/domain/coffret-usecase/src/conformance/`)
   to the new shape; keep its race case as "one reservation handed to two
   writers, exactly one wins" and say in its comment that the store sees
   bytes, not kinds, so the kind-independence of the fix is proved at the
   usecase layer (next item), not here.
5. **The regression test for CP-3 lives in the usecase layer.** Against an
   in-memory `ObjectStore` (add a test-support implementation under
   `coffret-usecase` if none exists; the conformance suite's
   `store_under_test.rs` may already be most of one), derive from one head at
   generation `g` the successor slot for a Journal record and for an
   activation Snapshot — the two must come out as the same slot under the
   same name `head-<g+1>` — and race two creates against it: exactly one
   succeeds, the other gets `AlreadyExists`, and the loser's read-back
   returns the winner's object. This is the test that would have failed
   before this task and must be named for CP-3.
6. **CP-2 keeps its wording; spell out the slot's persisted form.** What a
   head stores in `next_commit_slot` / `snapshot_slot` is the adapter's
   opaque token only — the minted ID on Drive, nothing on S3 — and the name is
   re-derived from the generation and the role when the slot is spent. Add the
   spend-time check as a spec rule: a slot is spent only under the name
   `head-<g+1>` (commit) or `idx-<g>` (snapshot) for the head it came from;
   a mismatch is a refusal, not a write. Also add, as a CP rule, that a writer
   re-reads the head object immediately before spending its slot and aborts
   on NotFound — after a later epoch's MR-3 purge the key of a consumed slot
   is free again on a name-keyed store, and without the re-read an old-epoch
   writer that wakes late could commit into a purged position.
7. **KL-14 is rewritten to what the port can honour.** It currently says
   repair writes use "create-if-absent semantics per slot"; the port writes
   Keyring replicas with the unconditional `put`
   (`object_store.rs:40`), and on Drive there is no shared reservation two
   repairing devices could aim at. State instead that a replica at
   `(generation, set_digest, index)` has exactly one valid content — the
   mapping the digest binds (KL-1, KL-3) — so two devices repairing the same
   replica write identical bytes and the duplicate is benign; repair is an
   unconditional put, and a read-back after it verifies the replica rather
   than the write's exclusivity.
8. **S3 409.** `s3-store/src/error.rs` maps 409 `ConditionalRequestConflict`
   together with 412 to `AlreadyExists`. 409 means a concurrent conditional
   operation was in flight, not that the object exists; if that operation then
   failed the slot is still empty. This is harmless only because every loser
   reads the slot back (item 4) — say so in a comment at the mapping.
9. **Rename everywhere.** `jrn-` disappears from code, tests, fixtures,
   generated interop manifests, and the spec and concept documents (FM-12,
   the Journal and Storage Object concepts, and any rustdoc or TS comment that
   spells a name). The `docs/tasks/` history keeps its old spellings. Update
   FM-12's discovery sentence: recovery lists `head-*` for the newest head and
   `idx-*` for the newest ordinary checkpoint, and a `head-`-named Activation
   Snapshot is a checkpoint candidate alongside `idx-*` for CK-9 and RV-1.
   CK-10's "no ordinary Snapshot is written for an activation head" stays,
   with its reason restated: it avoids a multi-megabyte duplicate, not a name
   collision (the names no longer collide).

Documentation, comments, commit message, and PR description are in English.
The concept documents' conventions are in `docs/concepts/README.md` and the
spec register's in `docs/spec/README.md` (keep rule IDs stable; a new rule
takes the next free number in its mechanism, a changed rule keeps its ID).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] From one head at generation `g`, the usecase-layer derivation yields the
      same slot and the same name `head-<g+1>` for a Journal-record successor
      and for an activation-Snapshot successor; racing two creates against
      that slot on the in-memory store leaves exactly one winner, the loser
      sees `AlreadyExists`, and the loser's read-back of the slot returns the
      winner's object (the CP-3 test).
- [x] `ObjectStore::reserve_create` takes the name and
      `ObjectStore::put_if_absent` takes only a slot and a body; the port
      offers a read-back from a `CommitSlot` to the object it holds; both
      adapters implement the new shape and the MinIO conformance run passes
      (`make s3-store-it`).
- [x] The conformance race case hands one reservation to two writers and
      asserts exactly one winner; its comment says the store sees bytes, not
      kinds, and points at the usecase-layer test for kind-independence.
- [x] `ControlObjectKind` has an `ActivationSnapshot` variant with header byte
      `0x04`, `Purpose` derives it under `coffret/v1/control/activation-snapshot`,
      the KD-4 registry in `docs/spec/key-derivation/README.md` lists that
      info string (grep-gated in `check_command`), and the TypeScript format
      package mirrors kind and purpose with a golden vector for the new key.
- [x] Encoding and decoding follow the admission table: `head-` with Journal
      or Activation Snapshot, `idx-` with Index Snapshot, `key-` with Keyring
      round-trip in Rust and TypeScript; every other name/kind pairing is
      rejected in both, with a test per rejected pairing.
- [x] Parsing a `head-<n>` name does not yield a kind — `ControlObjectName`
      (and its TS twin) no longer has a total `kind()` for head names — and
      the encode path takes the kind explicitly.
- [x] The interop fixture set includes a `head-`-named Activation Snapshot and
      a `head-`-named Journal record, and `make interop` exchanges them in
      both directions.
- [x] No spelling of `jrn-<digits>` remains under `backend/`, `frontend/`,
      `docs/spec/`, or `docs/concepts/` (grep-gated in `check_command`; the
      task file and `docs/tasks/` history are excluded from the gate).
- [x] `docs/spec/format/README.md`: FM-11 lists kind `0x04`; FM-12 names the
      chain `head-<generation>.cfrt` and states the admission table; FM-13
      says `head-0` is a Journal record. `docs/spec/commit-protocol/README.md`
      gains the spend-time name check and the pre-spend head re-read as rules
      with new IDs. `docs/spec/keyring-lifecycle/README.md` KL-14 describes
      unconditional replica writes whose duplicates are benign. CK-10's
      restated reason and CK-9/RV-1's "checkpoint candidate" sentence are in
      place.
- [x] The Journal, Index Snapshot, and Storage Object concept documents
      mention the new naming and the activation kind only as far as the
      concept-doc litmus test allows (what a reader can rely on), citing the
      rule IDs; no concept document still spells `jrn-`.
- [x] The S3 error mapping carries the comment explaining why folding 409
      into `AlreadyExists` is safe.
- [x] No test compares an error value for equality or through `Debug` /
      `Display`; assertions match the variant and destructure fields.

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-store-it` (which needs a grant a person clicked through)
      runs the Drive-only case `a_purged_pre_minted_id_reports_how_a_second_create_is_answered`
      against a real account; it prints a `DRIVE FINDING:` line saying whether
      a purged pre-minted ID accepts a second create or is refused. Record that
      line in the PR description under "Drive behaviour after deleting a
      pre-minted ID" (it decides whether the pre-spend head re-read is
      belt-and-braces or the only guard on Drive).

## Out of scope

- The Journal commit, activation, and checkpoint operations themselves — this
  task changes the names, the port, the format's kind table, and the rules,
  and proves CP-3 at the usecase layer with a derivation test; the
  `Interactor` that performs commits is later work.
- Master Key rotation (MR) beyond the naming and kind it needs here.
- The late ordinary-Snapshot write after an activation (a writer of `head-g`
  writing `idx-g` after a later epoch purged it) — a pre-existing residual
  that needs the rotation implementation to close.
- Any change to Container naming (FM-3) or to Keyring replica naming.
