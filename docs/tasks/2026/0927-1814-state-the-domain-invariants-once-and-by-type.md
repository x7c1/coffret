---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && B=backend/crates && ! grep -rqF "format!(\"{byte:02x}\")" $B/ && ! grep -qF "0 => return None" $B/apps/coffret-cli/src/report.rs && ! grep -rqE "fn digests\b" $B/domain/coffret-usecase/src/upload/ && ! grep -qE "^\s*pub(\(crate\))? object_ref:" $B/domain/coffret-usecase/src/device_state/pending_row.rs && grep -qE "pub use .*ControlObjectKind" $B/apps/coffret-device/src/lib.rs && grep -qE "pub use .*SourceChange" $B/apps/coffret-device/src/lib.rs && grep -qE "pub use .*ControlObjectFault" $B/apps/coffret-device/src/lib.rs'
assignee: null
branch: task/0927-1814-state-the-domain-invariants-once-and-by-type
created_at: 2026-09-27T18:14:00Z
updated_at: 2026-09-27T20:21:08Z
---

# refactor: state the domain's invariants once, and by type where they can be

## Overview

Several rules in the domain and usecase crates hold only because every writer happens to keep them, or are written out more than once. A prose invariant is kept by type where a type can carry it. A computation or a cleanup that several sites repeat is done by one helper. An error value the layer below reported travels as the value, not as its text. One rule covers the whole change: a rule is stated once, and by type where it can be. Nothing a person sees and nothing on the wire changes. No device-local format changes either: the SQLite layout stays at its current `SCHEMA_VERSION` / `DEVICE_SCHEMA_VERSION`.

Paths: `U` = `backend/crates/domain/coffret-usecase/src/`, `F` = `backend/crates/domain/coffret-format/src/`, `D` = `backend/crates/apps/coffret-device/src/`.

1. **A pending row that is still spooling has no object.**
   - `PendingRow` (`U/device_state/pending_row.rs`) carries `state: SpoolState` and `object_ref: Option<ObjectRef>` as independent fields. The rule tying them ("a `Spooling` row has no object") is prose. Settle's `completes` (`U/sync/settle.rs`) is safe only because it ANDs `state == Spooled` with membership.
   - Make it one value: `Spooling`, or `Spooled` carrying `Option<ObjectRef>` (`None` until the upload, `Some` after). Keep the concept's two states: `docs/concepts/index/README.md` tables Spooling and Spooled with an object-handle column, and says the only transition is Spooling → Spooled. Adding a third state would change that vocabulary.
   - Every writer and reader follows:
     - `U/freeze/spool.rs`, `U/sync/spool.rs`, and `U/upload/run.rs`, which records the uploaded row as a full upsert and also rewrites `created_at`. Keep the recorded time the row was created with.
     - The in-memory index `U/in_memory_index/state.rs`.
     - The SQLite adapter `backend/crates/gateway/coffret-sqlite-index/src/`: the reader in `rows/device.rs`, the writer and `mark_spooled` in `device_state.rs`.
     - The fixtures and wrappers in the conformance suites: `index_conformance/fixtures.rs`, `sync_conformance/interruption.rs`'s `plant_row`, `freeze_conformance/truncating_index.rs`, and `coffret-sqlite-index/tests/schema.rs`.
   - SQLite keeps its stored form, the `"spooling"` / `"spooled"` text plus the `object_ref` column. The reader rejects a `spooling` row that carries an object as an Index error, the way it already rejects unknown text. There is no schema bump and no CHECK constraint, since a constraint would be a layout change.
   - Add an `index_conformance/device_state.rs` case: a `spooling` row with an object is refused.
2. **A Keyring repair rewrote at least one replica.**
   - `KeyringRepair::rewritten` (`U/commit/keyring_repair.rs`) is a `Vec<u16>` whose non-emptiness is prose. The only construction (`U/commit/keyring.rs`) guards it with `is_empty()`, and the CLI defends against the impossible zero with `0 => return None` in `repair_sentence` (`backend/crates/apps/coffret-cli/src/report.rs`).
   - Give the field a type that cannot be empty, local to the crate, and drop the CLI's zero arm. The CLI's return type becomes plain.
   - `CommitError::UnrepairedKeyring { rewritten }` can legitimately be empty and keeps its `Vec`.
3. **An answer's length is checked in one place.**
   - The overrun / mismatch / short checks on a Storage answer (`Error::LengthOverrun` / `LengthMismatch`) are written three times: `ByteStream::collect_exact` (`U/byte_stream.rs`), `U/fetch/container.rs`, and `U/fetch/range_read.rs`.
   - The streaming sites check mid-loop, before a decoder sees an extra byte, against the declared length and either the recorded length or the requested range.
   - Put the counting in one helper beside `ByteStream`, for example a counting reader or tracker that yields those two errors, and use it at all three.
4. **"What the other mappings represent" is built once.**
   - The set of prefixes other mappings represent is built four times: `U/local_scan/walk_mappings.rs`, `U/fetch/translate.rs`, `U/sync/scan/deletions.rs` (whose comment says it is "the same set, built the same way"), and `D/browse/list.rs` (`Reach`).
   - Build it once, next to `Mapping` (`U/device_state/mapping.rs`), over an iterator of mappings, and use it at every site. If one site's input type makes that wrong, say why in its doc.
