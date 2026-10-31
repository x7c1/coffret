---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, spec-alignment, error-type-design, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1031-0717-stop-an-add-a-pack-or-a-fetch-from-the-activity-panel-before-it-commits
created_at: 2026-10-30T22:17:13Z
updated_at: 2026-10-31T01:32:46Z
---

# feat: stop an add, a Pack or a fetch from the Activity panel before it commits

## Overview

The Activity panel lists the work in progress with a Stop button drawn for it,
but nothing can be stopped yet: once an add (a drop or **+**), a "Pack this
folder…" or a fetch has started, it runs to its end. Make Stop work.

**What can be stopped, and until when.**

1. An add (drop or **+**), a "Pack this folder…" (freeze), and a fetch (a
   reader's fetch or a fill) can be stopped. A delete cannot: it shows no Stop.
2. A run can be stopped at any point *before* it starts trying to commit. Once
   it starts trying to commit, its row in the Activity panel says
   `Finishing…` and shows no Stop: a freeze commits in one Journal batch
   (spec: PK-7), so there is no state in which part of it is in the Library,
   and a commit that may already have landed must not be reported as stopped.
3. Stop is a request; the run answers it. The row says the run is stopping
   until the run has actually ended, and only then that it stopped. A stop
   that arrives after the run began committing is answered by the run's real
   outcome (committed or failed), never by "stopped".

**What stopping an add leaves.**

4. Stopping an add undoes it on this device: the files this add wrote into the
   folder chosen on this device are removed (only those — never a file that was
   there before, never a file the person picked or dropped from elsewhere,
   which stays where it was), and the Pack reservations the drop wrote on this
   device (spec: PK-17, and the reservation the server's log of finished work
   keeps outside its usual scope, LA-12) are withdrawn. The Activity panel keeps
   a line such as `Stopped — nothing was added to the Library`, with no Retry;
   adding again is dropping (or picking) again.
5. Objects already sent to Storage but never committed are left to the existing
   cleanup of orphaned objects; stopping does not try to delete them.
6. A stopped fetch or fill leaves what it already placed (whole files, each
   recorded as usual) and stops before the next file; nothing half-written is
   left at a path (the existing temporary-file rule applies).

**The rules.** The queued / running / committing / committed states of a run
and what a stop does in each are not written down yet. Write them in
`docs/spec/` (loopback access, beside LA-12, and the freeze and fetch rules
the stop touches), and in the concept docs where they describe a run's
outcome (`docs/concepts/`): a run's result (stopped, failed, finished) is a
separate axis from whether its batch committed. Add the stop route to the
loopback API (keyed like every route, LA-2) and its typed call to the gateway
(`frontend/packages/gateway/api/`), and wire the Activity panel's Stop to it.

Write code and comments so they do not tie coffret to one kind of content.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it`
and `make e2e-electron`, and in CI) must keep passing; add a journey that
stops an add of a folder and finds nothing added and nothing left behind.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Tests stop an add while it is receiving, while it is queued, and while
  it is running, and find nothing committed, the add's own files removed from
  the chosen folder, and a file that was there before kept.
- [x] A test asks to stop a run after it began committing and finds the run's
  real outcome reported, not "stopped", and the Stop no longer offered.
- [x] A test covers the answer to a stop being lost (the page reloads mid-stop)
  and the Activity panel still showing the run's real end.
- [x] Tests stop a freeze and a fill, and find nothing half-written.
- [x] A delete offers no Stop.
- [x] The spec rules for stopping and the run states are in `docs/spec/`, and
  the spec register's checks pass.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent drops a folder of a few hundred images, stops
  it while it is running, and records in the PR what the Activity panel said,
  what the chosen folder holds afterwards, and that the Library lists nothing
  new.
