---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && make s3-store-it"
assignee: null
branch: task/0825-0530-freeze-pack-segmentation
created_at: 2026-08-25T05:30:00Z
updated_at: 2026-08-25T08:32:00Z
---

# feat(backend): freeze — pack eligible files into path-ordered Packs

## Overview

Both halves of the round trip are on `main` (`coffret_usecase::sync` and
`coffret_usecase::fetch`), but every user-data Container they produce is
a one-file Container: a folder of thousands of images becomes thousands
of Storage objects and thousands of API round trips, which breaks both
the object-count band the pack policy exists to hold (PK spec preamble,
pack concept) and any practical fetch of a folder. This task builds
**`freeze`** in `coffret-usecase` — the one-shot pack-construction
operation the PK register specifies (PK-1..PK-8): select the eligible
local files in a folder, sort them by Entry Path, cut them into segments
around a size target, spool each segment as one **Pack**, upload, and
commit the batch (additions = the new Packs, removals = exactly the
absorbed one-file Containers).

Almost every piece it composes exists: `sync/`'s scan (EP-9/EP-10
selection, plaintext BLAKE3 for change detection), spool (streaming
ciphertext BLAKE3-256 + MD5 to disk, pending rows for OC-2 provenance),
upload (`ObjectStore::put` + `RetryPolicy` + provider-hash check), the
`commit/` flow (catch-up CK-9, Entry Path uniqueness, Keyring
pre-replication, conditional-create commit CP-15/16, settle), and
`fetch/` already decodes whatever the entry table lists, so Packs flow
back without changes. `coffret-model` has `ContainerKind::Pack`, and
`coffret-format`'s meta section carries the multi-entry table with
offsets (FM-9).

**The one real gap is streaming encode.** `coffret_format::encode()` is
whole-container-in-memory (`EncodeRequest` takes entry content slices;
`sync/spool.rs` reads the entire file first). That is fine for
image-sized one-file Containers and fatal for Packs: a normal Pack
targets ~GiB and an oversized singleton (PK-3) can exceed RAM. This task
adds a streaming encoder to `coffret-format` — the entry table's inputs
(path, mtime, size, plaintext BLAKE3) are all known before encoding
starts (the scan already hashes candidates), so the meta section can be
written first and entry bytes streamed through the chunked AEAD
afterwards. No wire-format change and no TypeScript change: the emitted
bytes must be identical to `encode()`'s for the same inputs.

### The flow (spec citations are the authority)

One public entry point — a directory-module interactor `freeze/`
(mirroring `sync/`), e.g. `freeze_folder(request)` — taking the two
ports, the keys, a `CommitPolicy`, a device clock, an Entry Path prefix
naming the folder to freeze (the Library root as the degenerate case),
and the **size target as an explicit policy parameter** (PK-5: a
parameter, not a format constant; its production default is chosen
later from measurements, so the request carries it and tests pass tiny
targets). Steps:

1. **Catch up and scan.** Bring the Index current (the `commit/` flow's
   catch-up, CP-4/CK-9) and walk the mappings intersected with the
   requested prefix, the same EP-9/EP-10 discipline as sync: only files
   this device can speak for.
2. **Select (PK-1, PK-13, PK-14).** Eligible: a local file not yet in
   the Library, and a local file whose current Entry is held by a
   one-file Container — including one whose content differs locally and
   one whose Container carries a key-lost marker (PK-13: freeze builds
   the replacement from the local bytes either way). Not eligible, and
   **surfaced with a reason, never silent** (PK-14): an Entry held by an
   existing Pack (with or without local modification — that is
   `update`/repack territory), and any update-eligible file the scan
   sees but this invocation cannot absorb. Existing Packs are never
   read, written, or listed as removals (PK-1, PK-2).