5. **A fetch's scratch is discarded by one guard.**
   - `write_entry` (`U/fetch/range_read.rs`) calls `discard_all` on nine failure paths, and `decode_into_place` (`U/fetch/container.rs`) calls `decoding.discard()` on six.
   - Replace them with a guard that discards on drop unless it is disarmed at success. `DegradedReport` in `U/commit/keyring.rs` is the precedent. Both functions' two error channels (`Err` and `Ok(Err)`) leave the guard armed.
   - Keep `discard_all`'s log-don't-raise behaviour.
   - `tests/place_faults.rs` and the fetch conformance suite must pass unchanged.
6. **Lowercase hex goes through `coffret_model::lowercase_hex`.**
   - `F/control/keyring/set_digest.rs`'s `to_lowercase_hex` and the hand-written `format!("{byte:02x}")` loops encode by hand. They are at `U/fetch/fetch_error.rs` (`hex`), `U/in_memory_store.rs`, `U/scratch.rs`, `U/spool_file.rs`, `D/batch_id.rs`, `D/server_key.rs`, `backend/crates/gateway/google-drive-store/src/upload_digest.rs` and `backend/crates/apps/coffret-server/src/server_id.rs`.
   - Use `lowercase_hex::encode` at each; md5 outputs need `.into()` to `[u8; 16]`. `coffret-server` depends on `coffret-device` and nothing below it (its `Cargo.toml` says so), so reach the encoder through a `coffret-device` re-export rather than a new dependency.
   - The output spelling is identical everywhere: batch ids, scratch names, the server-key file and FM-12 set digests stay byte for byte.
7. **A CBOR decode failure carries ciborium's error as its cause.**
   - `F/malformed_cbor.rs` and `F/control/cbor/mod.rs` (`read_body`, `deserialization_failed`) fold the decoder's error into the `detail: String` of `MalformedMeta` / `MalformedControlPayload` / `MalformedJournalRecord` / `MalformedIndexSnapshot` / `MalformedKeyringReplica` (`F/error/mod.rs`).
   - Keep `detail` for the field-level messages this crate writes itself. Give the decode-failure path a cause that holds the decoder's value. `Error` is `Clone`, so share the value, for example through an `Arc`.
   - Follow the crate's own policy (`F/error/mod.rs` doc): a dependency's error travels as `cause`, never rendered into `Display`, and `redacted()` keeps saying only what it says now, because ciborium's text may quote payload content.
   - The TypeScript mirror (`frontend/packages/domain/format/src/errors.ts`) compares codes only and needs nothing.
8. **The shell can name every payload type an error carries.**
   - `D/lib.rs` re-exports the error types but not three payload types their variants carry: `ControlObjectKind`, `SourceChange` and `ControlObjectFault`. Its own doc says "a shell branching on any of them has to be able to name it".
   - Re-export them, and check the other field types those variants carry (`ControlObjectName`, `Generation`, …) against the same rule.
9. **An upload is verified from its own answer, not by listing the bucket.**
   - `verify` / `digests` in `U/upload/run.rs` list the whole bucket, up to `MAX_PAGES`, after every batch to compare provider digests. It is O(N) in the Library, and on Drive it keys the listing by name while names are not unique. Its premise, "the digest is not part of what a write answers with", no longer holds:
     - the Drive adapter already reads `md5Checksum` from the upload answer and refuses a mismatch (`google-drive-store/src/upload.rs`);
     - S3 always sends a single `PutObject`, whose answer carries the ETag (`s3-store`).
   - Have `ObjectStore::put` answer with the provider's digest where it has one, and verify each object against its own answer. Keep the mismatch outcome as it is: an object that did not arrive whole is `TransferCorrupted`, and an answer that names no digest where the provider always sends one is refused as the adapter already refuses it.
   - Every `ObjectStore` implementation follows: the in-memory store, both gateways, the test and conformance wrappers in `coffret-usecase`, and the ones in `coffret-server` tests. `sync_conformance/mangling_store.rs`, which injects a wrong digest through `list`, moves to `put`.
   - `ListingLimitReached` stays, because other listing walks raise it.

Guard: no person-visible sentence, no wire field and no device-local layout changes. Spec texts FM-12, FM-15, CP-11, OC-7 and KL-15 stay as they are. The object handle keeps its concept-document name.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `PendingRow` no longer carries a free-standing `object_ref` field (grep gate), and the index conformance suite refuses a spooling row with an object (`make check`)
- [x] The CLI's `repair_sentence` has no zero arm (grep gate)
- [x] No hand-written `format!("{byte:02x}")` hex remains under `backend/crates/` (grep gate)
- [x] `upload/` no longer lists the bucket to verify (`digests` is gone; grep gate), and the sync conformance suite still catches a corrupted transfer (`make check`; the freeze suite has no corrupted-transfer case, before or after this change)
- [x] `coffret-device` re-exports `ControlObjectKind`, `SourceChange` and `ControlObjectFault` (grep gates)
- [x] `make check` passes, including `tests/place_faults.rs`, the fetch conformance suite and the interop job's codes
