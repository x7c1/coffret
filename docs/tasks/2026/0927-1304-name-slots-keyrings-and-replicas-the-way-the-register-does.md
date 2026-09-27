---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqw --exclude-dir=node_modules --exclude-dir=target --exclude-dir=dist --exclude-dir=dist-types "InvalidReplica" backend/crates/ frontend/packages/ && ! grep -rqw --exclude-dir=node_modules --exclude-dir=target --exclude-dir=dist --exclude-dir=dist-types "KeyringEntry" backend/crates/ frontend/packages/ && ! grep -rqE --exclude-dir=node_modules --exclude-dir=target --exclude-dir=dist --exclude-dir=dist-types "MalformedKeyringPayload|UnsupportedKeyringSchema" backend/crates/ frontend/packages/ && ! grep -rqiE --exclude-dir=node_modules --exclude-dir=target --exclude-dir=dist --exclude-dir=dist-types "spen(d|ds|t|ding)[^.]{0,40}slot|slot[^.]{0,30}spen(d|ds|t|ding)" backend/crates/ frontend/packages/ && ! grep -qw "pair" docs/concepts/keyring/README.md && grep -rqw "UnusableReplica" backend/crates/ && grep -rqw "KeyringElement" backend/crates/'
assignee: null
branch: task/0927-1304-name-slots-keyrings-and-replicas-the-way-the-register-does
created_at: 2026-09-27T13:04:34Z
updated_at: 2026-09-27T13:36:15Z
---

# refactor: name slots, Keyring elements and replicas the way the register does

## Overview

Four names in the code say something the spec and the concept documents do not. Each one is a rename; none changes behaviour, the wire, or any format byte.

1. **A commit slot is consumed, not spent.** The Journal concept (`docs/concepts/journal/README.md`, Collocations: "consume (a commit slot, by creating the successor it admits)") and the commit protocol (`docs/spec/commit-protocol/README.md`) say *consume*. The code says *spend* for the same act — doc comments and test names in `coffret-usecase` (`commit/journal.rs`, `commit/mod.rs`, `commit/run.rs`, `commit/catch_up.rs`, `commit_slot.rs`, `conformance/conditional_create.rs`, `control_head.rs`, `object_store.rs`, `lib.rs`), `coffret-model` (`index_checkpoint.rs`, `journal_record/mod.rs`) and the TypeScript format package (`frontend/packages/domain/format/src/model/indexCheckpoint.ts`). Replace the slot sense with *consume*. Other senses of *spend* are not this word and stay: a Passphrase spent at startup, a retry budget spent, time or memory spent.
2. **`InvalidReplica` becomes `UnusableReplica`.** KL-1 (`docs/spec/keyring-lifecycle/README.md`) makes *valid replica* a property of the content — it decrypts, authenticates and is consistent. The enum in `backend/crates/domain/coffret-usecase/src/commit/commit_error.rs` also carries `Absent` and `Unfetchable`, where nothing about the content is known, so "invalid" says more than the value knows. Rename it (and every use, re-export and test), and keep the doc's "definitively not one a mapping may be read from" on the variants that establish it (`Unreadable` and after), not on the type as a whole.
3. **`KeyringEntry` becomes `KeyringElement`.** FM-17 (`docs/spec/format/README.md`) calls the members of a Keyring mapping *elements*; the Keyring concept (`docs/concepts/keyring/README.md`) says *pair* in its Definition and *element-level* further down; the type is `KeyringEntry` (`backend/crates/domain/coffret-model/src/keyring_entry.rs`), which also collides with the Domain Model's **Entry** (a file inside a Container). Rename the type and its module to `KeyringElement` / `keyring_element.rs` across the backend and the TypeScript second implementation (`frontend/packages/domain/format/src/`), and make the Keyring concept say *element* where it says *pair*.
4. **The Keyring format errors use FM-17's noun.** The sibling errors in `backend/crates/domain/coffret-format/src/error/mod.rs` are named after their FM rule's noun — `MalformedJournalRecord` / `UnsupportedJournalRecordSchema` (FM-15), `MalformedIndexSnapshot` / `UnsupportedIndexSnapshotSchema` (FM-16). The Keyring pair is `MalformedKeyringPayload` / `UnsupportedKeyringSchema`, two nouns for one object. FM-17 names it "a Keyring replica's payload", so rename them `MalformedKeyringReplica` / `UnsupportedKeyringReplicaSchema`, with `display.rs`, `redacted.rs`, `control/keyring/decode.rs`, the rejection tests and the TypeScript counterparts following.

Where a rename reaches a test fixture, an interop vector, or a golden file, update it in the same change; where one of them encodes a name on the wire or in a stored format, stop and report instead — that would be a behaviour change this task does not make.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No `InvalidReplica` remains under `backend/crates/` or `frontend/packages/`, and `UnusableReplica` exists (grep gates)
- [x] No `KeyringEntry` remains under `backend/crates/` or `frontend/packages/`, and `KeyringElement` exists (grep gates)
- [x] No `MalformedKeyringPayload` or `UnsupportedKeyringSchema` remains (grep gate)
- [x] No line under `backend/crates/` or `frontend/packages/` pairs *spend* with a slot (grep gate)
- [x] The Keyring concept no longer says *pair* (grep gate)
- [x] `make check` passes, including the interop suite between the Rust and TypeScript implementations

### Manual / on-hardware (verified by a human before merge)

- [x] No slot-sense *spend* survives across a line break, and every remaining *spend* in the backend is one of the other senses named in the Overview (checked mechanically with a multi-line search over the tracked `.rs` / `.ts` / `.tsx` files: no *spend* within a sentence of *slot*; the two near *head* are about bytes and round trips)
