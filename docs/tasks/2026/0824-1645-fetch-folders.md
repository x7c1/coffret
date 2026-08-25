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
branch: task/0824-1645-fetch-folders
created_at: 2026-08-24T16:45:39Z
updated_at: 2026-08-24T19:35:00Z
---

# feat(backend): fetch the Library into the mapped folders — download, verify, and place

## Overview

The upload half of the round trip is on `main` (`coffret_usecase::sync`,
scan → spool → upload → commit). Nothing yet goes the other way as a
production flow: the only code that fetches a Container back, opens the
committed Keyring, unwraps an envelope, and decodes is test-side
(`sync_conformance/library.rs::open`). This task builds the **fetch**
flow in `coffret-usecase` — the mirror of `sync_folders` — that
materializes the Library's current Entries into this device's mapped
folders: catch the Index up, read the committed Keyring, fetch each
needed Container, verify, and place files. It is the other half of the
round trip and the data path the viewer will later sit on.

Every piece it composes exists: `commit::catch_up` (`pub(crate)`, reads
the head and brings the Index current per CK-9, and its `ControlListing`
already holds each Container's Storage handle), the `Index` queries
(`mappings`, `entries_under`, `containers_under`, `local_entry_at`,
`mark_present`) and `ContainerSummary` (FM-15 `ciphertext_hash`,
`ciphertext_len`, and the `object_ref` cache), `ObjectStore::get` +
`RetryPolicy`, and `coffret-format`'s
`decode_control_object` / `decode_keyring` (FM-17) /
`unwrap_container_key` / `decode` (which authenticates every chunk
before any byte reaches the caller).

### The flow (spec citations are the authority)

One public entry point — a directory-module interactor `fetch/`
(mirroring `sync/`), e.g. `fetch_folders(request)` — taking the two
ports, the keys, a `CommitPolicy` (for its `RetryPolicy`), a device
clock, and an optional Entry Path prefix that narrows the run to one
subtree. "Fetch" is the verb EP-10 already uses for this act ("uploaded
it, or fetched it into place"). Steps:

1. **Catch up (CK-9, RV-1).** Bring the Index to the current head via
   `commit::catch_up` — on a fresh device that is restore-from-newest-
   checkpoint plus replay, so a second enrolled device with an empty
   Index can fetch. A fetch that cannot read the head fails rather than
   serving a stale catalog.
2. **Select (EP-9, EP-10).** For each mapping (intersected with the
   optional prefix), walk `entries_under` and decide per Entry against
   the device's local record:
   - no `local_entry_at` row and no file at the translated local path →
     **fetch**;
   - row in state present and the disk file matches the recorded
     observation → **skip** (already materialized);
   - no row but a file exists at the path → **conflict, surfaced and
     untouched** — the file is not this device's materialization and is
     never overwritten (it may be an unsynced source file);
   - row in state present but the disk file differs from the recorded
     observation → **surfaced and untouched** — that is a pending local
     change the sync flow owns;
   - row in state absent (a deletion this device witnessed, EP-10) →
     **surfaced and not re-fetched** — resurrecting a deleted file is an
     explicit operation this task does not add (mirror of the
     deletion-propagation flag on the sync side).
   Nothing is ever skipped silently (the PK-14 discipline): every
   non-fetched Entry lands in the outcome under its reason.
3. **Open the committed Keyring (KL-1, KL-3, KL-6, KL-7, RV-2, RV-3).**
   The caught-up checkpoint's `KeyringCommitment` names generation,
   `set_digest`, and replica count; read one replica by its
   `ControlObjectName`, authenticated under the Keyring purpose key, and
   validate it against the commitment (KL-1). A replica that fails to
   open or validate falls back to the next position — a degraded set
   still serves a fetch (RV-2); all replicas failing is Keyring loss
   territory and a typed error (RV-7). A Container mapped to
   `key_lost` is **locked**: reported in the outcome, its Entries not
   fetched, the rest of the run unaffected (KL-7, RV-2).
4. **Fetch each Container once (FM-15, PK-16 posture).** Group the
   selected Entries by Container (`EntryLocation::container_id`); fetch
   the whole Container object exactly once per run via
   `ObjectStore::get` under `RetryPolicy` (a retry re-opens a fresh
   stream — same contract as upload). Resolve the handle from the
   `ContainerSummary::object_ref` cache when present, else from the
   catch-up's `ControlListing`. Before decrypting, check the
   ciphertext's BLAKE3-256 against the record's `ciphertext_hash`
   (FM-15) — a mismatch is transfer/substitution corruption, a typed
   error, nothing placed.
5. **Decode and verify (FM-1..9, KD-2, FM-14).** Unwrap the Container
   Key from the Keyring envelope (`unwrap_container_key` against the
   Container's own ID) and `decode` the fetched bytes — authentication
   is per chunk inside `decode` and nothing unauthenticated escapes it.
   Then compare each wanted Entry's plaintext BLAKE3 against the
   current Entry's `hash` in the Index (the entry table the record
   carried, CP-11): authenticity proves the bytes are a coffret object,
   the hash comparison proves they are the *committed content this
   catalog names*. A mismatch is a typed error and places nothing.
6. **Place (EP-4, EP-10).** Write each verified Entry to a temporary
   file in the destination directory (same filesystem), set the file's
   mtime to the Entry's `mtime`, and rename it into the translated
   local path — a reader never sees a partial or unverified file.
   Create parent directories as needed. Then `mark_present` with the
   Entry's size and mtime (`at` = now): the fetch is exactly the second
   way a device materializes an Entry (EP-10), so the next scan sees a
   clean match and the file is in the sync flow's scope from here on.
7. **Outcome.** A `FetchOutcome` reporting: fetched Entries, skipped
   (already present), conflicts (foreign file / locally modified),
   not-refetched witnessed deletions, locked Containers (key_lost), and
   the Containers fetched. Nothing silent.

### Spec

Add one rule to `docs/spec/entry-path/README.md` — **EP-11**, placement:
a fetch materializes an Entry only into a path whose local state the
device can vouch for — no existing file, or this device's own
materialization record matching the file on disk. Anything else is
surfaced as a conflict and never overwritten, and a fetched file becomes
visible at its final path only after full verification (authentication
plus content-hash match), via temp-write-and-rename. Cite EP-10 (the
materialization record) and EP-4's no-silent-selection posture.
*(Form: test)*. Do not renumber existing rules. Leave `docs/concepts/`
untouched — concept-doc registration (the `fetch` collocation among
them) is tracked separately.

### Implementation notes

- **Where.** `coffret-usecase/src/fetch/` as a directory module split by
  step (the `sync/` layout is the house pattern); one public type per
  module; a `FetchError` following `SyncError`'s precedent (cause-named
  variants, structured causes, no `PartialEq`, `source()` wired; wrap
  `CommitError` for the catch-up the way `sync` does).
- **Keys.** Mirror `sync_keys.rs`: the fetch needs the `ControlKeys`
  (catch-up + Keyring purpose key) and the Container-wrap purpose key.
  Reuse or share rather than duplicate derivation.
- **Buffered transport is this task's scope.** Whole-Container `get`
  and in-memory `decode` — this flow's subjects are image-sized
  files. Streaming chunk-to-disk decode, HTTP Range resume of an
  interrupted fetch, range-read prefetch inside Packs, and a persistent
  download cache are the viewer/Pack work and explicitly out of scope.
- **Keyring reading location.** The replica-read-and-validate (KL-1
  against a commitment, with positional fallback) is production logic
  this task introduces; put it where both fetch and future flows can
  reach it (e.g. a `keyring`-reading module under `coffret-usecase`,
  not inside `fetch/`'s privates, if the split is natural — judge by
  the `commit/` precedent).
- **Logging.** Per `coffret-logging`'s crate doc: counts, Container
  IDs, object names, generations — never Entry Paths, local paths,
  plaintext, or key material.
- **Conformance / E2E.** A `fetch_conformance` suite (house pattern,
  parameterized over the `ObjectStore` + `Index` pair, temp dirs as
  mapped folders), run in-memory under `make check` and against MinIO
  under `make s3-store-it` (reuse `tests/minio/mod.rs`):
  - a folder synced by one device is fetched by a **second device**
    (fresh Index, own temp folder, same Master Key): catch-up restores
    the catalog, every file's bytes equal the source, mtimes match the
    Entries, `local_entry_at` answers present, and a repeated fetch
    skips everything and fetches no Container again (assert via the
    counting-store pattern);
  - a prefix-narrowed fetch touches only that subtree's Containers;
  - a foreign file at a target path is surfaced and left byte-identical;
    a locally modified materialized file likewise;
  - a witnessed deletion (row in state absent) is surfaced and not
    re-fetched;
  - a mangled Container object (mangling-store pattern) is a typed
    error and no file appears at the target path (temp cleaned up);
  - a Container whose ciphertext hash does not match the record
    (substitution) is a typed error, nothing placed;
  - a `key_lost` Container is reported locked while the rest of the
    batch is fetched and placed;
  - a mangled first Keyring replica falls back to a later one and the
    fetch succeeds (RV-2).

### Conventions

`CLAUDE.md` is authoritative: English docs/comments/commit/PR,
Conventional Commits, no `PartialEq` on error types, tests match
variants, `make check` (and `make s3-store-it` for the MinIO half).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] One public fetch entry point exists in `coffret-usecase` and the
      in-memory `fetch_conformance` suite passes under `make check`; the
      MinIO run of the same suite passes under `make s3-store-it`,
      including the second-device round trip: a folder synced by one
      device is fetched by a fresh-Index device and every placed file's
      bytes equal the source.
