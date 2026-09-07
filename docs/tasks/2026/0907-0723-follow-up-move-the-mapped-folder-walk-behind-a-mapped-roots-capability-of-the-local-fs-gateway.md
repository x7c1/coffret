---
status: completed
pipeline_phase: null
plan: null
follow_up_of: docs/tasks/2026/0907-0006-move-the-mapped-folder-walk-behind-a-mapped-roots-capability-of-the-local-fs-gateway.md
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "mapped_roots_conformance" backend/crates/domain/coffret-usecase/Cargo.toml && grep -q "confinement target" backend/crates/domain/coffret-usecase/Cargo.toml && grep -q "The folder the freeze and fetch suites place into" backend/crates/gateway/s3-store/Cargo.toml && grep -q "as the flows that read and write it ask for it" backend/crates/apps/coffret-device/src/open_library/mod.rs && grep -q "the mapped folders to read" backend/crates/apps/coffret-device/src/open_library/mod.rs'
assignee: null
branch: task/0907-0723-follow-up-move-the-mapped-folder-walk-behind-a-mapped-roots-capability-of-the-local-fs-gateway
created_at: 2026-09-07T07:23:22Z
updated_at: 2026-09-07T08:28:55Z
---

# docs(backend): describe the mapped folders as read through the local-fs gateway in the crate comments

## Overview

The mapped-folder walk now reads the disk through the `MappedRoots`
capability, the sync and freeze conformance fixtures keep their mapped folder
in the in-memory fake, and the use-case crate carries a
`mapped_roots_conformance` suite. Five comments in files that change did not
touch still describe the previous arrangement. This task updates the text;
no behaviour changes.

- `backend/crates/domain/coffret-usecase/Cargo.toml`, lines 9–17 (the
  `[features]` comment block, anchored at `# The backend-agnostic contract suites`):
  replace those nine comment lines with:

  ```
  # The backend-agnostic contract suites — `conformance` for `ObjectStore`,
  # `index_conformance` for `Index`, `spool_conformance` for the `Spool` over
  # this device's own disk and `mapped_roots_conformance` for the `MappedRoots`
  # beside it, `commit_conformance` for the commit flow over `ObjectStore` and
  # `Index`, `sync_conformance` for the folder sync over those two plus the
  # mapped folders, `freeze_conformance` for packing those folders into Packs
  # instead, and `fetch_conformance` for the journey back — together with the
  # `InMemoryStore`, `InMemoryIndex`, and `InMemoryFs` they run against. Off by
  # default: only a gateway's test target needs them, and a shipping binary
  # should not carry the contract tests of the ports and capabilities it uses.
  ```

- `backend/crates/domain/coffret-usecase/Cargo.toml`, lines 63–66 (the
  `tempfile` dev-dependency comment, anchored at
  `# The folders a sync, a freeze, or a fetch case runs against`): replace
  those four comment lines with:

  ```
  # The real folder a fetch places into: the fetch suite's target, the freeze
  # suite's second device, and the confinement target's own tree. A sync's
  # mapped folder and every spool are in memory, so nothing else here needs a
  # directory. A suite does not make a case's directory itself — a backend hands
  # it over, so that a run against a real provider keeps it wherever it wants.
  ```

- `backend/crates/gateway/s3-store/Cargo.toml`, lines 27–28 (the `tempfile`
  dev-dependency comment, anchored at
  `# The folders the sync, freeze, and fetch suites run against`): replace
  those two comment lines with:

  ```
  # The folder the freeze and fetch suites place into: a fetch ends at a real
  # filesystem whichever provider the Library is on. The sync suite needs none —
  # its mapped folder and its spool are both in memory.
  ```

- `backend/crates/apps/coffret-device/src/open_library/mod.rs`, lines 4–8 (the
  module doc's second paragraph, anchored at
  `A sync, a freeze, and a fetch each take`): replace those five lines with:

  ```
  //! A sync, a freeze, and a fetch each take a store, a catalog, the keys of one
  //! Master Key epoch, and this device's own disk — the spool to write into, and
  //! for the two that scan, the mapped folders to read. None of them knows which
  //! provider the Library is on or whose filesystem it is reading and spooling
  //! onto, and none of them should: this module is the one place the settings
  //! file's answer — and the device's own disk — becomes a concrete gateway.
  ```

- `backend/crates/apps/coffret-device/src/open_library/mod.rs`, line 33
  (`/// This device's own disk, as the flows that write to it ask for it.`):
  replace that line with
  `    /// This device's own disk, as the flows that read and write it ask for it.`

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The `conformance` feature comment in `coffret-usecase/Cargo.toml` names `mapped_roots_conformance` (gate: `grep -q "mapped_roots_conformance" backend/crates/domain/coffret-usecase/Cargo.toml`).
- [x] The `tempfile` comment in `coffret-usecase/Cargo.toml` describes the one real folder that is left (gate: `grep -q "confinement target" backend/crates/domain/coffret-usecase/Cargo.toml`).
- [x] The `tempfile` comment in `s3-store/Cargo.toml` names the freeze and fetch suites as the ones that place into a real folder (gate: `grep -q "The folder the freeze and fetch suites place into" backend/crates/gateway/s3-store/Cargo.toml`).
- [x] The `local_fs` field doc in `open_library/mod.rs` says the flows read and write the disk (gate: `grep -q "as the flows that read and write it ask for it" backend/crates/apps/coffret-device/src/open_library/mod.rs`).
- [x] The module doc of `open_library/mod.rs` names the mapped folders a scan reads (gate: `grep -q "the mapped folders to read" backend/crates/apps/coffret-device/src/open_library/mod.rs`).
