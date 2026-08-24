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
branch: task/0824-0925-commit-flow
created_at: 2026-08-24T09:25:22Z
updated_at: 2026-08-24T11:01:26Z
---

# feat(backend): commit a batch through Keyring replication and the Journal record

## Overview

Every format and port the commit path needs now exists — the control
payloads (FM-15, FM-16, FM-17 with `encode_*` / `decode_*` /
`keyring_set_digest` in `coffret-format`), the `ObjectStore` port with
`reserve_create` / `put_if_absent` / `object_at`, `ControlHead` with both
slot reservations, the `Index` port with `restore` / `apply` / `refresh`
and the device-local tables, and `RetryPolicy` — but nothing composes
them. This task builds the **commit flow** in `coffret-usecase`: the
sequence that takes a prepared batch whose Container objects are already
on Storage and makes it the Library's next committed state, exercised
end-to-end against MinIO. It is the heart of the upload pipeline; the
scan / spool / upload stages that produce the prepared batch come next
and are out of scope here.

### The flow (spec citations are the authority)

One public entry point in `coffret-usecase` — a directory-module
interactor (name it for what it does, e.g. `commit/`) — that takes the
two ports (`&dyn ObjectStore`, `&dyn Index`), the purpose keys it needs,
a replica count, a checkpoint-policy threshold, and a prepared batch:
`additions` (each: the `ContainerAddition` the Journal record will carry
— id, kind, ciphertext hash and length, `object_ref`, entry table — plus
the Container's `KeyEnvelope`) and `removals` (Container IDs). Steps:

1. **Catch up (CK-9).** Bring the Index to the current head: list the
   control objects, find the newest valid checkpoint (`idx-*` and
   `head-*` Snapshots alike) and the newest head; adopt the checkpoint
   via `Index::restore` when it is newer than the Index, keep the Index
   otherwise; decode and `Index::apply` the Journal records after the
   starting point. Replay opens no Container (CP-11). This same routine
   is what a conflict rebase runs.
2. **Candidate checks.** The post-commit Entry set must satisfy the
   Entry Path uniqueness the EP register requires (two additions, or an
   addition and a kept current Entry, must not collide on a path unless
   the colliding current Container is in `removals`); refuse the batch
   otherwise, before writing anything.
3. **Keyring pre-replication (CP-8, CP-9, KL-14).** Compute the
   post-commit Container set `(current − removals) ∪ additions`, build
   the next-generation mapping over exactly that set (envelopes from the
   batch for additions, from the committed Keyring for kept Containers,
   key-lost markers carried over), encode it (FM-17), and upload all
   `replica_count` replicas under their FM-12 names with unconditional
   `put` (KL-14). Then read every replica back and verify it (KL-1: it
   decodes and authenticates, its header agrees with its name, its
   recomputed `set_digest` matches). One missing or invalid replica stops
   the flow before the commit (the design's rule: no commit until the
   candidate set is complete, KL-2).
4. **Commit (CP-2, CP-3, CP-15, CP-16).** Reserve the successor slot and
   the snapshot slot from the current head (`ControlHead`), re-read the
   head immediately before consuming (CP-16), encode the Journal record
   (FM-15: `prev`, both slots, the candidate Keyring tuple, additions
   with entry tables, removals) and `put_if_absent` it into the commit
   slot. Success is the batch's commit point (CP-1).
5. **Conflict (CP-4, CP-7).** A consumed slot is a normal outcome: rerun
   the catch-up (step 1) onto the new head, redo the candidate checks
   and the Keyring pre-replication against the new current set (a fresh
   generation — the previous candidate set stays uncommitted and is not
   reused), and retry. Cap the attempts; surface a typed error when the
   cap is hit. Conflicts are never resolved by timestamps (CP-7).
6. **After the commit.** `Index::refresh` with the committed batch;
   `trash` each removal's object (failure here is retryable and does not
   un-commit — the record is already the truth); then the checkpoint
   policy (CK-8): if the Journal committed since the newest checkpoint
   exceeds the threshold, encode the Index Snapshot (FM-16) from
   `Index::snapshot` and `put_if_absent` it into the `snapshot_slot` the
   record reserved (CK-10). Losing that race is benign: read back and
   accept the sibling for the same head (CK-11).

