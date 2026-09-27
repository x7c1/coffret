---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqiE --exclude-dir=target "pending_?upload" backend/crates/ scripts/ && grep -rqw --exclude-dir=target "PendingRow" backend/crates/ && grep -rqF --exclude-dir=target "pending_rows" backend/crates/gateway/coffret-sqlite-index/src/ && grep -qF "pub(crate) const SCHEMA_VERSION: i64 = 7;" backend/crates/gateway/coffret-sqlite-index/src/schema.rs && grep -qF "pub(crate) const DEVICE_SCHEMA_VERSION: i64 = 7;" backend/crates/gateway/coffret-sqlite-index/src/schema.rs'
assignee: null
branch: task/0927-1421-call-the-unfinished-container-record-a-pending-row
created_at: 2026-09-27T14:21:43Z
updated_at: 2026-09-27T14:33:44Z
---

# refactor(index): call the device-local record of an unfinished Container a pending row

## Overview

The Index concept (`docs/concepts/index/README.md`, "A **pending row** is the device-local record of a Container this device is about to spool, has spooled, or has uploaded before any commit") names this record a *pending row*. The code calls it an upload: the type `PendingUpload` (`backend/crates/domain/coffret-usecase/src/device_state/pending_upload.rs`), the `Index` port's `record_pending_upload` / `clear_pending_upload` / `pending_uploads` (`coffret-usecase/src/index.rs`), and the SQLite table `pending_uploads` (`backend/crates/gateway/coffret-sqlite-index/src/`). For most of its life the row names a Container that has not been uploaded — a spool being written, or a whole spool not yet sent — so the name says something the row does not.

1. Rename the type to `PendingRow` (module `pending_row.rs`), the port methods to `record_pending_row` / `clear_pending_row` / `pending_rows`, and every implementation and caller: the SQLite adapter, the in-memory index, the conformance suites and their fault-injecting indexes (`refusing_index.rs`, `rival_index.rs`, `truncating_index.rs`, `watching_index.rs`), and the tests under `coffret-usecase/tests/`. Local variables and doc comments that call the row an upload follow. Words that describe a real upload — `SpoolState`'s object handle, "uploaded before any commit" — stay.
2. Rename the SQLite table to `pending_rows`. This is a change to a device-local table, so `schema.rs`'s rule applies as written: `SCHEMA_VERSION` and `DEVICE_SCHEMA_VERSION` both move to 7, and a file stamped 6 or lower is refused whole, with `RefusedIndex` still reading `prefix` and `local_root` for the recovery it offers. No in-place migration is added: coffret has no deployed Libraries whose device state must survive, and the refusal path is the one the layout rule already prescribes. Update the doc on `DEVICE_SCHEMA_VERSION` ("In this build the two are equal, because …") to name this change as the reason the window is empty, and follow the version constants in the tests (`coffret-sqlite-index/tests/`, `coffret-device/src/mapping/tests.rs` `PREVIOUS_SCHEMA_VERSION`) and in `scripts/drive-index-layout-it.sh` where it names the table or the versions.

Guard: the Index concept's words *pending row* and *local provenance* are the vocabulary this aligns to; the concept document itself does not change.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No `pending_upload`, `pendingupload` or `PendingUpload` remains under `backend/crates/` or `scripts/` (case-insensitive grep gate), and `PendingRow` exists (grep gate)
- [x] The SQLite adapter names the table `pending_rows` (grep gate)
- [x] `SCHEMA_VERSION` and `DEVICE_SCHEMA_VERSION` are both 7 (grep gates), and the schema tests that open a file stamped with the previous version still see it refused with its mappings readable through `RefusedIndex` (`make check`)
- [x] `make check` passes
