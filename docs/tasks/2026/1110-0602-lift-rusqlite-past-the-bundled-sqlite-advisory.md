---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make deny'
assignee: null
branch: task/1110-0602-lift-rusqlite-past-the-bundled-sqlite-advisory
created_at: 2026-11-10T06:02:23Z
updated_at: 2026-11-10T07:54:54Z
---

# fix(backend): move rusqlite to 0.41 to leave RUSTSEC-2026-0459 behind

## Overview

`cargo-deny` now fails on every pull request (and would on `main`) with
RUSTSEC-2026-0459: the SQLite that `libsqlite3-sys` 0.30.1 bundles can return
uninitialized memory when it reads a crafted `-wal` file. `libsqlite3-sys`
comes in through `rusqlite = { version = "0.32", features = ["bundled"] }` in
`backend/Cargo.toml`, used by `coffret-sqlite-index`, `coffret-device` and
`coffret-measure`.

Fix it by moving the workspace's `rusqlite` to 0.41 (keeping `bundled`), so
that the bundled SQLite is one the advisory does not cover. Do not add the
advisory to `ignore` in `backend/deny.toml`: the fix is the upgrade.

Adapt the three crates to whatever API changed between 0.32 and 0.41 (for
example, changes to `params!`, `ToSql` / `FromSql`, `Connection::open_with_flags`,
`OptionalExtension`, or error variants), keeping their behaviour as it is: the
Index schema, the queries and the on-disk layout stay unchanged, so a Library's
Index written by the current release opens and reads the same after the
upgrade. Where an adaptation is not purely mechanical, say why in the PR
body. Update `Cargo.lock` with the upgrade, and nothing else beyond what the
upgrade pulls in.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `backend/Cargo.toml` asks for `rusqlite` 0.41 with `bundled`, and
      `Cargo.lock` resolves a `libsqlite3-sys` that RUSTSEC-2026-0459 does
      not cover.
- [x] `make deny` passes with no new entry in `backend/deny.toml`'s
      `ignore` list.
- [x] `make check` passes, including the existing tests of
      `coffret-sqlite-index`, `coffret-device` and `coffret-measure`, with no
      test skipped or removed.
