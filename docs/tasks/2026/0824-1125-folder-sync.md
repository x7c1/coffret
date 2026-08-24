---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && make s3-store-it"
assignee: null
branch: task/0824-1125-folder-sync
created_at: 2026-08-24T11:25:58Z
updated_at: 2026-08-24T13:30:31Z
---

# feat(backend): sync a folder — scan, spool, upload, and commit

## Overview

The commit flow (`coffret_usecase::commit`) turns a prepared batch into the
committed Library state, but nothing yet produces a prepared batch from a
folder on disk. This task builds the **folder sync** in `coffret-usecase`:
scan the device's mapped folders against the Index, encrypt what changed
into spool files, upload the Containers, and hand the batch to the existing
commit flow — the first end-to-end upload path, exercised against MinIO.
The pieces it composes all exist: the `Index` device-local tables
(`mappings`, `mark_present` / `mark_absent` / `local_entry_at` /
`present_under` / `pending_uploads`), `coffret_format::encode`
(`EncodeRequest` / `EntrySource` / `EncodedContainer`),
`generate_container_id` / `generate_container_key` /
`wrap_container_key`, `ObjectStore::put` + `RetryPolicy`, `ProviderHash`,
and `commit::commit_batch` with `PreparedBatch`.

### The flow (spec citations are the authority)

One public entry point — a directory-module interactor (e.g. `sync/`) —
taking the two ports, the keys, a `CommitPolicy`, a spool directory, and a
device clock. Steps:

1. **Scan (EP-9, EP-10).** For each of the device's `mappings`, walk the
   local folder and translate file paths to Entry Paths. Compare each file
   against the Index: a path with no current Entry is **new**; a path whose
   (size, mtime) differs from the device's own local record is a
   **candidate change**, confirmed by hashing the plaintext (BLAKE3) and
   comparing with the current Entry's `hash` — equal content is a no-op
   that just refreshes the local observation. A row this device itself
   materialized whose file is gone is **deleted locally**; an Entry the
   device never materialized is never reported as changed or deleted,
   whether or not a mapping covers it (EP-10).
2. **What this sync handles.** New files become one-file Containers.
   A changed file whose current Entry lives in a **one-file** Container
   becomes a replacement Container (new Container ID, the old one in
   `removals` — CP-14, PK-10). A changed file whose Entry lives in a
   **Pack**, and locally deleted files, are **surfaced in the sync's
   outcome but not acted on**: read-modify-replace over a Pack is the
   Pack half of `update` (PK-10..PK-12), deferred to the Pack task, and
   deletion propagation is an explicit-flag flow
   this task does not add. Surfacing is mandatory — a sync must never
   silently skip a file needing update (PK-14).
3. **Spool (design: upload pipeline).** For each new/replacement
   Container: generate the Container ID and Key, encode the Container
   (`EncodeRequest`, one entry, FM-1..9), write the ciphertext to a spool
   file under the spool directory, computing its BLAKE3-256 (the
   `ciphertext_hash` FM-15 carries) and its MD5 (for the provider
   comparison) while writing; wrap the key (`wrap_container_key`, FM-14);
   record a `PendingUpload` in the Index (`record_pending_upload`) before
   uploading, so an interrupted run can resume from the spool.
4. **Upload.** `ObjectStore::put` each spool file under the Container's
   object name, through `RetryPolicy` — the caller opens a fresh
   `ByteStream` from the spool file for every attempt (the documented
   contract). After a completed upload, compare the provider-reported
   hash (`ObjectRef` / `ProviderHash`) with the spool's MD5 where the
   provider reports one, and treat a mismatch as transfer corruption (a
   typed error, the object not trusted). Update the `PendingUpload` with
   the returned `object_ref`.
5. **Commit.** Assemble the `PreparedBatch` (additions with entry tables,
   ciphertext hashes and lengths, `object_ref`s, envelopes; removals =
   replaced one-file Containers) and call `commit::commit_batch`. On
   success: `mark_present` each uploaded Entry (it is now materialized —
   this device placed it), `clear_pending_upload` each Container, delete
   the spool files, and report the commit outcome plus the surfaced-but-
   unhandled findings (Pack-resident changes, local deletions).
