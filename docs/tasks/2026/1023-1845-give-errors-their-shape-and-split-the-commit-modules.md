---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq "revokeSession" frontend/packages/gateway/api/src/ frontend/packages/apps/ && ! grep -rq "anyhow::Result" backend/crates/apps/coffret-desktop-core/src/lib.rs backend/crates/apps/coffret-desktop-core/src/desktop/ && [ "$(cat backend/crates/domain/coffret-usecase/src/commit/commit_error.rs 2>/dev/null | wc -l)" -lt 600 ] && [ "$(cat backend/crates/domain/coffret-usecase/src/commit/keyring.rs 2>/dev/null | wc -l)" -lt 500 ] && ! grep -q "Arc<ControlObjectFault>" backend/crates/domain/coffret-usecase/src/commit/checkpoint_failure.rs'
assignee: null
branch: task/1023-1845-give-errors-their-shape-and-split-the-commit-modules
created_at: 2026-10-23T09:45:00Z
updated_at: 2026-10-23T16:35:00Z
---

# refactor: give errors their shape, drop a dead API export, and split the two largest commit modules

## Overview

Seven structural leftovers from review, none changing behaviour. Each is one
decision already taken; apply them as written. Paths are relative to
`backend/crates/` unless they start with `frontend/`.

### 1. `coffret-desktop-core` gets its own `Error`

The crate exposes `anyhow::Result` from `lib.rs:76 libraries()`,
`idle_minutes.rs:20`, `process.rs:31-34 start()`, `desktop/mod.rs:44
Desktop::new`, `desktop/issue_ticket.rs:9`, `desktop/take_drop.rs:19` and
`desktop/open_door.rs:13`, and `refusal_line.rs:16-28` / `:39-49` downcast
anyhow's chain to find `coffret_device::Error::MasterKeyNotUnlocked`. Define a
crate `Error` (variants for the device error, the host, I/O and the few other
causes these functions raise; `From` at the boundaries) and `Result`, and make
`refusal_line` / `refusal_evidence` a match over variants. `AttemptRefused::Library(anyhow::Error)`
(`attempt.rs:40`) carries the crate error instead. The Node-API addon and the
Electron app read the same lines as before; `refusal_line`'s output text does
not change.

### 2. `SourceFile::reader` / `read` return `LocalIoError`; the unreachable arm goes

`domain/coffret-usecase/src/local_scan/source_file.rs:62` (`read`) and
`:106-112` (`reader`) return `LocalError` though only `LocalIoError` can occur
(`content_hash` at `:85-88` already returns it). Narrow them and adjust the
callers. In `delete/mapped_copies.rs:308-321` `descent`, the
`LocalError::PathCollision` arm is unreachable by the function's own comment;
restructure so the caller passes what can actually occur and the arm is gone.

### 3. Remove `revokeSession` from the API client

`frontend/packages/gateway/api/src/auth.ts:60-75` exports `revokeSession`
(re-exported from `index.ts:72`) and only its own test calls it; the explorer
offers no control that ends its session (LA-18, "which the explorer offers no
control to make"). Remove the function, its re-export and its test. The server
route (`coffret-explorer-host/src/auth/mod.rs:32`, LA-16) stays: the desktop
app is its caller.

### 4. `DuplicateControlObject` names both objects

`domain/coffret-usecase/src/commit/commit_error.rs:241-244`
`DuplicateControlObject { object }` lost the two duplicate objects' handles
that the earlier warning printed. Carry both `ObjectRef`s (`first`, `second`)
on the variant, print them in Display and Redacted (an `ObjectRef` carries no
secret), re-export `ObjectRef` from `coffret-device` (`lib.rs:413-417`) and
adjust the constructions in `commit/control_listing.rs:93-100`, the CLI tests
(`coffret-cli/src/answer/tests.rs:611-614`), `from_error.rs:178` and
`commit_conformance/first_head_tests.rs:210,222`.

### 5. Split `commit/commit_error.rs` and `commit/keyring.rs`

`commit_error.rs` is 1312 lines (`CommitError` with 19 variants, `UnusableReplica`,
`UnrepairedReplica`, `ControlObjectFault`, four Display impls, four
`error::Error` impls, Redacted impls, `advice`, `From`s, 350 lines of tests).
`keyring.rs` is 920 lines (`Examined`/`examine`, `Walked`/`Found`/`rewrite`,
`replicate`/`write_generation`/`next_generation`, `ReadKeyring`, `ShortKeyring`,
`ShortReport`, `read_committed`/`write_replica`/`read_replica`) and its public
type is `ShortKeyring`, which the module name does not say. Turn each into a
directory module split by responsibility (one public type per module, named
after it; the Display/Redacted arms stay beside their type; tests beside what
they test). Paths other modules import (`commit::CommitError`,
`commit::ShortKeyring`, …) keep working through re-exports in `commit/mod.rs`.

### 6. `EndedFills` records the end where the run ends

`coffret-server/src/fill/ended_fills.rs:32-75` learns that a run ended by
observing `Fills::reported()` from `routes/file.rs:128,194`, and its own doc
(`:19-26`) admits the run it misses (a folder taken up by the `POST /api/fill`
queue from a Done run steals `on_record`, so a present page starts one more
run). Record the end in `fill/fills.rs:166-180 publish` or `fill/run.rs:231-246
finish`, whichever is the single point every run passes, and delete the
observation. In the same change, `coffret-device/src/browse/remote_in.rs:11-29`
reads the same subtree twice (`present_under`, then `entries_under`) for
`wholly_present`; read it once.

### 7. `CheckpointFailure` carries its cause

`domain/coffret-usecase/src/commit/checkpoint_failure.rs:24-53`: `Other { reason: String }`
flattens the cause into a redacted string and `SlotTaken` holds
`Arc<ControlObjectFault>` because, its doc says, the fault is not clonable.
`CommitError` (`commit_error.rs:35`) and `ControlObjectFault` (`:400`) both
derive `Clone` today. Hold the `CommitError` in `Other` and the
`ControlObjectFault` by value in `SlotTaken`, drop the `Arc`, and fix the two
stale doc sentences (`:12-13`, `:43-45`).

### Out of scope

- Any change to what is logged or shown, beyond the two handles in item 4.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-desktop-core` has a crate `Error`/`Result`; no `anyhow::Result` remains in `lib.rs` or `desktop/` (gate appended to `check_command`); `refusal_line` tests pass unchanged in their expected text.
- [x] `SourceFile::read`/`reader` return `LocalIoError`; `descent` has no unreachable arm.
- [x] No `revokeSession` under `frontend/packages/gateway/api/src/` or `frontend/packages/apps/` (gate appended); the host's revoke route tests still pass.
- [x] `DuplicateControlObject` carries two `ObjectRef`s and a CLI test asserts both appear in the line.
- [x] `commit/commit_error.rs` and `commit/keyring.rs` are each gone or under 600 / 500 lines (gates appended), the directory modules compile and `cargo doc` has no broken intra-doc links.
- [x] A server test shows a present page starting no extra fill run after a Done run (the case `ended_fills.rs:19-26` describes), and `remote_in.rs` reads the subtree once.
- [x] `checkpoint_failure.rs` holds no `Arc<ControlObjectFault>` (gate appended) and `Other` carries a `CommitError`.
