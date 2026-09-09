---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0908-1938-keep-private-locations-out-of-diagnostic-events.md
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "EL-1: a path read back out of a record" backend/crates/domain/coffret-model/src/error.rs && grep -q "^        /// (spec: EL-1)\.$" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs && grep -q "EL-1, EP-9: the message is written" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs && grep -q "their file (spec: EL-1)" backend/crates/apps/coffret-server/src/sync/run.rs && grep -q "EL-1: the path is what identifies the conflict" backend/crates/domain/coffret-usecase/src/index_error.rs && grep -q "own files (spec: EL-1)" backend/crates/apps/coffret-server/src/routes/upload/mod.rs && grep -q "their file (spec: EL-1)" backend/crates/apps/coffret-server/src/refresh/run.rs && grep -q "name for it (spec: EL-1)" backend/crates/apps/coffret-server/src/freeze/run.rs && grep -q "name for it (spec: EL-1)" backend/crates/apps/coffret-server/src/fill/run.rs && grep -q "own files (spec: EL-1)" backend/crates/domain/coffret-usecase/src/catch_up/run.rs && grep -q "own files (spec: EL-1)" backend/crates/domain/coffret-usecase/src/sync/sync_error.rs && grep -q "carrying (spec: EL-1)" backend/crates/apps/coffret-server/src/routes/upload/outran.rs && grep -q "(spec: EL-1)" backend/crates/apps/coffret-server/src/routes/upload/room_for.rs && grep -q "so neither names a file or a location" backend/crates/gateway/google-drive-store/src/upload.rs && ! grep -q ''an object name says nothing about'' backend/crates/gateway/google-drive-store/src/api/failed_response.rs'
assignee: null
branch: task/0909-1249-follow-up-keep-private-locations-out-of-diagnostic-events
created_at: 2026-09-09T12:49:50Z
updated_at: "2026-09-09T13:19:23Z"
---

# docs(backend): cite EL-1 for log privacy and stop calling recorded names opaque

## Overview

The event-logging spec (`docs/spec/event-logging/`) now owns the rule that a
diagnostic event carries no Entry Path, local path, or filename (EL-1), and it
states that what licenses recording an object name is that coffret or the
provider minted it, not that the name is opaque (EL-5). A number of comments
outside the change that introduced those rules still cite `EP-1` (Entry Path
normalization) as the authority for log privacy, and two comments in the Drive
gateway still justify what is recorded by opacity. Bring them in line. No
behaviour changes: only comment and doc-comment text is edited.

`EP-1` → `EL-1` citations (each is a comment or doc comment; genuine `EP-1`
citations about path normalization or shape in the same files stay as they
are):

- `backend/crates/domain/coffret-model/src/error.rs`, line 444, anchor
  `// EP-1: a path read back out of a record is the user's own name for their`:
  replace `// EP-1: a path read back out of a record` with
  `// EL-1: a path read back out of a record`.
- `backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`, line 119,
  the line `        /// (spec: EP-1).` closing the doc comment on
  `UnmaterializablePath::component`: replace that line's `/// (spec: EP-1).`
  with `/// (spec: EL-1).`.
- `backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`, line 487,
  anchor `    // EP-1, EP-9: the message is written for whoever is keeping the Library and`:
  replace `// EP-1, EP-9: the message is written` with
  `// EL-1, EP-9: the message is written`.
- `backend/crates/apps/coffret-server/src/sync/run.rs`, line 64, anchor
  `/// their file (spec: EP-1) — how many there were is enough to read a run's`:
  replace `their file (spec: EP-1)` with `their file (spec: EL-1)`.
- `backend/crates/domain/coffret-usecase/src/index_error.rs`, line 295, anchor
  `    // EP-1: the path is what identifies the conflict to whoever is keeping the`:
  replace `// EP-1: the path is what identifies the conflict` with
  `// EL-1: the path is what identifies the conflict`.
- `backend/crates/apps/coffret-server/src/routes/upload/mod.rs`, line 262,
  anchor `    // user's own names for their own files (spec: EP-1).`: replace
  `own files (spec: EP-1)` with `own files (spec: EL-1)`.
- `backend/crates/apps/coffret-server/src/refresh/run.rs`, line 30, anchor
  `    // the user's own name for their file (spec: EP-1).`: replace
  `their file (spec: EP-1).` with `their file (spec: EL-1).`.
- `backend/crates/apps/coffret-server/src/freeze/run.rs`, line 74, anchor
  `/// user's own name for it (spec: EP-1): what is recorded of it is how long it`:
  replace `name for it (spec: EP-1)` with `name for it (spec: EL-1)`.
- `backend/crates/apps/coffret-server/src/fill/run.rs`, line 156, anchor
  `/// user's own name for it (spec: EP-1): what is recorded of it is how long it`:
  replace `name for it (spec: EP-1)` with `name for it (spec: EL-1)`.
