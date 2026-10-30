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
branch: task/1031-0146-settle-the-idle-lock-and-freeze-retry-waits-and-cover-picked-files
created_at: 2026-10-30T16:46:54Z
updated_at: 2026-10-30T17:56:22Z
---

# test: make the desktop journey's idle-lock wait fail fast on use, and cover the files picked with +

## Overview

Two gaps in what the tests prove.

**The desktop journey's idle-lock wait failed once.** In
`frontend/packages/apps/e2e/electron/desktop.spec.ts`, the step that waits for
the idle lock (`expect.poll(lockedForIdling, …)` around line 230) once waited
its full two minutes without seeing the lock; a re-run passed and the log was
overwritten. The server pushes the idle deadline back on every request that
needs the Library (`lock_when_idle`), so something the journey or the page
keeps asking for during the wait can keep the lock from ever landing. Find what
can push the deadline back while the journey waits (a poll the page keeps
running, a request the journey itself makes, a timer), and make the wait
deterministic: either the journey stops everything that counts as use before
it waits, or the server's notion of use leaves out what the page asks for on
its own without a person, whichever the code shows is right. Do not paper over
it by lengthening the timeout or retrying the step. If the cause turns out to
be a product defect (the explorer keeps a Library unlocked forever while a tab
merely sits open), fix the product and say so in the PR; the spec rule for the
idle lock is in `docs/spec/` (search for `idle`), and a change to what counts as
use updates that rule in this same change. Keep the journey's log of the run
when it fails (for example by copying it next to the test output) so a future
failure can be read.

**Nothing tests how the desktop app hands over the files picked with +.**
`frontend/packages/apps/desktop/src/preload.cts` maps each picked `File` to its
path (`pickedPath`) and removes duplicates, and
`frontend/packages/apps/web/src/drop.ts` `pickedFiles` takes the branch where a
desktop app holds the paths (`heldBy`) apart from the browser's own files.
Add tests for both: the path mapping and de-duplication, and `pickedFiles`
with and without a desktop app holding the paths (the spec rule for a pick is
LA-26 in `docs/spec/`).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test covers `pickedPath` and the de-duplication of picked paths.
- [x] A unit test covers `pickedFiles` when a desktop app holds the paths and
  when it does not.
- [x] The desktop journey's idle-lock step waits on a condition that nothing
  in the journey or the idle page can push back, with no longer timeout and no
  retry added.

### Before merge (verified outside the check command)

- [x] The agent runs `make e2e-it` and `make e2e-electron` three times each in a row and each run
  passes; the PR records the runs and how long the idle-lock step took.
