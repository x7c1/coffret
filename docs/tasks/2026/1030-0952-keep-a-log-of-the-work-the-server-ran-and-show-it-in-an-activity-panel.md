---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/explorer-v0.2
perspectives: [completeness, clarity, user-experience, spec-alignment, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1030-0952-keep-a-log-of-the-work-the-server-ran-and-show-it-in-an-activity-panel
created_at: 2026-10-30T00:52:16Z
updated_at: 2026-10-30T02:33:52Z
---

# feat: keep a log of the work the server ran and show it in an Activity panel

## Overview

Part of the explorer redesign on the `feat/explorer-v0.2` integration branch.
Today the explorer reports work in a status bar (`StatusBar.tsx`) and in
notices above the list that, once dismissed, cannot be read again. Replace
both with an **Activity panel** at the bottom of the screen, fed by a log the
server keeps.

Server:

- Keep a bounded, in-memory log of finished work — fills, syncs, freezes,
  deletions (for example `5 files, 8.2 MB removed from the Library`), folders a
  queue threw away, and fetch failures — newest first, dropping the oldest past
  a fixed count. Notices only the page knows (for example `<name> — preview of
  this format is not supported yet`, or a request that never reached the
  server) stay in the page and join the panel there. A failure stays in the log until a retry settles it. Like the
  rest of what a server holds about its work it lives exactly as long as the
  process and is never written to disk: a server started afterwards answers
  with an empty log.
- Serve it, with the work currently running and its progress, through the
  authorized loopback API alongside the existing activity answer.
- Amend LA-12 in `docs/spec/loopback-access` to say that the server keeps this
  bounded log of finished work for the life of the process and serves it to an
  authorized page, instead of only the last outcome of each kind (this is a
  deliberate change of the rule, made with the project owner).

Explorer:

- **Collapsed** (the default), the panel is one row: a chevron, **Activity**,
  a one-line summary of what is running (`Uploading Vol. 06 · 112 of 188 files
  · 1 more`), a small progress bar, and a red `<n> failed` badge when anything
  failed. A failure does not open the panel by itself; the badge is the
  signal.
- **Expanded**, it shows two groups: **In progress** — time, what, a progress
  bar, and a Stop icon button reserved for a later change (render it only once
  stopping exists; until then show no button) — and **Earlier** — finished
  entries newest first, each with a time, a dot coloured by outcome (success,
  failure, notice), the text, and a Retry icon button on a failure that can be
  retried. Retry replaces the second-attempt offers the status bar makes
  today (stopped fill/sync/freeze runs, folders a queue lost, a failed
  deletion), with the same behaviour. There is no "clear" button.
- The panel's right end holds a **?** button; hovering or focusing it shows the
  keyboard shortcuts (↑ ↓ move, → expand or go in, ← collapse or go to the
  parent, Enter open in the reader, Esc close the reader, Shift ↑ ↓ and
  Shift/⌘ click select several, right-click actions).
- Remove the status bar and the dismissable notices; every sentence they carry
  today reaches the person through the panel. The reader sheet and the tree
  end above the panel, so opening it shrinks them rather than covering it.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it` and `make e2e-electron`, and in CI) must keep passing: when a change moves something a journey relies on (a role, a label, a row it counts), update the journey in this same change so it still proves what it proved.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A backend test covers the log: newest first, the oldest dropped past the
  cap, a failure kept until a retry settles it, and an empty log after a
  restart of the state.
- [x] A backend test covers the loopback route refusing an unauthorized request
  and answering an authorized one with the log and the running work.
- [x] A unit test covers the collapsed summary for nothing running, one run,
  several runs, and failures present.
- [x] A unit test covers which entries offer Retry.
- [x] `make check`'s spec-rule and spec-citation gates pass with the amended
  LA-12.

### Before merge (verified outside the check command)

- [ ] On `make local`, the agent runs a sync and a failing operation, opens and
  closes the panel, retries the failure, and records in the PR what it saw.