- `backend/crates/domain/coffret-usecase/src/catch_up/run.rs`, line 66, anchor
  `/// names for their own files (spec: EP-1), and how many there now are is enough`:
  replace `own files (spec: EP-1)` with `own files (spec: EL-1)`.
- `backend/crates/domain/coffret-usecase/src/sync/sync_error.rs`, line 180,
  anchor `    /// the user's own names for their own files (spec: EP-1). What is left is`:
  replace `own files (spec: EP-1)` with `own files (spec: EL-1)`.
- `backend/crates/apps/coffret-server/src/routes/upload/outran.rs`, line 13,
  anchor `/// carrying (spec: EP-1).`: replace `carrying (spec: EP-1).` with
  `carrying (spec: EL-1).`.
- `backend/crates/apps/coffret-server/src/routes/upload/room_for.rs`, line 26,
  the line `/// (spec: EP-1).` that closes the paragraph ending
  `Neither is anybody's name for anything`: replace that line with
  `/// (spec: EL-1).`.

Drive gateway comments that justify recording by opacity:

- `backend/crates/gateway/google-drive-store/src/upload.rs`, lines 42-45, the
  comment above `info!(operation, object = name, bytes, "stored an object");`,
  anchor `The name is opaque and the size is of ciphertext,`: replace the two
  comment lines

  ```
      // they expected to go up. The name is opaque and the size is of ciphertext,
      // so neither says anything about what was stored.
  ```

  with

  ```
      // they expected to go up. The name is one coffret minted and the size is
      // of ciphertext, so neither names a file or a location.
  ```

  Control-object names reach this upload path (a Keyring replica name, for
  instance), and FM-12 reserves "opaque" for a Container's name while a control
  object's name is recognizable. What licenses recording the name is that
  coffret minted it (EL-5); the S3 gateway's comment already reads this way.

- `backend/crates/gateway/google-drive-store/src/api/failed_response.rs`,
  lines 152-153, anchor
  `// recorded is opaque or Drive's own: an object name says nothing about`:
  replace the two lines

  ```
        // recorded is opaque or Drive's own: an object name says nothing about
        // the Library, and the body has had any credential taken out of it.
  ```

  with

  ```
        // recorded is opaque or Drive's own, and the body has had any
        // credential taken out of it. Opacity alone is not what makes it
        // safe — a provider may echo any part of a request (spec: EL-5).
  ```

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-model/src/error.rs` cites EL-1: `grep -q "EL-1: a path read back out of a record" backend/crates/domain/coffret-model/src/error.rs`
- [x] `fetch_error.rs` closes the `component` doc with EL-1: `grep -q "^        /// (spec: EL-1)\.$" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`
- [x] `fetch_error.rs` message comment cites EL-1: `grep -q "EL-1, EP-9: the message is written" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`
- [x] `coffret-server/src/sync/run.rs` cites EL-1: `grep -q "their file (spec: EL-1)" backend/crates/apps/coffret-server/src/sync/run.rs`
- [x] `index_error.rs` cites EL-1: `grep -q "EL-1: the path is what identifies the conflict" backend/crates/domain/coffret-usecase/src/index_error.rs`
- [x] `routes/upload/mod.rs` cites EL-1: `grep -q "own files (spec: EL-1)" backend/crates/apps/coffret-server/src/routes/upload/mod.rs`
- [x] `refresh/run.rs` cites EL-1: `grep -q "their file (spec: EL-1)" backend/crates/apps/coffret-server/src/refresh/run.rs`
- [x] `freeze/run.rs` cites EL-1: `grep -q "name for it (spec: EL-1)" backend/crates/apps/coffret-server/src/freeze/run.rs`
- [x] `fill/run.rs` cites EL-1: `grep -q "name for it (spec: EL-1)" backend/crates/apps/coffret-server/src/fill/run.rs`
- [x] `catch_up/run.rs` cites EL-1: `grep -q "own files (spec: EL-1)" backend/crates/domain/coffret-usecase/src/catch_up/run.rs`
- [x] `sync_error.rs` cites EL-1: `grep -q "own files (spec: EL-1)" backend/crates/domain/coffret-usecase/src/sync/sync_error.rs`
- [x] `routes/upload/outran.rs` cites EL-1: `grep -q "carrying (spec: EL-1)" backend/crates/apps/coffret-server/src/routes/upload/outran.rs`
- [x] `routes/upload/room_for.rs` cites EL-1: `grep -q "(spec: EL-1)" backend/crates/apps/coffret-server/src/routes/upload/room_for.rs`
- [x] `google-drive-store/src/upload.rs` no longer justifies the name by opacity: `grep -q "so neither names a file or a location" backend/crates/gateway/google-drive-store/src/upload.rs`
- [x] `google-drive-store/src/api/failed_response.rs` no longer claims an object name says nothing: `! grep -q 'an object name says nothing about' backend/crates/gateway/google-drive-store/src/api/failed_response.rs`