6. **Resume.** A sync run first reconciles `pending_uploads` left by an
   interrupted run. A spool is never adopted into a new batch: the
   Container Key existed only in the interrupted run's memory, and the
   envelope that would have recorded it was bound for a Keyring whose
   commit never happened, so the spooled ciphertext is unopenable going
   forward. Reconciliation therefore deletes the spool, trashes an
   uploaded object that the caught-up Index confirms no committed state
   names (the creating device's own pending row is the local provenance
   the OC register requires for reclaiming), clears the row, and lets
   this run's scan re-spool the file under a fresh Container ID and Key —
   converging to exactly one committed Entry without orphaning spool
   files or Storage objects silently.

### Implementation notes

- **Where.** `coffret-usecase/src/sync/` as a directory module split by
  step (the `commit/` layout is the house pattern); values
  (`SyncOutcome`, per-file findings, etc.) one type per module. Scan and
  spool touch the local filesystem directly (device-side work, not a
  port); use tokio's fs. A `SyncError` following `CommitError`'s
  precedent (cause-named variants, structured causes, no `PartialEq`,
  `source()` wired; wrap `CommitError` for the commit step rather than
  flattening it).
- **Hashing.** Plaintext BLAKE3 for change detection (EP register /
  design); ciphertext BLAKE3-256 for FM-15; MD5 only for the provider
  transfer check. Compute spool hashes streamingly while writing — do
  not read the spool back just to hash it.
- **MIME.** Leave `EntrySource::mime` as `None`; detection is not this
  task.
- **Conformance / E2E.** A `sync_conformance` suite (house pattern),
  parameterized over the `ObjectStore` + `Index` pair, using a temp
  directory as the mapped folder: first sync of a folder (files become
  one-file Containers, committed, decodable — fetch a Container back,
  unwrap its envelope via the committed Keyring, decode, and compare
  plaintext bytes); an unchanged second sync commits nothing; a modified
  file produces a replacement + removal; an mtime-only touch with equal
  content commits nothing; a Pack-resident change and a local deletion
  are surfaced and untouched; an interrupted run (spool + pending row,
  no upload / uploaded but uncommitted) resumes to a single committed
  Entry; a provider-hash mismatch is a typed error. Run in-memory under
  `make check` and against MinIO from `gateway/s3-store/tests/` under
  `make s3-store-it` (reuse the `tests/minio/mod.rs` harness).
- **Logging.** Per `coffret-logging`'s crate doc: counts, Container IDs,
  object names, generations — never Entry Paths, local paths, plaintext,
  or key material.

Documentation, comments, commit message, and PR description are in
English. No new spec rule IDs are expected; cite the existing ones.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] One public sync entry point exists in `coffret-usecase` and the
      in-memory `sync_conformance` suite passes under `make check`.
- [x] The MinIO run of the same suite passes under `make s3-store-it`,
      including the full round trip: a synced file's Container is fetched
      back from MinIO, opened through the committed Keyring's envelope,
      and its decoded bytes equal the source file.
- [x] An unchanged second sync commits nothing; an mtime-only touch with
      equal content commits nothing and refreshes the local observation.
- [x] A modified file whose Entry lives in a one-file Container commits a
      replacement Container with the old one removed (CP-14); a
      Pack-resident change and a locally deleted file are surfaced in the
      outcome and not acted on.
- [x] An interrupted run resumes: a spool + pending row without an
      upload, and an uploaded-but-uncommitted Container, each converge to
      one committed Entry with the spool cleaned up; a stale pending row
      is dropped with its spool deleted.
- [x] A provider hash mismatch after upload is a dedicated typed error
      and the object is not committed.
- [x] Dependency directions hold (`make deps`); error types derive no
      `PartialEq`; tests match variants; `make check` passes.

## Out of scope

- Pack construction (`freeze`, PK-1..8) and the Pack half of `update`
  (read-modify-replace, PK-10..12) — the next task; this sync only
  surfaces Pack-resident changes.
- Deletion propagation (explicit-flag removals-only flow), `prune`,
  epoch activation, orphan reclamation (OC).
- Resumable-upload sessions and S3 multipart (single-request `put` via
  `RetryPolicy` is this task's transport), MIME detection, thumbnails.
- The download / viewing path.
