---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && test -d backend/crates/gateway/coffret-sqlite-index && ! grep -rn 'NOCASE' backend/crates/gateway/coffret-sqlite-index/src"
assignee: null
branch: task/0823-1745-local-index-port-and-sqlite-adapter
created_at: 2026-08-23T08:27:41Z
updated_at: 2026-08-23T10:24:00Z
---

# feat(backend): add the local Index port and its SQLite adapter

## Overview

The Index is the device-local catalog of the Library: which Entry Path lives
in which Container at which offset, which Containers are current, and — beside
that catalog — the state only this device knows (how it maps the Library onto
local folders, which Entries it has actually placed on disk, what it has
spooled but not yet committed). Nothing of it exists yet: `coffret-usecase`
has the `ObjectStore` port and `ControlHead`, but no port for the Index, and
there is no adapter. The transfer flow (scan → encrypt → upload → commit) and
the viewer both sit on this.

This task adds the port and one adapter, with the domain types they need, and
proves the port's contract with a backend-agnostic conformance suite in the
same style as `coffret-usecase::conformance` for `ObjectStore`. It does **not**
add the wire encoding of the Index Snapshot or the Journal record (their CBOR
payloads in `coffret-format`) — the port takes and returns domain values, and
the payload encoding is the next task.

### Where things go

- **Port** `coffret_usecase::Index` (name per the Index concept document; one
  trait, its own module, async like `ObjectStore`), plus the domain values it
  speaks: put into `coffret-model` what is vocabulary (a current Container's
  summary — id, kind, ciphertext hash and length, optional provider object
  reference; an Entry's location — path, container, offset, size, mtime,
  content hash, optional mime and `derived_from` — reusing `ContainerId`,
  `ContainerKind`, `ContentHash`, `EntryPath`, `EntryMetadata`, `Mtime`,
  `DerivedFrom`, `Generation`, `MasterKeyEpoch` where they already exist; the
  checkpoint tuple — epoch, head generation, last applied Journal generation,
  next commit slot, Keyring generation/replica count/set digest), and into
  `coffret-usecase` what is operation shape (the port itself, a `JournalRecord`
  value and a `SnapshotContent` value as the inputs/outputs of replay and
  checkpointing, the device-local states). `coffret-model` stays free of
  third-party dependencies (`make deps` enforces it).
