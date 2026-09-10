---
status: completed
pipeline_phase: null
plan: null
base_ref: null
follow_up_of: docs/tasks/2026/0910-1112-say-diagnostic-event-where-a-comment-means-an-event.md
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "^/// diagnostic event — so an unavailable root" backend/crates/apps/coffret-server/src/noted.rs && grep -q "No Entry Path reaches the diagnostic event" backend/crates/domain/coffret-usecase/src/catch_up/run.rs && grep -q "reaches a diagnostic event" frontend/packages/domain/format/src/model/secretBytes.ts'
assignee: null
branch: task/0910-1142-follow-up-say-diagnostic-event-where-a-comment-means-an-event
created_at: 2026-09-10T11:42:57Z
updated_at: 2026-09-10T11:53:10Z
---

# docs: say "diagnostic event" in the three comments the rename did not reach

## Overview

The rename of "log line" to "diagnostic event" across the backend's comments
left three places untouched: two where the phrase was wrapped across two
comment lines or lacked the word "log", and one in the frontend. Rewrite them
the same way. No behaviour changes; only comment text moves.

- `backend/crates/apps/coffret-server/src/noted.rs`, lines 25-29: replace
  those five doc-comment lines with

  ```
  /// and a local path is not something to put across this boundary or into a
  /// diagnostic event — so an unavailable root arrives as the sentence about it
  /// and no path at all. The Entry Path is another matter: it is the user's own
  /// name for their own file, it is what the row on the screen is keyed by, and
  /// the listing carries it already.
  ```

- `backend/crates/domain/coffret-usecase/src/catch_up/run.rs`, lines 65-67:
  replace those three doc-comment lines with

  ```
  /// No Entry Path reaches the diagnostic event: what a catch-up learned is the
  /// user's own names for their own files (spec: EL-1), and how many there now
  /// are is enough to read a run's account of itself.
  ```

- `frontend/packages/domain/format/src/model/secretBytes.ts`, lines 6-10:
  replace those five block-comment lines with

  ```
   * Key material reaches a diagnostic event through a formatter more easily
   * than through deliberate code, so every key type keeps its bytes in a private
   * field and spells itself `<redacted>` in a string context and in JSON. The
   * bytes are handed out only through [`bytes`], and as a copy, so a holder
   * cannot reach back into the key and change it.
  ```

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `noted.rs` says "diagnostic event":
      `grep -q "^/// diagnostic event — so an unavailable root" backend/crates/apps/coffret-server/src/noted.rs`.
- [x] `catch_up/run.rs` says "diagnostic event":
      `grep -q "No Entry Path reaches the diagnostic event" backend/crates/domain/coffret-usecase/src/catch_up/run.rs`.
- [x] `secretBytes.ts` says "diagnostic event":
      `grep -q "reaches a diagnostic event" frontend/packages/domain/format/src/model/secretBytes.ts`.
- [x] Existing backend, frontend, and interoperability checks continue to pass.
