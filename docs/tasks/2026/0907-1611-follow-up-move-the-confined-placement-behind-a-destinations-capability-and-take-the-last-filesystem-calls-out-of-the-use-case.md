---
status: completed
pipeline_phase: null
plan: null
base_ref: null
follow_up_of: docs/tasks/2026/0907-1732-move-the-confined-placement-behind-a-destinations-capability-and-take-the-last-filesystem-calls-out-of-the-use-case.md
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "both capabilities" backend/crates/gateway/coffret-local-fs/src/unix_fs.rs && ! grep -q "fetching device.s folder is a real one" backend/crates/domain/coffret-usecase/src/freeze_conformance/fixtures.rs && ! grep -q "for the same reason it is real here" backend/crates/domain/coffret-usecase/src/fetch_conformance/fixtures/mod.rs && ! grep -q "only under the directories the backend hands it" backend/crates/domain/coffret-usecase/src/fetch_conformance/mod.rs && ! grep -q "whether a flow made the call itself" backend/crates/domain/coffret-usecase/src/local_operation.rs'
assignee: null
branch: task/0907-1611-follow-up-move-the-confined-placement-behind-a-destinations-capability-and-take-the-last-filesystem-calls-out-of-the-use-case
created_at: 2026-09-07T16:11:36Z
updated_at: 2026-09-07T17:41:15Z
---

# docs(backend): describe the fetch's placement as an in-memory capability in the comments the Destinations move left behind

## Overview

Five comments in five files still describe the arrangement before the fetch's
placement moved behind the `Destinations` capability: two capabilities where
there are now three, a fetching device that places into a real folder where it
now places into the in-memory fake, and a call a flow could make itself where
every call is now a gateway's. Nothing in the build or at runtime changes; each
fix replaces comment text only.

- `backend/crates/gateway/coffret-local-fs/src/unix_fs.rs`, lines 20–24,
  anchored at `/// It answers both capabilities the flows reach this disk through:`.
  Replace those five doc lines with:

  ```
  /// It answers all three capabilities the flows reach this disk through:
  /// [`Spool`] here, and [`MappedRoots`](coffret_usecase::MappedRoots) and
  /// [`Destinations`](coffret_usecase::Destinations) beside it. One type for all
  /// three because a device has one disk — a composition root hands the same
  /// value to a request's several fields, and a case that scripts a folder which
  /// will not list and a spool which will not flush scripts one thing.
  ```

- `backend/crates/domain/coffret-usecase/src/freeze_conformance/fixtures.rs`,
  lines 221–223, anchored at
  `/// The fetching device's folder is a real one — a fetch places bytes through the`.
  Replace those three doc lines with:

  ```
  /// Both suites' fetching devices place into an in-memory disk through
  /// `Destinations`, so this is the fetch suite's reader rather than a second copy
  /// of it.
  ```

- `backend/crates/domain/coffret-usecase/src/fetch_conformance/fixtures/mod.rs`,
  lines 11–12, anchored at
  `// The freeze suite reads the fetching device's folder too, and it is a real one`.
  Replace those two comment lines with:

  ```
  // The freeze suite reads the fetching device's folder too, and it is in that
  // fixture's own in-memory disk for the same reason it is in this one's.
  ```

- `backend/crates/domain/coffret-usecase/src/fetch_conformance/mod.rs`,
  lines 43–44, anchored at
  `//! but only under the directories the backend hands it. It is behind the`.
  Replace those two doc lines with:

  ```
  //! It reads and writes files, as the sync suite does — a fetch ends at a folder —
  //! but every one of them is in the in-memory disk the fixture makes rather than
  //! on a directory the backend hands it. It is behind the
  ```

- `backend/crates/domain/coffret-usecase/src/local_operation.rs`, lines 16–19,
  anchored at
  `/// direction the bytes were going. A gateway that implements a capability over`.
  Replace those four doc lines with:

  ```
  /// direction the bytes were going. Every one of those calls is a gateway's,
  /// behind one of the three capabilities over that disk, and it answers in this
  /// vocabulary through [`LocalIoError`](crate::LocalIoError).
  ```

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `unix_fs.rs` no longer says "both capabilities"
      (`! grep -q "both capabilities" backend/crates/gateway/coffret-local-fs/src/unix_fs.rs`).
- [x] The freeze fixtures no longer call the fetching device's folder a real one
      (`! grep -q "fetching device.s folder is a real one" backend/crates/domain/coffret-usecase/src/freeze_conformance/fixtures.rs`).
- [x] The fetch fixtures no longer say the folder is real here
      (`! grep -q "for the same reason it is real here" backend/crates/domain/coffret-usecase/src/fetch_conformance/fixtures/mod.rs`).
- [x] The fetch suite's module doc no longer says it writes under directories the backend hands it
      (`! grep -q "only under the directories the backend hands it" backend/crates/domain/coffret-usecase/src/fetch_conformance/mod.rs`).
- [x] `local_operation.rs` no longer says a flow made the call itself
      (`! grep -q "whether a flow made the call itself" backend/crates/domain/coffret-usecase/src/local_operation.rs`).
- [x] `make check` is green.