- **Adapter** `backend/crates/gateway/coffret-sqlite-index` (our own layer, so
  the `coffret-` prefix plus the technology), on `rusqlite` with the `bundled`
  feature, one database file per Library at a path the caller passes (the
  default location under the XDG state directory is the composition root's
  business, not the adapter's).
- **Conformance suite** `coffret_usecase::index_conformance` (feature-gated
  like the existing one), run by the SQLite adapter's integration test and by
  an in-memory implementation in `coffret-usecase` used for the suite's own
  tests.

### Tables — two groups, separated by table, never by column

Library-wide (what a Snapshot carries; CK-7):

- `checkpoint` — one row: `master_key_epoch`, `head_generation`,
  `journal_generation`, `next_commit_slot`, `keyring_generation`,
  `keyring_replica_count`, `keyring_set_digest`, `adopted_snapshot` (the
  checkpoint object's name this Index last adopted, or NULL).
- `containers` — `id` PK, `kind`, `ciphertext_hash`, `ciphertext_len`,
  `object_ref` NULL.
- `entries` — `path` PK, `container_id` FK, `offset`, `size`, `mtime`, `hash`,
  `mime` NULL, `derived_from_container` / `derived_from_path` NULL. An index on
  `container_id`. Paths are stored in canonical form (EP-1 to EP-3) and
  compared bytewise: `BINARY` collation, never `NOCASE` (grep-gated).
  `UNIQUE(path)` is a defence; uniqueness is the commit's job (EP-5, EP-6).

Device-local (never in a Snapshot; EP-9, EP-10):

- `mappings` — `prefix` (NULL = Library root), `local_root`.
- `local_entries` — `path` PK, `state` (`absent` / `present`),
  `observed_size`, `observed_mtime`, `observed_at`. `present` means this device
  uploaded or fetched the file into place; only `present` rows can ever be
  reported as deleted locally (EP-10). Rows may outlive their `entries` row.
- `pending_uploads` — `container_id` PK, `spool_path`, `batch`, `created_at`,
  `object_ref` NULL: encrypted spools and Containers uploaded before commit,
  the provenance orphan cleanup needs (OC-2).

### Operations (the port's surface)

Named after the Index concept's collocations; each runs in one transaction.

- `restore(snapshot: SnapshotContent)` — replace the three Library-wide
  tables wholesale with the Snapshot's content; leave device-local tables
  untouched (CK-9 adoption, RV-1 restore).
- `apply(record: JournalRecord)` — remove the record's removals (Containers and
  their Entries), insert its additions (Containers with kind, and the entry
  tables the record carries, CP-11), advance `checkpoint`. Opens no
  Container — the record carries everything.
- `refresh(batch)` — after this device's own successful commit (CP-1): insert
  additions, remove removals, advance `checkpoint` to the new head, mark the
  uploaded files `present` in `local_entries`, drop them from
  `pending_uploads`. Never called before the commit succeeded.
- `snapshot() -> SnapshotContent` — the three Library-wide tables in canonical
  order (containers by id, entries by path bytes), for the caller to encode
  and upload when the checkpoint policy asks (CK-8).
- Queries the scan and the viewer need: entries under a path prefix (range
  over the primary key), the current Entry at a path, the `present` rows under
  a mapping, the set of Containers holding any Entry under a prefix (distinct;
  PK-8 overlap), an Entry's (container, offset, size). Plus the device-local
  writers: set a mapping, mark a path `present` / `absent`, record and clear a
  pending upload, list pending uploads.

Invariants the suite checks: the Library-wide tables are a pure function of
the control state applied (restore then apply yields the same tables as a
restore of the later Snapshot would); `refresh` of a batch equals `apply` of
the record that batch committed; device-local tables survive `restore`;
`snapshot()` after `restore(s)` equals `s`; a `present` row whose path is not
in `entries` is reported as such and not deleted by `restore`; paths with
different case or NFC form are distinct rows.

### Not in this task

- Encoding/decoding the Snapshot and Journal payloads to CBOR (`coffret-format`
  + TS + interop) — the next task. Here `SnapshotContent` / `JournalRecord` are
  domain values.
- The scan itself, the transfer `Interactor`, the checkpoint policy's
  decision, and the viewer — callers of this port, later tasks.
- Derived/search metadata tables (EXIF etc.).
- Migration of an existing database file: there is none; the adapter creates
  the schema and refuses a file whose schema version it does not know.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret_usecase::Index` exists with `restore`, `apply`, `refresh`,
      `snapshot`, the listed queries and device-local writers; the SQLite
      adapter `coffret-sqlite-index` and an in-memory implementation in
      `coffret-usecase` both pass the new `index_conformance` suite.
- [x] `restore(s)` followed by `snapshot()` yields content equal to `s`
      (canonical order); `restore(s0)` then `apply(r1..rn)` yields the same
      Library-wide tables as `restore(sn)` where `sn` is the Snapshot of the
      head those records reach.
- [x] `refresh` of a batch and `apply` of the record committing that batch
      leave identical Library-wide tables; `refresh` additionally marks the
      batch's files `present` and clears their `pending_uploads` rows.
- [x] Device-local tables (`mappings`, `local_entries`, `pending_uploads`)
      are unchanged by `restore` and `apply`; a `local_entries` row whose path
      left `entries` remains and is reported by the "present but gone from the
      Library" query.
- [x] Two Entry Paths differing only in case, or only in Unicode
      normalization form, are distinct rows; `NOCASE` appears nowhere in the
      adapter (grep-gated).
- [x] The adapter creates its schema in a fresh file, reopens an existing one,
      and refuses a file with an unknown schema version with a typed error.
- [x] `coffret-model` gains no third-party dependency (`make deps`); error
      types derive no `PartialEq`; tests match variants.
- [x] `make check` passes.

## Out of scope

As listed under "Not in this task". Additionally: the default database
location and file permissions (0600) are the composition root's concern, not
the adapter's.