- [x] Placement is verified-and-atomic: a mangled Container and a
      ciphertext-hash mismatch are dedicated typed errors and no file
      (partial or whole) appears at any target path.
- [x] Placement marks materialization: after a fetch, `local_entry_at`
      answers present with the Entry's size and mtime, the placed file's
      mtime equals the Entry's, and a repeated fetch skips every Entry
      and fetches no Container (counting-store assertion).
- [x] Conflicts are surfaced, never clobbered: a foreign file at a
      target path and a locally modified materialized file are reported
      in the outcome and left byte-identical; a witnessed deletion is
      reported and not re-fetched.
- [x] A `key_lost` Container is reported locked while every other
      selected Entry is fetched; a mangled first Keyring replica falls
      back to a later replica (RV-2) and the run succeeds.
- [x] `docs/spec/entry-path/README.md` gains EP-11 codifying placement,
      with existing rule IDs unchanged, and the rule is cited from the
      conformance cases that enforce it.
- [x] Existing sync / commit / index conformance suites pass under
      `make check` and `make s3-store-it`; error types derive no
      `PartialEq`.

### Manual / on-hardware (verified by a human before merge)

- [x] The fetch module's rustdoc reads as the mirror of `sync`'s story
      (select → open Keyring → fetch → verify → place) and EP-11's
      wording matches what the code enforces (judgement about prose
      intent, not mechanically checkable).

## Out of scope

- Streaming chunk-to-disk decode, HTTP Range resume, range-read
  prefetch inside Packs (PK-16 reads), and the persistent download
  cache — the viewer/Pack tasks.
- Restoring a witnessed-deleted file (an explicit flag/flow, like
  deletion propagation on the sync side).
- The viewer connection itself, thumbnails, MIME detection.
- Keyring repair (KL-11/KL-13) — a degraded set is read through, not
  repaired here.
- `freeze` / Pack construction, S3 multipart, orphan reclamation.