### Implementation notes

- **Where.** `coffret-usecase/src/commit/` as a directory module split by
  step, mirroring how `index_conformance/` and `conformance/` are laid
  out; shared values (`PreparedBatch`, per-step outcomes) follow
  one-type-per-module. Reuse `ControlHead`, `CommitSlot`,
  `CommittedBatch`, `RetryPolicy`; do not duplicate name grammar or
  digest logic — `coffret-format` owns those.
- **Errors.** Dedicated cause-named variants on `coffret-usecase`'s
  `Error` (or a commit-specific error if the module warrants it — say
  which and why): the candidate-check refusal carries the colliding
  path; the incomplete-Keyring failure carries which replica index was
  missing or invalid; the retry-cap failure carries the attempt count.
  No `PartialEq`; tests match variants.
- **Conformance suite.** A `commit_conformance` suite in
  `coffret-usecase` (the pattern `conformance/` and `index_conformance/`
  set), parameterized over an `ObjectStore` + `Index` pair, covering at
  least: the single-writer happy path (the stored record decodes to the
  committed batch, the Keyring tuple it names is complete and valid on
  Storage, the Index answers the new state, removals are trashed); a
  two-writer race where exactly one commit wins and the loser rebases
  and lands at the next generation with the right `prev`; a read-back
  with a missing replica stopping the flow with nothing written to the
  head; the checkpoint policy writing a Snapshot exactly when the
  threshold is crossed, and the snapshot-slot race converging (CK-11);
  an interrupted flow (Keyring uploaded, no record) leaving the head
  unchanged and the next run committing a fresh generation.
- **Where the suite runs.** In-memory (`InMemoryStore` + `InMemoryIndex`)
  in `coffret-usecase` unit tests — part of `make check`; against MinIO
  from `gateway/s3-store/tests/` (with `InMemoryIndex`) — part of
  `make s3-store-it`, following the existing conformance test's harness
  and env vars. No Google Drive test here (its authorization is manual).
- **Logging.** Follow `coffret-logging`'s crate doc for what may be
  logged: object names, generations, and counts are fine; no Entry
  Paths, no plaintext, no key material.

Documentation, comments, commit message, and PR description are in
English. No new spec rule IDs are expected; cite the existing ones.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] One public commit entry point exists in `coffret-usecase` and the
      in-memory `commit_conformance` suite passes under `make check`.
- [x] The MinIO run of the same suite passes under `make s3-store-it`,
      including the two-writer race: exactly one of two concurrent
      commits from the same head succeeds, the loser rebases (CK-9) and
      commits the next generation with `prev` = the winner's generation.
- [x] A batch is refused with a typed error, before any Storage write,
      when its post-commit Entry set would collide on an Entry Path.
- [x] The flow stops with a typed error and writes no Journal record
      when a Keyring replica is missing or invalid on read-back.
- [x] After a commit past the checkpoint threshold, the Index Snapshot
      at the record's `snapshot_slot` decodes and equals
      `Index::snapshot`; below the threshold no Snapshot is written; two
      writers racing on the same snapshot slot converge (CK-11).
- [x] Removals' objects are trashed after the commit, and an interrupted
      flow (Keyring uploaded, no record) leaves the head unchanged with
      the next run committing cleanly.
- [x] Dependency directions hold (`make deps`); error types derive no
      `PartialEq`; tests match variants; `make check` passes.

## Out of scope

- The scan / Pack-decision / encrypt-spool / upload stages that produce
  the prepared batch and put Container objects on Storage (the next
  task), and resumable-upload handling.
- Deletion propagation (the explicit-flag removals-only flow), `prune`,
  Master Key epoch activation, and orphan reclamation (the OC register).
- The Google Drive integration test (manual authorization), and any
  provider-limit / backoff tuning beyond using the existing
  `RetryPolicy`.
- Compression or format changes; the wire forms are fixed by FM-15..17.