3. **Segment (PK-3, PK-4, PK-6).** Sort the selected Entries by Entry
   Path; append the next Entry while the resulting **pre-padding
   Container footprint** (entry contents + canonical metadata + framing)
   stays at or below the target, else close the current non-empty Pack.
   A single Entry over the target forms an oversized singleton Pack.
   No empty Pack. The footprint calculation belongs beside the encoder
   in `coffret-format` (it is framing knowledge), exposed so the policy
   layer never re-derives it. The resulting invariant to assert: no two
   adjacent normal Packs from one invocation can merge without
   exceeding the target (PK-4).
4. **Spool each Pack, streaming.** New Container ID and key per Pack,
   kind `Pack` (PK-15); stream every member file through the new
   encoder into the spool file, folding ciphertext BLAKE3-256 + MD5 as
   bytes hit disk (the `sync/spool.rs` pattern, including the pending
   row before upload — OC-2/OC-3 provenance). Whole-Pack (and
   whole-entry) buffering is a defect; memory stays bounded by the
   write-chunk size.
5. **Upload and commit (PK-7).** Upload with the existing retry and
   provider-hash discipline, then commit **one Journal batch**:
   additions = the new Packs (with their entry tables, CP-11), removals
   = exactly the absorbed one-file Containers — a newly imported file
   has no removal, an existing Pack never appears. The `commit/` flow
   supplies Keyring pre-replication, uniqueness checks, rebase, and
   settle (Index refresh, trash of the absorbed objects, checkpoint
   policy) unchanged.
6. **Outcome.** A `FreezeOutcome`: the Packs created (Container ID,
   entry count, oversized or normal), the absorbed one-file Containers,
   and every surfaced non-candidate with its reason. Nothing silent.

Interrupted runs follow sync's posture unchanged: pending rows name
what a dead run created; spools are not reused (the Container Key lived
only in memory); reconciliation is the existing machinery.

### Spec and concepts

No new rules: PK-1..PK-8, PK-13..PK-15 exist with *(Form: test)* and
this task is what makes them tested — cite rule IDs from the
conformance cases that enforce them. Do not renumber or reword rules.
Leave `docs/concepts/` untouched — concept-doc registration (the
`freeze` collocations among them) is tracked separately.

### Implementation notes

- **Where.** `coffret-usecase/src/freeze/` as a directory module split
  by step (the `sync/` layout is the house pattern); a `FreezeError`
  following `SyncError`'s precedent (cause-named variants, structured
  causes, no `PartialEq`, `source()` wired). Where freeze and sync need
  the same logic (scan, spool plumbing, upload), extract and share
  rather than duplicate — judge the split by the `commit/` precedent,
  and fix the smells you meet in the code you touch.
- **Streaming encoder.** In `coffret-format`, a writer-style
  counterpart to `encode()` (entry metadata upfront, entry bytes fed
  incrementally, ciphertext emitted incrementally). Property to pin
  with a test: for identical inputs, its output is **byte-identical**
  to `encode()`. Add a multi-entry (pack) interop fixture if the
  fixture set has none, so the TS side keeps proving the wire format —
  no TS code change is expected.
- **S3 interplay, noted not solved.** An oversized singleton can exceed
  S3's 5 GB single-PUT limit; that upload keeps failing with the
  existing typed rejection from #21 until the separate multipart task
  lands. Drive is unaffected. Do not special-case it here.
- **Logging.** Per `coffret-logging`'s crate doc: counts, Container
  IDs, object names, generations, byte totals — never Entry Paths,
  local paths, plaintext, or key material.
