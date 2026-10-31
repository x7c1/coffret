---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1101-0220-scan-only-the-folders-a-drop-landed-in-when-the-drop-arms-its-sync
created_at: 2026-10-31T21:44:46Z
updated_at: 2026-10-31T22:31:42Z
---

# feat(server): scan only the folders a drop landed in when the drop arms its sync

## Overview

A plain drop copies its files into the mapped folder and arms a sync
(`routes/upload/finish_drop.rs`, `sync/arm_sync.rs`). That sync walks every
folder this device maps, so the cost of each drop grows with the whole
Library rather than with what was dropped.

Narrow the sync a drop arms to the paths the drop wrote: walk only the
folders the drop landed in (the drop's destination and what it created under
it), and carry in nothing found elsewhere. A full sync stays where it is: the
`coffret sync` CLI, `POST /api/sync` (the retry of a sync Storage stopped),
and anything else that arms a sync without a drop. When a narrowed sync and a
full one are both wanted, the full one covers the narrowed one rather than
both running. A drop stopped from the Activity panel (`stop_receiving.rs`)
follows the same rule for what it already wrote.

The pieces of a sync that are not about scanning keep their scope:
settlement of pending work (OC-2) still runs before the scan, and a narrowed
sync must not mistake a file outside its scope for deleted or departed — it
does not look there at all. Write the scope rule into `docs/spec/` where a
sync's walk is defined: a sync covers every mapped folder unless a drop
armed it, in which case it covers the drop's paths. Update the doc comments
in `routes/sync.rs` (which says a sync takes no path) and `routes/upload/`
to match.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it`
and `make e2e-electron`, and in CI) must keep passing.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A server test covers a drop's sync carrying in the dropped files and
  leaving an unrelated new file in another mapped folder untouched, which a
  later full sync then carries in.
- [x] A server test covers a narrowed sync not reporting files outside its
  scope as deleted or departed.
- [x] A server test covers a full sync armed while a drop's sync waits
  running once and covering both.
- [x] A test covers `POST /api/sync` and the CLI still walking every mapped
  folder.
- [x] The spec states the scope rule, and `make check`'s spec checks pass.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent drops a few files onto one folder with an
  unrelated new file placed by hand in another mapped folder, and records in
  the PR that only the dropped files were committed and the other waited
  for a full sync (the CLI's `coffret sync`).
