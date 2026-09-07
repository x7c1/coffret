---
status: completed
pipeline_phase: null
plan: null
follow_up_of: docs/tasks/2026/0906-1928-move-the-spools-filesystem-writes-behind-a-spool-capability-in-a-local-fs-gateway.md
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "two empty catalogs, and three empty directories" backend/crates/domain/coffret-usecase/src/freeze_conformance/mod.rs && ! grep -q "empty catalogs, and three empty directories" backend/crates/domain/coffret-usecase/src/fetch_conformance/mod.rs && grep -q "spool_conformance" backend/crates/domain/coffret-usecase/Cargo.toml && ! grep -q "The folder and the spool directory the sync suite" backend/crates/gateway/s3-store/Cargo.toml && ! grep -q "The folders and the spool directory a sync or a fetch case" backend/crates/domain/coffret-usecase/Cargo.toml'
assignee: null
branch: task/0906-2326-follow-up-move-the-spools-filesystem-writes-behind-a-spool-capability-in-a-local-fs-gateway
created_at: 2026-09-06T23:26:49Z
updated_at: 2026-09-07T00:02:00Z
---

# docs(backend): bring the conformance fixture comments in line with the spool moving to the in-memory fake

## Overview

The sync, freeze and fetch conformance fixtures no longer take a spool
directory — each builds an `InMemoryFs` for the spool and takes only the real
folders a case reads — and the use-case crate gained a `spool_conformance`
suite and an `InMemoryFs` fake. Five comments in files that change did not
touch still describe the previous arrangement. This task updates the text;
no behaviour changes.

- `backend/crates/domain/coffret-usecase/src/freeze_conformance/mod.rs`, line 104
  (the macro doc line `/// two empty catalogs, and three empty directories to run the case against, or`):
  replace with `/// two empty catalogs, and two empty folders to run the case against, or`.
- `backend/crates/domain/coffret-usecase/src/fetch_conformance/mod.rs`, line 105
  (the macro doc line `/// empty catalogs, and three empty directories to run the case against, or \`None\``):
  replace with `/// empty catalogs, and two empty folders to run the case against, or \`None\``.
- `backend/crates/domain/coffret-usecase/Cargo.toml`, lines 9–16 (the comment
  block above `conformance = []`, anchored at `# The backend-agnostic contract suites`):
  replace the whole eight-line block with:

  ```
  # The backend-agnostic contract suites — `conformance` for `ObjectStore`,
  # `index_conformance` for `Index`, `spool_conformance` for the `Spool` over
  # this device's own disk, `commit_conformance` for the commit flow over the two
  # ports, `sync_conformance` for the folder sync over them plus a local folder,
  # `freeze_conformance` for packing that folder into Packs instead, and
  # `fetch_conformance` for the journey back — together with the `InMemoryStore`,
  # `InMemoryIndex`, and `InMemoryFs` they run against. Off by default: only a
  # gateway's test target needs them, and a shipping binary should not carry the
  # contract tests of the ports and capabilities it uses.
  ```

- `backend/crates/gateway/s3-store/Cargo.toml`, lines 27–28 (the two comment
  lines above `tempfile = { workspace = true }`, anchored at
  `# The folder and the spool directory the sync suite runs against`):
  replace with:

  ```
  # The folder the sync suite runs against: a sync starts at a real filesystem
  # whichever provider the Library is on.
  ```

- `backend/crates/domain/coffret-usecase/Cargo.toml`, lines 62–65 (the four
  comment lines above `tempfile = { workspace = true }`, anchored at
  `# The folders and the spool directory a sync or a fetch case runs against`):
  replace with:

  ```
  # The folders a sync or a fetch case runs against, and the folder the crate's
  # own unit tests walk. A suite does not make a case's directories itself — a
  # backend hands them over, so that a run against a real provider keeps them
  # wherever it wants.
  ```

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `freeze_conformance/mod.rs` no longer says "two empty catalogs, and three empty directories" (gate: `! grep -q "two empty catalogs, and three empty directories" backend/crates/domain/coffret-usecase/src/freeze_conformance/mod.rs`).
- [x] `fetch_conformance/mod.rs` no longer says "empty catalogs, and three empty directories" (gate: `! grep -q "empty catalogs, and three empty directories" backend/crates/domain/coffret-usecase/src/fetch_conformance/mod.rs`).
- [x] The `conformance` feature comment in `coffret-usecase/Cargo.toml` names `spool_conformance` (gate: `grep -q "spool_conformance" backend/crates/domain/coffret-usecase/Cargo.toml`).
- [x] The `tempfile` comment in `s3-store/Cargo.toml` no longer mentions the spool directory (gate: `! grep -q "The folder and the spool directory the sync suite" backend/crates/gateway/s3-store/Cargo.toml`).
- [x] The `tempfile` comment in `coffret-usecase/Cargo.toml` no longer mentions the spool directory (gate: `! grep -q "The folders and the spool directory a sync or a fetch case" backend/crates/domain/coffret-usecase/Cargo.toml`).
