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
branch: task/1105-0758-agree-the-unreadable-drop-test-with-a-listable-folder
created_at: 2026-11-04T22:57:00Z
updated_at: 2026-11-04T23:58:01Z
---

# test(device): agree the unreadable-drop test with a folder the process can list

## Overview

`drops::tests::an_empty_missing_or_unreadable_path_is_reported`
(`backend/crates/apps/coffret-device/src/drops/tests.rs`) locks a folder
with mode `000` and drops it beside an empty path and a missing file. It
reads back whether the folder can still be listed (`listable`), because a
process that may list any folder whatever its mode — the superuser's — does
list it. The earlier assertions already allow for that: the files may be
non-empty when `listable`, and the folder is reported `unreadable` only when
it is not. The last assertion does not: it insists that `dropped.handle` is
`None` ("nothing taken, so nothing is held"). Run as root, the locked folder's
page is taken, a handle is drawn, and the test fails on `main`.

Change the last assertion so that it agrees with the others: a handle is
held exactly when the folder was listable, i.e.
`assert_eq!(dropped.handle.is_some(), listable, …)`, with a message that
says why. Keep the test's intent for an unprivileged process unchanged
(nothing taken, nothing held). Do not change any product code.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The last assertion of `an_empty_missing_or_unreadable_path_is_reported` is `dropped.handle.is_some() == listable` (as an `assert_eq!` with a message)
- [x] No file outside `backend/crates/apps/coffret-device/src/drops/tests.rs` and this task file changes
- [x] `cargo test -p coffret-device drops::tests::an_empty_missing_or_unreadable_path_is_reported` passes both as root and as an unprivileged user
- [x] `make check` passes

### Before merge

(none)
