---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && ! git grep -qiE 'defer' -- backend && git grep -qF 'pub enum Surfaced' -- backend/crates/domain/coffret-usecase/src/sync/surfaced.rs && git grep -qF 'surfaced = outcome.surfaced.len()' -- backend/crates/domain/coffret-usecase/src/sync/run.rs && git grep -qF -- '- surface (' docs/concepts/library/README.md"
assignee: null
branch: task/0827-0716-rename-sync-deferred-to-surfaced
created_at: 2026-08-27T07:16:13Z
updated_at: 2026-08-27T08:24:00Z
---

# refactor(backend): rename sync's Deferred finding to Surfaced

## Overview

The three flows that report findings they do not act on name that concept two
different ways. Fetch calls it `Surfaced` (`fetch/surfaced.rs:15`, carried as
`FetchOutcome::surfaced` and logged as `surfaced = outcome.surfaced.len()`,
`fetch/run.rs:221`), and freeze carries `FreezeOutcome::surfaced`. Sync alone
calls the same concept `Deferred` (`sync/deferred.rs:14`) — "a file the sync
found needing work it does not do" — carried as `SyncOutcome::deferred`
(`sync/sync_outcome.rs:44`), `Survey::deferred` (`sync/survey.rs:25`), and
logged as `deferred = outcome.deferred.len()` (`sync/run.rs:118`). The concept
documents already speak the fetch/freeze language — "A scan **surfaces** every
file needing `update`" (`docs/concepts/library/README.md:100`) — so the sync
code is the odd one out, and a reader crossing from `fetch_folders` to
`sync_folders` meets two names for one idea.

Rename sync's side to match:

- `sync/deferred.rs` → `sync/surfaced.rs`; `pub enum Deferred` →
  `pub enum Surfaced`; the re-export in `sync/mod.rs` follows
  (`pub use surfaced::Surfaced;`). The variants (`PackResident`,
  `DeletedLocally`) and every doc comment's meaning stay as they are — only the
  wording that says "deferred" moves to "surfaced" (e.g. the enum's own doc and
  `sync/run.rs:39`'s "findings in [`SyncOutcome::deferred`]").
- `SyncOutcome::deferred` → `SyncOutcome::surfaced`, `Survey::deferred` →
  `Survey::surfaced`, and the `info!` field in `sync/run.rs:118` becomes
  `surfaced = outcome.surfaced.len()` — the exact shape `fetch/run.rs:221`
  has, so the two flows' log lines read alike.
- Every use site follows: `sync/scan/mod.rs`, `sync/scan/examine.rs`,
  `sync/scan/deletions.rs`, and the conformance modules
  (`sync_conformance/modification.rs`, `scope.rs`, `roots.rs`, `import.rs`,
  `repeat.rs`).
- `fetch::Surfaced` and `sync::Surfaced` sharing a name is the point, not a
  collision: the two enums are module-scoped exactly like the two flows' other
  outcome types, and no module imports both.
- Register the verb in the Library concept's Collocations
  (`docs/concepts/library/README.md:36`–`:48`): one list entry in the existing
  style, e.g. `- surface (a finding a run reports but does not act on)`,
  placed with the other flow verbs. Nothing else in `docs/spec/` or
  `docs/concepts/` changes — the concept prose already uses the surviving
  word.

This is the rename that was deliberately left out of the unavailable-roots fix
(`docs/tasks/2026/0825-2307-detect-unavailable-mapped-roots.md`, Out of scope:
"renaming a public type is its own change with its own diff").

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `backend/crates/domain/coffret-usecase/src/sync/surfaced.rs` defines
      `pub enum Surfaced` with the `PackResident` and `DeletedLocally` variants
      unchanged in shape and meaning, `sync/deferred.rs` no longer exists, and
      `sync/mod.rs` re-exports `Surfaced` (the check command requires
      `pub enum Surfaced` in the new file).
- [x] No spelling of defer survives anywhere under `backend/` — type, field,
      log key, or rustdoc prose (`! git grep -qiE 'defer' -- backend` is part
      of the check command).
- [x] Sync's summary log line names the finding count `surfaced`, matching
      fetch's (`git grep -qF 'surfaced = outcome.surfaced.len()'` on
      `sync/run.rs` is part of the check command).
- [x] The Library concept's Collocations list registers `surface` in the
      existing entry style (`git grep -qF -- '- surface ('` on
      `docs/concepts/library/README.md` is part of the check command).
- [x] Behavior is untouched: every sync, freeze, and fetch conformance case
      passes under `make check` with only renames in the diff — no variant
      added or removed, no assertion's expected value changed beyond the type
      and field names.

## Out of scope

- **Acting on the findings.** Update propagation into Packs
  (PK-9..PK-12) stays future work; this task changes only what the reported
  finding is called.
- **Fetch and freeze.** `fetch::Surfaced`, `FetchOutcome::surfaced`, and
  `FreezeOutcome::surfaced` already carry the surviving name and do not
  change; `NotFrozen` (the freeze finding's element type) keeps its own name.
- **Historical documents.** Task files under `docs/tasks/` that mention
  `Deferred` are records of past work and stay as written.
