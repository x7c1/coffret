---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq "DegradedKeyring" backend/crates/ && ! grep -rqi "desktop shell" backend/Cargo.toml scripts/desktop.sh .github/workflows/ backend/crates/apps/coffret-desktop/ && grep -q "keyring_degraded" frontend/packages/gateway/api/src/finding-reasons.json'
assignee: null
branch: task/1014-0343-call-a-runs-keyring-finding-short-type-the-checkpoint-failure-and-call-the-desktop-app-an-app
created_at: 2026-10-13T18:43:00Z
updated_at: 2026-10-14T07:00:36Z
---

# refactor: call a run's Keyring finding short, carry a checkpoint failure as a typed cause, and call the desktop app an app

## Overview

Three naming and typing findings from the concept-alignment and
error-type-design reviews of PRs #244, #253 and #273. None changes behaviour
or a wire string.

### 1. A run's finding is *short*; the set's state is *degraded*

The Keyring concept (`docs/concepts/keyring/README.md:102-112`) keeps two
words apart: **short** is what a run found — no valid replica at some
position of the set, to that run — and **degraded** is the set's own state,
established only where a loss was. The code calls the run's finding
`DegradedKeyring` (`coffret-usecase/src/commit/keyring.rs:504`, with
`DegradedReport` at `:605`, the `degraded: Option<DegradedKeyring>` fields of
every outcome, `Finding::DegradedKeyring` in `coffret-device/src/finding.rs:114`,
and `Facts::DegradedKeyring` in the CLI), so a reader of the code infers the
set is degraded wherever the type appears, which PR #273 showed is exactly
the mistake that let a repaired set go on being called degraded.

Rename the Rust side to the concept's word: `ShortKeyring`, `ShortReport`,
fields and locals `short` where they hold the run's finding, the device
finding `Finding::ShortKeyring`, and the CLI `Facts::ShortKeyring`. The gate
in `check_command` greps that `DegradedKeyring` is gone from `backend/crates`.
Keep `degraded` only where the set's state is meant — `established()`, the
log line "the committed Keyring is degraded and awaits repair" (`:568`), the
explorer sentence in `coffret-server/src/finding.rs:206`.

**Wire strings stay.** The server's reason `keyring_degraded` and the CLI
kind `degraded_keyring` are read by the explorer's contract files and by
scripts; renaming them is a compatibility change this task does not make.
Where each string is produced, write one comment saying the wire name
predates the rename and names the condition the explorer shows (loss
established), and leave the frontend untouched (the gate greps the reason in
`finding-reasons.json` to make sure).

### 2. A checkpoint failure carries a typed cause

`CheckpointOutcome::Failed { cause: Box<CommitError> }`
(`coffret-usecase/src/commit/checkpoint_outcome.rs:13-39`) becomes
`Finding::CheckpointFailed { cause: String }` in
`coffret-device/src/findings.rs:292-299` through `chained()`
(`finding.rs:286-295`), which renders the unredacted Display chain into a
`String`, because `CommitError` cannot be cloned into a finding. The
sibling `UntrashedRemoval` keeps a typed `cause: StorageError`. A string
cause cannot be matched on, and it is the one finding text not built from
the redacted form (EL-1).

Introduce a small clonable type in the usecase — `CheckpointFailure`, an
enum of the ways a checkpoint write fails as `after_commit.rs:120-131`
observes them (Storage refusing or unreachable, the record not serializing,
whatever the match on `CommitError` yields; one variant per cause a caller
could decide on, and an `Other` that keeps the redacted sentence for the
rest) — and carry it in both `CheckpointOutcome::Failed` and
`Finding::CheckpointFailed`. Display renders from the redacted form. Move
the fixtures (`findings.rs:577,1017`, `coffret-cli/src/report.rs:321-323`)
and the server mapping (`coffret-server/src/finding.rs:170-180`, which keeps
hiding the finding from the explorer) together. Delete `chained` if nothing
else uses it.

### 3. The desktop app is an app

`docs/guides/environments.md` calls it "the desktop app" (PR #249), while
the crate still calls itself the desktop *shell*: the type `Shell`
(`coffret-desktop/src/unlock/shell.rs`), the module doc of `main.rs`, the
crate `description` in its `Cargo.toml:3`, and the phrase "desktop shell" in
`backend/Cargo.toml:94-95,140-156`, `scripts/desktop.sh:3-35`,
`.github/workflows/ci.yml:20` and `toolchain-drift.yml:51`, and
`ui/unlock.js:3-13`. "Shell" is also this codebase's word for an app crate
over the device (`coffret-cli/src/main.rs:3` "Everything here is a shell",
`coffret-shell` the crate): that sense stays everywhere, including
`coffret-desktop/Cargo.toml:24` "a shell over the same three crates".

Rename what means the desktop app: the type `Shell` → `Desktop` (its file
`desktop.rs`; `Shell::new` → `Desktop::new`, the `shell` locals and `State`
parameters → `desktop`), the phrase "desktop shell" → "desktop app" in the
files above, "the shell's one window" and the like in the desktop crate's
docs → "the app's". The user-visible string in `open_library.rs:47` ("the
shell did not finish starting") becomes "the app did not finish starting".
The gate in `check_command` greps that "desktop shell" is gone from the
listed places.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `DegradedKeyring` and `DegradedReport` no longer exist under
      `backend/crates`; the run's finding is `ShortKeyring` / `ShortReport`
      in the usecase, device and CLI, and the existing tests
      (`a_degraded_keyring_is_said_as_loss_only_where_a_loss_is_established`
      and its siblings) are renamed to say *short* where they mean the run's
      finding and pass
- [x] The wire strings `keyring_degraded` and `degraded_keyring` are produced
      unchanged, the frontend is untouched, and the explorer contract tests
      pass (gate greps `finding-reasons.json`)
- [x] `CheckpointOutcome::Failed` and `Finding::CheckpointFailed` carry the
      same `CheckpointFailure` type; a test shows the finding's text is the
      redacted form of a Storage failure; `chained` is gone or has another
      caller
- [x] No "desktop shell" remains in the workflow files, `backend/Cargo.toml`,
      `scripts/desktop.sh` or the desktop crate (gate in `check_command`);
      the `Shell` type is `Desktop`
- [x] `make check` passes
