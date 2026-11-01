---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1102-0422-keep-the-route-tests-out-of-the-desktop-trash
created_at: 2026-11-01T19:22:23Z
updated_at: 2026-11-01T20:01:31Z
---

# test: keep the route tests out of the desktop's trash

## Overview

The `coffret-server` route tests (`backend/crates/apps/coffret-server/tests/routes/`)
open the Library with the real `UnixFs` (`support/mod.rs`, `OpenLibrary.local_fs`
is an `Arc<UnixFs>`), so every case that discards a departed file asks the
desktop's own trash to take it. On macOS that is the Finder's trash
(`coffret-local-fs/src/unix_destinations/trash.rs`, `NsFileManager`).

Two consequences:

1. **A test run puts fixtures into the developer's trash.** For example
   `moves::a_folder_moved_to_where_this_device_maps_nothing_leaves_the_device`
   discards `spring.jpg` and `summer.jpg` into the person's real trash on every
   run.
2. **The suite depends on the trash being writable.** With writes to
   `~/.Trash` denied (reproduced with
   `sandbox-exec -p '(version 1)(allow default)(deny file-write* (subpath "'$HOME'/.Trash"))'`
   on the built `routes` binary), that case fails at `moves.rs:112` with
   `left=0` and two `RelocationRefused` findings. The product behaves
   correctly there — it leaves the file in place, reports it, and retries
   next run — but the test reads as a product failure in any sandbox or CI
   runner without a usable trash.

## Changes

- Let the server's opened Library take its trash as a seam (the
  `LocalTrash` port from `coffret-usecase`), and have the route tests supply
  a test trash instead of the desktop's. The in-memory fake
  (`coffret-usecase/src/in_memory_fs/local_trash.rs`) is the model; a test
  trash that actually removes the file from the temp mapped folder (so
  `assert!(!spring.exists())` still holds) is fine, as long as it never
  reaches the desktop's trash. Keep the descent and vouching checks that
  `UnixFs::discard` performs exercised where they are already tested
  (`coffret-local-fs/tests/trash_refusals.rs`); the route tests are about
  the route, not the trash.
- Apply the same rule to every other test target in the workspace that
  reaches the desktop's trash through `UnixFs` (grep for `UnixFs::new()` in
  `tests/` and `#[cfg(test)]` code and follow which ones discard). State in
  the PR which targets were changed and which were checked and left alone.
- Keep one test that the real desktop trash takes a file, in
  `coffret-local-fs`, run only when asked: `#[ignore]` with a reason, plus a
  Makefile target (e.g. `make test-desktop-trash`) with a comment saying it
  puts a temp file into the person's own trash. On a trash that refuses, it
  fails with the OS error, not a generic message.
- Production wiring is unchanged: the server and the desktop app still
  discard into the desktop's trash.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] No route test reaches the desktop's trash: the opened Library in
      `tests/routes/support/` takes a test trash, and the cases that discard
      still assert the file left the mapped folder.
- [x] Every other test target found by the grep above that reached the
      desktop's trash now uses a test trash, or the PR names it as checked
      and left alone with the reason.
- [x] The opt-in real-trash test exists in `coffret-local-fs` and is skipped
      by `make check` and `cargo test`.
- [x] `make check` passes.

### Before merge (verified outside the check command)
- [x] The built `routes` test binary passes in full under
      `sandbox-exec -p '(version 1)(allow default)(deny file-write* (subpath "'$HOME'/.Trash"))'`.
- [x] The opt-in real-trash test passes through its Makefile target on macOS.
