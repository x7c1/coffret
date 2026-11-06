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
branch: task/1106-0656-measure-the-explorer-at-a-hundred-thousand-files
created_at: 2026-11-06T06:56:21Z
updated_at: 2026-11-06T08:13:02Z
---

# test: measure the explorer against a Library of a hundred thousand files

## Overview

A filter for the explorer is planned. It will match a typed name against
every path segment of every Entry, and pick the n most recently deposited
books. Before it is designed, we need numbers for how the parts it rests on
behave at a hundred thousand files. This change adds what produces those
numbers. It adds no filter and changes no product behaviour.

Three things are measured, all against a Library of about 100,000 files
created locally (no cloud account):

1. **Full-scan match in the server.** Read every row of the device's Index
   `entries`, split each Entry Path into its `/`-separated segments,
   normalize each segment (Unicode NFKC, then case-folded), and test whether a
   query is a substring of any segment (a match never crosses a `/`). Do the
   same for "the n most recently deposited books": a book is a folder that
   directly holds files, its time is the latest `deposited_at` among the
   files directly in it, files directly under the Library root count together
   as one, and the n newest are taken. Time both: the median and the worst of
   several runs, for a few queries (one that matches many Entries, one that
   matches a few, one that matches none) and for n = 5 and n = 50. This is a
   measurement harness written in Rust over the same Index the server reads,
   not a new route. It must not write to the Index.
2. **The folder listing the explorer uses today**, including the per-folder
   size and count aggregates it returns, timed at the Library root and at the
   largest folder.
3. **The tree's `↑`/`↓` with many rows open.** In the explorer, run headless
   with Playwright's Chromium, open folders until about 5,000 rows are drawn,
   then about 20,000. Press `↓` and measure the time from the key press to the
   frame where the new focus has been painted, as a median and worst over
   several presses. Today the tree draws every open row and draws all of them
   again whenever the focus moves.

What to add:

- **A way to create the large Library**, reusing `make local`'s machinery (a
  MinIO container, a device state under `.tmp/`, a fixed Passphrase). For
  example, a new `coffret-fixtures` scenario that lays out about 100,000 small
  files as books and series of realistic shape (a few to a few hundred pages
  per book, series nesting a few levels deep, Japanese and long names among
  them, `deposited_at` spread over time), and a target that seeds it under
  its own directory so it does not disturb `make local`'s Library. The files
  can be tiny, since content is not what is measured. The Library must be
  real: created, filled and committed the way `make local`'s is, so that the
  Index and the listing are what a device holds.
- **A single target that runs the three measurements** against that Library
  and prints a plain report of the numbers (entry count, and for each
  measurement the median and worst in milliseconds).
- **Documentation** in `docs/guides/environments.md` (or a guide it links to)
  of the two targets: what they need (Docker, as `make local` does), where
  they keep their state, how long seeding takes, and how to remove it.

Keep the new targets out of `make check`, as `make local` and the `-it`
targets are: they need Docker and a browser. Add unit tests for the pure parts
that `make check` can run (the segment normalization and matching, and the
choice of the n most recent books, over small hand-built inputs), so the
harness measures logic that is known to be correct.

Out of scope: a filter route, any change to the explorer's UI or the Index
schema.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Unit tests run by `make check` cover the measured matching: matching
      per path segment and never across `/`, NFKC (full-width and half-width
      are the same) and case-insensitive substring matching, and the n most
      recent books (folders with files directly in them, the root's own files
      counted as one, the time being the latest `deposited_at` directly in
      the folder).
- [x] `make check` passes and does not run the new seeding or measuring
      targets.

### Before merge (verified outside the check command)

- [x] On the development machine, the seeding target creates a Library of
      about 100,000 files, and the measuring target prints all three
      measurements for it. The PR body records the report (the entry count and
      every median and worst), the machine it ran on, and how long seeding
      took.
