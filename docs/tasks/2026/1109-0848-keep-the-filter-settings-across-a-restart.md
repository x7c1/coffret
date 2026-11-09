---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment]
max_refine_rounds: 3
retries_remaining: 0
check_command: 'make check && make e2e-it && grep -rq "LA-43" docs/spec/loopback-access/README.md'
assignee: null
branch: task/1109-0848-keep-the-filter-settings-across-a-restart-retry-1
created_at: 2026-11-09T08:48:33Z
updated_at: 2026-11-09T10:58:11Z
---

# feat: keep the explorer's filter settings across a reload and a restart of the desktop app

## Overview

The explorer's tree can be filtered by a name and by the most recent books
(the header's search field and its More filters panel,
`frontend/packages/apps/web/src/FilterField.tsx`; the settings are
`FilterSettings` in `frontend/packages/apps/web/src/filter.ts`, held in
`App.tsx` as `filterSet`, starting from `NO_FILTER`). Today they live only in
the page's state, so a reload of the page, or the desktop app started again,
forgets them. Keep all of them — the name, whether Recent is on, and the
Recent count — so that the explorer opens again filtered exactly as it was
left, with the tree drawn filtered (the search field's text and the
`Recent n ×` chip show that it is).

Keep them the way the colour scheme is kept (spec LA-38,
`docs/spec/loopback-access/README.md`): as this device's own state, by the
server, in the state directory's `explorer.json` beside the appearance
(`backend/crates/apps/coffret-device/src/appearance_file.rs`), never in a
Library and never uploaded. The desktop app serves the explorer on a new port
at each launch, so browser storage would not survive a restart; that is why
the server keeps it. Concretely:

- `explorer.json` holds the filter settings beside `appearance`. Writing one
  must not lose the other (read, change the one field, write back), and a
  file written before this change (appearance only) still reads, with no
  filter. Give the file type a name that fits both (it is no longer only the
  appearance's), and update its doc comments.
- The server answers the kept settings and keeps new ones over a route of its
  own beside `/api/appearance` (for example `GET`/`POST /api/filter-settings`
  with `{"name": …, "recentOn": …, "recent": …}`). A body that is not valid
  (a count below 1 or not an integer, a name too long — pick a sane bound) is
  refused as `400` and leaves what was kept. Nothing kept, or a file that
  cannot be read, is answered as no filter.
- Decide whether a locked server answers these routes. The typed name is
  about the Library's contents, unlike the colour scheme, so lean toward
  answering them only as the Library's other routes are; whichever is chosen,
  write the reason in the rule.
- The explorer host forwards the new route as it does `/api/appearance`, and
  the server's fence test lists it: add it to `EVERY_ROUTE` in
  `backend/crates/apps/coffret-server/tests/routes/fences.rs`. Add
  `/api/filter`, which is missing from that list, at the same time.
- The page reads the kept settings when it starts and applies them before it
  first draws a filtered tree, and keeps every change. Typing in the name
  must not send a request per key: keep the name once typing pauses (a short
  debounce) and on leaving the field. A failure to keep is logged and does not
  disturb the filter on screen.
- Add a rule **LA-43** to `docs/spec/loopback-access/README.md`, beside
  LA-38, saying the above (what is kept, where, the route and its body, the
  refusal, what a device that never kept anything is answered, and the lock
  behaviour with its reason), and mention it in the spec's overview where
  LA-38 is listed. Cite LA-43 from the route and the tests, as the
  appearance code cites LA-38.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Rust tests for the route (beside
      `backend/crates/apps/coffret-server/tests/routes/appearance.rs`) show:
      nothing kept is answered as no filter; what is kept is what the next
      `GET` answers, also from a server started afresh; an invalid body is
      refused as `400` and leaves what was kept; keeping the filter settings
      leaves the kept appearance as it was, and keeping the appearance leaves
      the filter settings; an `explorer.json` holding only `appearance`
      still reads.
- [x] The fence test's `EVERY_ROUTE` lists the new route and `/api/filter`.
- [x] Frontend unit tests show that the page applies kept settings at start
      and keeps a change, with the name kept once after typing pauses rather
      than once per key.
- [x] An e2e journey under `frontend/packages/apps/e2e/journeys/` filters by
      a name, turns Recent on with a count, reloads the page, and asserts that
      the search field, the `Recent n ×` chip, the panel's count and the
      filtered tree are as they were; it then clears the filter so that later
      journeys start unfiltered.
- [x] `docs/spec/loopback-access/README.md` has a rule LA-43 (`grep` appended
      to `check_command`).

## Last subagent report

The check (`make check && make e2e-it`) failed on the first attempt. `make check`
passed; two e2e journeys failed, both after the work had been put on top of the
latest `main`, which by then held two other filter changes: the More filters
panel drawn above the reader (journey `15-filter-over-reader.spec.ts`, and a
shared stacking order `LAYER` in `theme.ts`), and the filtered tree leaving its
open folders alone when the same filter is answered again (`answeredTree` and
`sameAsk` in `filter.ts`, the `answered` ask in `useFilter.ts`, and the effect
in `App.tsx` that compares the ask behind each answer; journey
`16-filter-reanswer.spec.ts`). Build on top of both.

- `14-filter-read-on.spec.ts` (line 76): with the tree filtered by `kept` and
  the reader open on `read-on-a-kept/page-0000.jpg`, `ArrowDown` closed the
  reader instead of moving to `page-0001.jpg`: the URL became `#path=runs`, the
  focus went to the `runs` row, and the filter stayed on (`2 matches`). So
  something the kept filter does (applying the kept settings when the page
  starts or after an unlock, the keep itself, or a re-ask it causes) resets the
  tree or the view while the reader is open, and so it ends the reading. The
  filter must be kept without disturbing the tree, the focus, or the reader.
- `15-filter-over-reader.spec.ts` (line 23): the reader region never appeared
  (test timeout). This probably follows from the same cause, or from journey 14
  ending before it cleared its kept filter, so that 15 opened filtered. Every
  journey that leaves a kept filter behind must clear it, and a journey that
  needs an unfiltered start must not depend on the one before it (journey 16
  ends with a name filter on).

Also make sure that the explorer's own state (the tree, the focus, the
reader) is not reset when the kept settings are read back and they equal the
filter already on screen.