- **Conformance / E2E.** A `freeze_conformance` suite (house pattern,
  parameterized over the `ObjectStore` + `Index` pair, temp dirs as
  mapped folders), run in-memory under `make check` and against MinIO
  under `make s3-store-it`. Use tiny size targets to force boundaries
  cheaply. Cases:
  - **initial import** (PK-3/4/6/7/15): a folder of files freezes into
    path-ordered Packs — every normal Pack's pre-padding footprint ≤
    target, no two adjacent normal Packs mergeable, no empty Pack, no
    removals, every Container's kind decodes as `Pack`, and decoding
    round-trips every member's bytes;
  - **absorption** (PK-1/7): previously synced one-file Containers
    freeze into Packs; the batch's removals are exactly those
    Containers and they are trashed after commit;
  - **idempotence** (PK-2): an immediately repeated freeze selects
    nothing, uploads nothing, and existing Packs stay byte-for-byte
    untouched (counting-store assertion: no writes against them);
  - **oversized singleton** (PK-3): a file larger than the target forms
    a one-Entry Pack of its own;
  - **modified and key-lost one-file entries** (PK-13): both freeze to
    the current local bytes, the old Containers land in removals;
  - **surfacing** (PK-14): a Pack-held Entry with a local modification
    is surfaced and untouched; nothing in the outcome is silent;
  - **round trip**: a second device (fresh Index, own temp folder,
    same Master Key) fetches the frozen folder and every placed file's
    bytes equal the source — proving fetch reads Packs;
  - **streaming equality**: the streaming encoder's output equals
    `encode()`'s for a multi-entry request (in `coffret-format`'s own
    tests).

### Conventions

`CLAUDE.md` is authoritative: English docs/comments/commit/PR,
Conventional Commits, no `PartialEq` on error types, tests match
variants, `make check` (and `make s3-store-it` for the MinIO half).
Commit and PR text must be self-contained.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] One public freeze entry point exists in `coffret-usecase` and the
      in-memory `freeze_conformance` suite passes under `make check`;
      the MinIO run of the same suite passes under `make s3-store-it`,
      including the second-device round trip: a frozen folder is
      fetched by a fresh-Index device and every placed file's bytes
      equal the source.
- [x] Segmentation is enforced with the size target as a request
      parameter: Entries are packed in Entry Path order, every normal
      Pack's pre-padding footprint is at or below the target, no two
      adjacent normal Packs from one invocation can merge without
      exceeding it, an over-target file forms an oversized singleton
      Pack, no empty Pack is created, and every produced Container
      records kind `Pack` (PK-3, PK-4, PK-5, PK-6, PK-15).
- [x] The commit batch matches PK-7: additions are the new Packs,
      removals are exactly the absorbed one-file Containers (none for
      newly imported files), the absorbed objects are trashed after
      commit, and a repeated freeze selects nothing, uploads nothing,
      and leaves existing Packs byte-for-byte untouched (PK-1, PK-2 —
      counting-store assertion).
- [x] A locally modified one-file Entry and a key-lost one-file Entry
      both freeze to the current local bytes (PK-13); a Pack-held
      modified Entry is surfaced, untouched, and nothing is skipped
      silently (PK-14).
- [x] Pack encoding streams: `coffret-format` gains a streaming encoder
      whose output is byte-identical to `encode()` for the same
      multi-entry inputs (equality test), the freeze spool feeds entry
      bytes through it without whole-Pack or whole-entry buffering, and
      a multi-entry interop fixture exists so both implementations
      prove the Pack wire format.
- [x] Existing sync / commit / fetch / index conformance suites pass
      under `make check` and `make s3-store-it`; error types derive no
      `PartialEq`.

### Manual / on-hardware (verified by a human before merge)

- [x] The freeze module's rustdoc reads as the PK register's story
      (select → segment → spool → upload → commit) and the surfaced
      reasons in `FreezeOutcome` match what PK-14 demands (judgement
      about prose intent, not mechanically checkable).
- [x] Spooling a synthetic multi-GB file as an oversized singleton
      keeps memory flat (bounded by the write chunk), observed on real
      hardware.

## Out of scope

- `derive` / thumbnail Packs (separate task; the derived-entry
  namespace and FM-9 `derived_from` revision land there).
- `repack` and compaction (PK-8's cross-invocation regrouping).
- `update` and deletion propagation into Packs — the read-modify-
  replace machinery (PK-9..PK-12) for Entries already held by Packs.
- S3 multipart upload (the 5 GB rejection stays typed, #21).
- Choosing the production size-target default (a measurement question
  tracked as an observation, not a code question).
- Range/streaming fetch of Packs and the persistent download cache
  (the viewer-side tasks).
- Any CLI/UI surface for freeze; conformance drives it directly.
