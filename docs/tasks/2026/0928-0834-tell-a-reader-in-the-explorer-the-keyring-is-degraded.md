---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 0
check_command: 'make check'
assignee: null
branch: task/0928-0834-tell-a-reader-in-the-explorer-the-keyring-is-degraded-retry-1
created_at: 2026-09-28T08:34:22Z
updated_at: 2026-09-28T09:05:42Z
---

# fix(web): tell a reader in the explorer that the Keyring is degraded, not only a freeze

## Overview

A fetch, an Entry fetch and an uncommitted freeze now carry a degraded Keyring on their outcomes, and the device turns it into a finding (`Finding::DegradedKeyring`, wire reason `keyring_degraded`) that needs no attention. KL-15 says replica loss is never silent, and the person it matters most for is one who only reads — who opens files and folders in the explorer and never syncs or freezes there. Today that person never sees it: the work answer carries findings for sync runs and freeze runs (`frontend/packages/gateway/api/src/work.ts`, `SyncOfItsOwn.findings`, `FreezeOfItsOwn.findings`), but a fill run has no findings list, so a degraded Keyring an Entry fetch meets while the explorer fills a folder or opens a file goes only to the log.

1. **Give a fill run the findings its fetches report.** Add `findings` to the fill run on the server (`coffret-server/src/fill/`, the run the work answer's `fill` carries) and on the wire (`work.ts`, `work.json` and its golden fixtures), filled from what the fill's Entry fetches report through `Finding::of`. A degraded Keyring is reported once per fill run, however many Entries met it, as the device's guard reports it once per flow. The one-file fetch behind `GET /api/file` (`coffret-server/src/routes/file.rs`) arms a fill of the holding folder already; make the degraded report it meets reach that fill run's findings too, or say in the PR why it takes another path.
2. **Show it.** The status bar shows a fill run's findings the way it shows a sync's and a freeze's (`frontend/packages/apps/web/src/StatusBar.tsx` and the finding sentences it uses), without escalating: the finding does not turn the run into a stopped one.

Guard: the finding's wire reason `keyring_degraded` and its sentences stay as they are; this change carries it further, it does not redefine it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A fill run whose Entry fetches meet a degraded Keyring reports it once in its findings on the work answer, pinned in a server route case (`make check`)
- [x] The work contract's golden fixture carries a fill run's findings and `work.ts` reads them (`make check`)
- [x] The status bar shows a fill run's degraded-Keyring finding, pinned in `StatusBar.test.tsx` (`make check`)
- [x] `make check` passes

## Last subagent report

Check phase (attempt 1) exited non-zero in `make check`'s `cargo doc --no-deps --workspace` under `RUSTDOCFLAGS="-D warnings"`: `crates/apps/coffret-server/src/fill/fill_run.rs:50`, the public doc of `FillRun::degraded`, links to the private function `graver` (``[`graver`]``), which `rustdoc::private-intra-doc-links` refuses. Everything else passed.

The whole of attempt 1 — implemented and reviewed — is commit `60ce8df` on branch `task/0928-0834-tell-a-reader-in-the-explorer-the-keyring-is-degraded`. Start from it (`git checkout 60ce8df -- .` from the repository root brings its tree into this branch), fix the doc link (name `graver` in plain backticks, or move the sentence to a place that may link it), and run the full check_command.
