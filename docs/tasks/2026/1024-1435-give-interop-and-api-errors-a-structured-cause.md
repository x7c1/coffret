---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq "anyhow" backend/crates/apps/coffret-interop/Cargo.toml && ! grep -rqE "cause: String" backend/crates/apps/coffret-server/src/api_error/'
assignee: null
branch: task/1024-1435-give-interop-and-api-errors-a-structured-cause
created_at: 2026-10-24T05:35:00Z
updated_at: 2026-10-24T07:30:29Z
---

# refactor: give coffret-interop an error type of its own, and keep the cause of an ApiError as a value

## Overview

Two places where an error's structure is thrown away early.

1. **`coffret-interop` returns `anyhow::Result`.** The crate
   (`backend/crates/apps/coffret-interop/src/`) has no `Error` / `Result` of
   its own, and `dump/tests.rs` matches failures by the exact `to_string()`
   text. Add an `error.rs` (or an `error/` module, following the crate layout
   the other app crates use) whose variants are the crate's kinds of failure
   — reading a fixture, decoding a control object, a key that does not open,
   and so on, as the code actually distinguishes them — and have the tests
   match on variants rather than text. Drop the `anyhow` dependency from the
   crate. `main.rs` still prints the whole chain of causes on failure.
2. **`ApiError` takes its cause as `cause: String`.** Every constructor in
   `backend/crates/apps/coffret-server/src/api_error/` (`from_error.rs`:
   `from_commit`, `from_storage`, `catalog_unusable`, `library_predates_format`
   and the rest) formats the cause into a string at the point of conversion.
   Keep it as a value instead — a `caused_by: Box<dyn std::fmt::Debug + Send + Sync>`
   or a boxed `std::error::Error` where every cause is one — and format it
   only where it is logged. What reaches the client must not change: the
   response bodies carry kinds and fixed sentences, never the cause (EL-1 /
   EL-5 in `docs/spec/`), and the existing tests on the bodies pin that.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-interop` has its own error type, its tests match on variants,
      and its `Cargo.toml` no longer depends on `anyhow` (gate appended).
- [x] No `cause: String` remains under `coffret-server/src/api_error/` (gate
      appended); the server log still records the cause, and the response
      bodies are byte-for-byte what they were (existing body tests unchanged
      in expectation).
