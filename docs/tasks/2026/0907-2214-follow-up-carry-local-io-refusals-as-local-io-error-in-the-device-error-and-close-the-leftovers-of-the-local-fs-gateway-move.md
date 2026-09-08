---
status: completed
pipeline_phase: null
plan: null
base_ref: null
follow_up_of: docs/tasks/2026/0908-0244-carry-local-io-refusals-as-local-io-error-in-the-device-error-and-close-the-leftovers-of-the-local-fs-gateway-move.md
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -B1 "^tempfile" backend/crates/libs/coffret-logging/Cargo.toml | grep -q "^#" && grep -q "or any caller keeping files of its own on the same disk" backend/crates/domain/coffret-usecase/src/local_io_error.rs && grep -q "or a composition root keeping files of its own on the same disk" backend/crates/domain/coffret-usecase/src/lib.rs'
assignee: null
branch: task/0907-2214-follow-up-carry-local-io-refusals-as-local-io-error-in-the-device-error-and-close-the-leftovers-of-the-local-fs-gateway-move
created_at: 2026-09-07T22:14:07Z
updated_at: 2026-09-08T00:56:44Z
---

# docs(backend): say who may build a LocalIoError and why the logging tests need a directory

## Overview

Three comments describe the arrangement before `coffret-device` started
building `LocalIoError` for files of its own: two of them say only a gateway
outside the use-case crate builds one, and a third `tempfile` dev-dependency
has no why-comment where every other one in the workspace does. Nothing in the
build or at runtime changes; each fix replaces or inserts comment text only.

- `backend/crates/libs/coffret-logging/Cargo.toml`, line 24,
  `tempfile = { workspace = true }` under `[dev-dependencies]`. The comment
  above it (`# The cases in tests/ read the file back, and the file is JSONL.`)
  belongs to the `serde_json` line, so this `tempfile` has no why-comment of
  its own. Insert these three lines directly above
  `tempfile = { workspace = true }`:

  ```
  # The directory the rotating sink writes its log files into: what the cases
  # are about is the files as they land — how many of them, how large, and what
  # is in them — so each one needs a real directory of its own.
  ```

- `backend/crates/domain/coffret-usecase/src/local_io_error.rs`, lines 12–16,
  the type doc's opening paragraph. It says the vocabulary exists "so that a
  gateway outside this crate can report a refusal in the same three parts every
  flow here already reads", which is now narrower than the truth: a caller
  outside this crate builds one for files of its own that no capability covers.
  Replace those five lines with:

  ```
  /// The vocabulary the capabilities over the local filesystem answer in —
  /// [`Spool`](crate::Spool) is the first of them — so that a gateway outside
  /// this crate, or any caller keeping files of its own on the same disk, can
  /// report a refusal in the same three parts every flow here already reads:
  /// what the run was doing, which file or directory it was doing it to, and
  /// what the operating system said.
  ```

- `backend/crates/domain/coffret-usecase/src/lib.rs`, lines 215–217, the
  comment above `mod local_io_error;`. It introduces the type "as a value a
  gateway outside this crate can build", the same narrow claim in a second
  place. Replace those three lines with:

  ```
  // What one operation on this device's own disk failed with, as a value any
  // caller outside this crate can build — a gateway behind a capability, or a
  // composition root keeping files of its own on the same disk: the vocabulary
  // every capability over the local filesystem answers in, and what
  // `LocalError::Io` carries.
  ```

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-logging`'s `tempfile` dev-dependency has a comment line directly
      above it
      (`grep -B1 "^tempfile" backend/crates/libs/coffret-logging/Cargo.toml | grep -q "^#"`).
- [x] `local_io_error.rs`'s type doc admits callers outside the crate that keep
      files of their own
      (`grep -q "or any caller keeping files of its own on the same disk" backend/crates/domain/coffret-usecase/src/local_io_error.rs`).
- [x] `lib.rs`'s comment above `mod local_io_error` says the same
      (`grep -q "or a composition root keeping files of its own on the same disk" backend/crates/domain/coffret-usecase/src/lib.rs`).
- [x] `make check` is green.
