---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "ctrl_c" backend/crates/apps/coffret-cli/src/main.rs && grep -rq "UnsupportedJournalRecordSchema" backend/crates/apps/coffret-cli/src/answer/ && grep -rq "KeyringReplica" backend/crates/domain/coffret-usecase/src/sync/'
assignee: null
branch: task/1023-1835-say-what-a-run-found-and-where-it-stands
created_at: 2026-10-23T09:35:00Z
updated_at: 2026-10-23T12:40:00Z
---

# fix(cli,server): say what a run found and where it stands — six gaps in reports, progress and interruption

## Overview

Six defects in what the CLI and the server tell a person, found while
dogfooding and in review. Each is one decision already taken; implement them as
written. Paths are relative to `backend/crates/`.

### 1. An old-format Library is told to be recreated, not "unsupported"

A Library written before the current Journal record schema is refused by
`domain/coffret-format/src/control/journal_record/decode.rs:51-54`
(`Error::UnsupportedJournalRecordSchema`, displayed as "unsupported Journal
record payload schema 2"). The CLI classifies it under kind `sync`
(`apps/coffret-cli/src/answer/failure.rs`, `classified()`; only
`JournalRecordNeedsNewerBuild` is special-cased, at line 126), and the server's
start-up catch-up turns it into `unverified`
(`apps/coffret-server/src/api_error/from_error.rs:170`, `CommitError::Format(_)`).
Both read as "the Library is broken".

- Give the CLI a kind of its own for a record schema older than this build
  reads (beside the `JournalRecordNeedsNewerBuild` case), and an advice line
  from `CommitError::advice` (`domain/coffret-usecase/src/commit/commit_error.rs:655`)
  / `coffret_device::Error::advice` saying that the Library predates this
  build's format and has to be made again (`coffret init`, then bring the
  books in again); nothing of it is read or repaired.
- Give the server the same distinct answer: an `ApiError` kind that the
  explorer can show as "this Library predates this app's format; make it
  again", not `unverified`. Follow the shape of the `JournalRecordNeedsNewerBuild`
  arm at `from_error.rs:167`.

### 2. A deletion says it is removing local copies

After a deletion commits, `delete/run.rs:222-228` discards the mapped copies
(`delete/mapped_copies.rs`, hashing included) while the progress stage stays
`Committing`. Add a `Phase` variant to `domain/coffret-usecase/src/progress.rs:30`
for removing this device's copies, step to it just before
`discard_mapped_copies`, and name it in the two exhaustive matches —
`apps/coffret-server/src/routes/work/step_dto.rs:56` and
`apps/coffret-cli/src/progress.rs:401`. Update the `Committing` doc comment
(lines 54-79) where it says the stage stays after the head.

### 3. Keeping a listing is a cache; failing to keep it loses no answer

`domain/coffret-usecase/src/catch_up/remote_head.rs:68-75`: when the remote head
is current, `memory.remember_listing(...)` is awaited with `?`, so a catalog
failure inside it discards the `RemoteHead::Current` already established.
`catch_up/run.rs:62-67` (`catch_up_catalog`) and `fetch/entry_run/listed.rs:66-68`
do the same after the Index has advanced. In all three, a failure to remember
the listing is logged (`warn!`, with the redacted cause) and the answer in hand
is returned; the next read simply pays the listing again.

### 4. Retained Keyring candidates are reported

A commit interrupted after its Keyring replicas were put leaves `key-<gen>-<digest>-r<i>-of-<n>.cfrt`
objects of a generation no head ever named. Spec OC-4 lets such suspected
orphans be reported; today `commit/control_listing.rs:114` only records their
names into `handles` and nothing reports them, while retained Containers do
reach `Settled::Retained` → `Finding::Settled` → the CLI's text and JSON.

Make `ControlListing` keep the Keyring replica names it saw, derive the
generations no head names, and report them through the same `Finding` family
(a sibling of `Settled`, or a `Settled` with a kind that says Keyring) in
`apps/coffret-device/src/findings.rs`, `finding.rs` (Display) and the CLI's
`answer/found.rs` (JSON). Reporting only: nothing is trashed (OC-4).

