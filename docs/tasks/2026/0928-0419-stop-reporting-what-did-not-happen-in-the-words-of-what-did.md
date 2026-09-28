---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqE "trashed: bool" --exclude-dir=target backend/crates/domain/coffret-usecase/src/sync/ && grep -rqE "LeftInStorage" --exclude-dir=target backend/crates/domain/coffret-usecase/src/sync/ && grep -rqE "untrashed" --exclude-dir=target backend/crates/apps/coffret-device/src/ && grep -rqE "CheckpointOutcome::Failed" --exclude-dir=target backend/crates/apps/coffret-device/src/'
assignee: null
branch: task/0928-0419-stop-reporting-what-did-not-happen-in-the-words-of-what-did
created_at: 2026-09-28T04:19:13Z
updated_at: 2026-09-28T04:51:05Z
---

# fix: stop reporting what did not happen, or did not finish, in the words of what did

## Overview

A run's outcome sometimes says something happened that did not, or reports a step that failed in the words of one that succeeded. Each case below is an instance of one rule: what a run tells its caller distinguishes what it did from what it did not do or could not finish. None of them changes what a run does to Storage or to the Index; they change what it says.

1. **"settling what an interrupted run left" on every sync.** `sync/run.rs` (around line 102) emits `progress.step(Step::begun(Phase::Settling))` before calling `settle::settle`, and `settle` returns at once when `index.pending_rows()` is empty (`sync/settle.rs`, around lines 95-98). So every sync announces a settle, in the CLI (`coffret-cli/src/progress.rs`, around line 154) and in the explorer (`frontend/packages/apps/web/src/fill.ts`, `DOING.settling`), when no interrupted run left anything. Report the `Settling` phase only when there are pending rows to settle — for example by passing the progress sink into `settle` and emitting the step after the emptiness check, or by checking in `run.rs` — and update the expected phase sequences in `sync_conformance/progress.rs` (around lines 50 and 86). The CLI already leaves a phase with nothing in it unsaid (`progress.rs`, around lines 170-176); this makes the settle phase follow the same rule. The phase's wording stays.
2. **A disposal Storage refused is reported as done.** `Settled::Disposed { trashed: bool }` (`coffret-usecase/src/sync/settled.rs`, around lines 49-58) folds two different things into `false`: the earlier run never uploaded, so there was nothing on Storage, and Storage refused the trash, which leaves an object no current state names (OC-1, OC-4). `dispose` (`sync/settle.rs`, around lines 241-290) writes the refusal only to a `warn!`, and the device's line (`coffret-device/src/finding.rs`, around lines 243-247) says "nothing committed it, so what it left was disposed of" either way. Replace the bool with a three-way value — `NeverUploaded`, `Trashed`, `LeftInStorage` — carrying for `LeftInStorage` what Storage answered, the way `UntrashedRemoval` carries its `cause` (`commit/untrashed_removal.rs`). The device's line for `LeftInStorage` says the object is still in Storage and that orphan cleanup is what finds it; the other two keep today's sentence. Pin each sentence in the device's Display tests, and add a settle case with a store that refuses the trash, in the `sync_conformance/interruption.rs` family, asserting `LeftInStorage`.
3. **A commit's untrashed removals and a checkpoint that failed reach no caller.** `CommitOutcome` (`coffret-usecase/src/commit/commit_outcome.rs`) reports `untrashed: Vec<UntrashedRemoval>` — its doc says "Reported so a later run can finish it rather than being lost in a diagnostic event" — and `checkpoint: CheckpointOutcome`, whose `Failed { cause }` says a Snapshot could not be written. Nothing under `backend/crates/apps/` reads either field, so both are lost in exactly the way the doc says they must not be. Turn them into findings the device reports alongside the others (`coffret-device/src/findings.rs`, `finding.rs`), for sync and freeze wherever the outcome carries a `CommitOutcome`: one line per untrashed removal naming the Container and what Storage answered, and one line for a failed checkpoint saying the commit stands and the next qualifying commit writes the checkpoint (CK-8).
4. **The server's sync and freeze runs do not read `mappings`, silently.** The outcomes' docs say `mappings == 0` is the state where counts alone would say "everything is already there", and the CLI says it through `report::nothing_mapped`. The server's `sync/run.rs` (around lines 50-58) and `freeze/run.rs` (around lines 63-70) read only the counts. The fill run already leaves unmapped folders to the explorer with a comment saying why (`coffret-server/src/fill/run.rs`, around lines 62-69: "The explorer says this over the rows already, out of the listing itself"). Do the same at the two sync and freeze sites: say in a comment that the explorer tells an unmapped device from its listing, so the work answer does not carry `mappings`. No wire change.
5. **"A run that only settled exits 0" has no test at the CLI.** `report::findings` in `coffret-cli/src/report.rs` (around lines 162-166) decides the exit status from `needs_attention()`; inverting that condition breaks no test today, because the CLI cannot build a `Findings` without a use-case type. Add a test that does: expose a constructor for a `Findings` holding only settled findings from `coffret-device` behind a feature that only `coffret-cli`'s `[dev-dependencies]` turns on — the pattern `stub-bucket` already follows (`coffret-device/Cargo.toml`, `[features]`) — and assert in `coffret-cli` that such a run prints its settled lines and exits 0. Extend the same test to the findings item 3 adds.

Decisions this task makes, so a reviewer can check them:

- None of the new findings needs attention: `LeftInStorage`, an untrashed removal and a failed checkpoint all leave the committed state correct and are finished by a later run or by orphan cleanup, so the exit status of a run that only reports them stays 0. They are said, not escalated.
- The explorer does not show them. The server already maps `Settled` to no finding (`coffret-server/src/finding.rs`, around lines 121 and 441); the new ones follow it, since the explorer never said "disposed of" either. The server's mapping is exhaustive, so it names each new case explicitly.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `Settled::Disposed` no longer carries `trashed: bool`, and a `LeftInStorage` case exists in the sync use case (grep gates)
- [x] The device reads `CommitOutcome.untrashed` and `CheckpointOutcome::Failed` into findings (grep gates under `coffret-device/src/`)
- [x] A sync with no pending rows reports no `Settling` phase, pinned in `sync_conformance/progress.rs` (`make check`)
- [x] A settle against a store that refuses the trash reports `LeftInStorage` (`make check`)
- [x] A `coffret-cli` test asserts that a run holding only settled findings, and the findings item 3 adds, exits 0 (`make check`)
- [x] The device's Display tests pin the sentences for `NeverUploaded` / `Trashed`, `LeftInStorage`, an untrashed removal and a failed checkpoint (`make check`)
- [x] `make check` passes
