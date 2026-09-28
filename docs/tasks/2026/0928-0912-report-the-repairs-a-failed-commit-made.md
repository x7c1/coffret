---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/0928-0912-report-the-repairs-a-failed-commit-made
created_at: 2026-09-28T09:12:33Z
updated_at: 2026-09-28T09:43:02Z
---

# fix: report the Keyring repairs a failed commit made, and put its advice after the cause

## Overview

A commit that repairs the committed Keyring and then fails has written replicas to Storage that no caller is told about, and the one error that advises what to do says the advice before the cause. Both are in how a failed commit reports itself.

1. **Repairs made before a commit failed are lost.** `commit/run.rs` accumulates repairs (around line 76) and extends them after each attempt's examine (around line 105), but every `?` after that — `replicate`, `journal::commit`, `index.refresh`, the next attempt's `catch_up`, `examine` failing with `UnrepairedKeyring` (around lines 94-95), and `ConflictLimitReached` (around lines 149-151) — drops what was accumulated. The doc at around lines 29-33 states that earlier attempts' repairs are not reported. KL-15 says a repair performed is never silent, and `KeyringRepair` exists to carry it (`commit/keyring_repair.rs`). Make every error the commit returns carry the repairs this run performed before it failed, and have the CLI print them through `report::repair_line` (`coffret-cli/src/report.rs`, around line 133) on the failure path as it does on success (around lines 124-130). Keep an attempt's repairs apart from `UnrepairedKeyring`'s `rewritten`, which is the current generation's. Whether that is one wrapper around the commit error or a field on each variant is this task's call; prefer the one that does not make every caller of the commit error learn a new shape. The server reports them the way it reports the success path's repairs. Correct the doc at around lines 29-33.
2. **`UnrepairedKeyring` gives its advice before its cause.** Errors print as a chain, each layer's sentence and then its `source()`'s. `UnrepairedKeyring`'s `Display` (`commit/commit_error.rs`, around lines 394-431) ends with the advice ("running again examines…", around line 428), and its source follows, so a person reads what to do before why. Take the advice out of the `Display` and have the shells print it after the chain: give the error a way to state its advice, and have the CLI's final error output add it as its own line after the chain. Update the comment that states the current order as intended (around lines 519-529) and the tests that pin the chain (around lines 826-890 and 931-952). A search found no other error that both advises and has a source; confirm that with the same search and say the result in the PR.

Guard: `redacted()` output is unchanged for every error type — the advice line is for the person at the terminal, not for the log.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A commit that repairs on attempt 1 and then ends in `ConflictLimitReached` returns an error carrying that repair, pinned in the commit conformance suite (`make check`)
- [x] A commit that repairs and then fails in a later step (for example `journal::commit`) carries the repair too (`make check`)
- [x] The CLI prints the repair lines on the failure path, pinned in a CLI test (`make check`)
- [x] `UnrepairedKeyring`'s `Display` no longer contains the advice, and the CLI prints the advice after the chain, pinned by the updated chain tests (`make check`)
- [x] `make check` passes