### 5. Ctrl-C stops the CLI cleanly

`apps/coffret-cli/src/main.rs:216` awaits `run(cli.command, form)` with no
signal handling, so Ctrl-C kills the process wherever it is. Race the run
against `tokio::signal::ctrl_c()` in a `tokio::select!`; when the signal wins,
drop the run's future (the pending-rows guard and the start-up guard already
release on drop, see `pending_rows_guard.rs:1-3`) and exit through the normal
`Failure` path with a line that says the run was interrupted and that the next
`sync` settles what it left. Exit status is non-zero. Do not add a
`CancellationToken` plumbing through the use cases; dropping the future is the
mechanism.

### 6. `coffret-interop dump` renders a Container's meta section

`apps/coffret-interop/src/dump.rs:27` tries each control-object kind's key and
renders the payload by name; a Container is not handled. Add an optional
`--keyring <path>` argument (a Keyring replica object): when every control
kind fails and a keyring is given, unwrap the Container's key from it with the
recovery code (`unwrap_container_key`), open the Container with
`ContainerOutline::open` (`container_reader/container_outline.rs:109`) and
render `kind`, `pad_len` and the entry table as JSON. Without `--keyring`, a
Container is reported as "a Container; pass --keyring to render its meta
section". Reuse `coffret_format::decode` paths as `verify/check_container.rs:20`
does; do not re-implement the meta decoding.

### Out of scope

- Trashing retained Keyring candidates (OC-5 was dropped).
- Cancellation inside a use case at finer grain than dropping the future.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] CLI test (`apps/coffret-cli/src/answer/tests.rs`, beside the `newer_build` case at line 568): an `UnsupportedJournalRecordSchema` failure is classified under its own kind and its advice says the Library predates this build and is to be made again (`grep -rq "UnsupportedJournalRecordSchema" backend/crates/apps/coffret-cli/src/answer/` is appended to `check_command`).
- [x] Server test (`apps/coffret-server/src/api_error/tests.rs`): the same error maps to its own `ApiError` kind, not `unverified`.
- [x] Delete conformance (`delete_conformance/`, after the pattern of `sync_conformance/progress.rs`): the steps observed during a deletion include the new removing-local-copies phase after `Committing`; `routes/work/contract.rs` names it.
- [x] Tests with a refusing Index (`refusing_index.rs` pattern) for `remote_head`, `catch_up_catalog` and the entry run's listed path: when remembering the listing fails, the answer already established is still returned and a warning is logged.
- [x] Sync conformance (`sync_conformance/uncertain_commit.rs` or a sibling): a Keyring generation whose replicas were put but which no head names is reported as a retained Keyring candidate in `SyncOutcome`; the CLI's text and JSON name it (`grep -rq "KeyringReplica" backend/crates/domain/coffret-usecase/src/sync/` is appended to `check_command`).
- [x] `apps/coffret-cli/src/main.rs` races the command against `tokio::signal::ctrl_c()` (`grep -q "ctrl_c"` is appended to `check_command`); a unit test of the interruption path asserts the interrupted `Failure` line and non-zero status.
- [x] `apps/coffret-interop` test (in `dump/tests.rs`, beside `dump/control_object.rs`'s `a_body_is_rendered_by_name`; refine split `dump.rs` into the `dump/` directory module): `dump --keyring` on a Container renders its kind, pad length and entries; `dump` without it on a Container says to pass `--keyring`.

### Before merge (verified outside the check command)

- [ ] On this machine, with the agent's development Library (`libraries-dev/agent`, see `docs/guides/environments.md`): start a `coffret sync` over a drop of a few files, press Ctrl-C during it, and confirm the interrupted line, a non-zero exit, and that the next `coffret sync` settles the leftover (no row stuck). The merging session records what it ran in the PR.
