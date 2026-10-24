---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && test -f backend/crates/gateway/google-drive-store/tests/commit_conformance.rs && grep -q "commit_conformance" Makefile && ls backend/crates/domain/coffret-usecase/tests/ | grep -q "^conformance_minting.rs$"'
assignee: null
branch: task/1024-1440-run-the-commit-conformance-suite-against-real-drive
created_at: 2026-10-24T05:40:00Z
updated_at: 2026-10-24T08:48:16Z
---

# test(drive): run the commit conformance suite against real Google Drive, and the ObjectStore suite against the minting double

## Overview

The commit protocol on Google Drive rests on two Drive behaviours: an object
can be created under an id another device minted with `files.generateIds`
under the same grant, and a create under an id that names a live file answers
`AlreadyExists`. `InMemoryMintingStore`
(`backend/crates/domain/coffret-usecase/src/in_memory_minting_store.rs`) is a
double built on those two assumptions, and the commit suite runs against it in
every `cargo test` (`tests/commit_conformance_minting.rs`). Nothing checks the
assumptions against Drive itself: they were confirmed by hand once, before the
change that consumed the head's carried slot was merged. If Drive answered
differently, the double would pass and real commits would break.

Two gaps, both closed here:

1. **The commit suite on real Drive.** Add
   `backend/crates/gateway/google-drive-store/tests/commit_conformance.rs`,
   after `backend/crates/gateway/s3-store/tests/commit_conformance.rs` and
   using the Drive test support in `tests/support/` the ObjectStore run
   (`tests/conformance.rs`) uses: each case in a subfolder of its own under
   `COFFRET_DRIVE_FOLDER_ID`, trashed when the case ends, skipped when the
   variables are unset. The catalogs stay in memory, as in the S3 run.
   Include the cases the minting run includes (read its module docs for which
   races it holds and why). Add it to `make drive-store-it` and say so in the
   target's comment and in `docs/guides/environments.md` where the Drive
   targets are described. It is never part of CI: it needs an account and a
   consent.
2. **The ObjectStore suite on the minting double.** The double has no
   `ObjectStore` conformance run of its own, so a drift between it and the
   in-memory store's contract goes unnoticed. Add
   `backend/crates/domain/coffret-usecase/tests/conformance_minting.rs`
   running `object_store_conformance!` over it, as `tests/conformance.rs`
   does over the name-keyed store. Where a case cannot hold on a minting store
   by construction, leave it out with the reason in the module docs, as the
   minting commit run does.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `google-drive-store/tests/commit_conformance.rs` exists and
      `make drive-store-it` runs it (gates appended); without the variables
      its cases report themselves skipped, so `make check` stays green.
- [x] `coffret-usecase/tests/conformance_minting.rs` runs the ObjectStore
      suite over `InMemoryMintingStore` in `make check` (gate appended).

### Before merge (verified outside the check command)

- [x] On this macOS machine, `make drive-store-it` passes with the commit
      suite included, against the test folder configured in
      `~/.config/coffret/local.mk` (an agent may run it: the person granted
      the test client for this). The merging session records the per-suite
      counts and timings in the PR, and that every subfolder was trashed.
