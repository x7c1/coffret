---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1109-0807-keep-the-filtered-tree-open-folders-on-a-reanswer
created_at: 2026-11-09T08:07:36Z
updated_at: 2026-11-09T08:39:46Z
---

# fix(web): leave the filtered tree's open folders alone when the same filter is answered again

## Overview

While the explorer's tree is filtered (by a name, by the most recent books,
or both), the filter is asked of the server again (`GET /api/filter`, through
`useFilter` in `frontend/packages/apps/web/src/useFilter.ts`) whenever the
Library changes under it — after a drop, a move, a rename, a sync. Each new
answer gives a new `shape` (`App.tsx`, the `useMemo` over `filterAsk` and
`filterAnswer`), and the effect in `App.tsx` that reacts to `shape` (the one
commented "The filter turned on, answered anew, or turned off") then sets the
tree's open folders to `filteredOpen(shape, held.expanded)`
(`frontend/packages/apps/web/src/filter.ts`), which opens every folder above
an answered row again. So a person who filtered by a name, closed every
folder, and then dropped a file sees the whole filtered tree spring open
again.

Open folders for a filter only when it is turned on and when its conditions
change (the name, Recent on or off, the Recent count — whatever makes
`askOf(filterSet)` a different ask). When the same ask is answered again,
leave the open folders as they are: do not re-open folders above answered
rows, and do not open folders above rows that appear for the first time in
the new answer either (the Activity panel already says what arrived). The
focus rule stays as it is (kept only on a row still drawn). Folders that are
no longer drawn may be dropped from the open set as today's `standing`
handling does; nothing else about the open set changes on a re-answer.

Tell "same conditions answered again" apart from "conditions changed" by the
ask the answer was for, not by comparing answers: keep the ask that the shape
last drawn was made for (for example, alongside `shapeDrawn`), and compare
the ask behind the new shape with it. Make sure an answer to an older ask
that arrives late is not mistaken for a re-answer of the current one.

Update the comment above the effect, and `filteredOpen`'s doc comment if its
role changes, to say what happens on a re-answer.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test (under `frontend/packages/apps/web/src/`) shows that, for
      the same ask answered twice, the second answer leaves the open folders
      exactly as they were — including when every folder was closed, and when
      the second answer holds a row under a folder that is closed.
- [x] A unit test shows that changing the name, turning Recent on or off, or
      changing the Recent count still opens the folders above every answered
      row, as before.
- [x] An e2e journey under `frontend/packages/apps/e2e/journeys/` filters the
      tree by a name, closes every folder, adds a matching file from another device and
      presses Check for changes (a drop is not used: it opens the folder the
      file lands in by design, filtered or not) and waits for the filter's
      new answer, and asserts that the folders stay closed.
