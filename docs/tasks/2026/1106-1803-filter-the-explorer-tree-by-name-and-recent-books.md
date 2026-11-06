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
branch: task/1106-1803-filter-the-explorer-tree-by-name-and-recent-books
created_at: 2026-11-06T18:03:09Z
updated_at: 2026-11-06T21:11:17Z
---

# feat(web): filter the explorer's tree by name and by the most recent books

## Overview

The server answers which rows a filtered tree keeps: `GET /api/filter`
(rules LA-40 to LA-42 in `docs/spec/loopback-access/README.md`, the route in
`backend/crates/apps/coffret-server/src/routes/filter.rs`). The explorer does
not use it yet. This change gives the explorer its filter: a search field in
the header, a small panel for "the n most recent books", and the tree drawn
from the filter's answer while a filter is on. The server is not changed,
except for the contract case below.

**The client.** Add `getFilter` to `@coffret/api`
(`frontend/packages/gateway/api/src/`), with the answer's type (`rows` of
`path`, `kind` and, on a folder, `deposited_at`; `total`; `truncated`), taking
an `AbortSignal`. Add one filter answer to the contract the server writes
(`backend/crates/apps/coffret-server/tests/routes/contract.rs`, written into
`frontend/packages/gateway/api/src/contract/answers.json`) and read it in
`contract.test.ts` like the other answers.

**The header.** The search field sits between the location and the
appearance switch (`frontend/packages/apps/web/src/Header.tsx`). Inside it, in
order: a magnifier icon, the name input (placeholder `Filter by name`), a ×
that clears the name (shown only while there is a name), a divider, and a ⏷
button (`More filters`). The header already wraps at narrow widths; the field
must keep working there (it may shrink, the input keeps a usable minimum
width).

- ⏷ opens a small panel under the field holding a checkbox
  `Recent [n] folders`, where `[n]` is a number input (whole numbers from 1;
  default 10), and one line of explanation: `Folders that directly hold files,
  newest first by when their files were added.` `Escape` or a click outside
  closes the panel; closing it keeps what was set. The panel is built so that
  more conditions can be added to it later.
- While Recent is on, a chip `Recent n ×` shows inside the field, before the
  name input; its × turns Recent off.
- The explorer waits 250 ms after the last keystroke (or change of n) before
  asking, and aborts the previous request when a new one is sent. An empty
  name with Recent off is no filter at all: the tree goes back to normal and
  nothing is asked.

**Keys.**
- `Ctrl+F` (`Cmd+F` on macOS) and `/` focus the name input, from anywhere on
  the page except while a modal dialog is open or another text input has the
  focus (`/` typed into an input stays a character). `Ctrl/Cmd+F` is taken
  from the browser's own find only while the explorer is shown (not on the
  locked screen).
- In the input, `↓` or `Enter` moves the focus to the first row of the tree.
- In the input, the first `Escape` empties the name; the next (or the first,
  when the name is already empty) gives the focus back to the tree. It does
  not turn Recent off.

**The filtered tree.** While a filter is on, the tree
(`frontend/packages/apps/web/src/TreeTable.tsx`, fed from `App.tsx`) is drawn
from the answer instead of from the folders open in the normal tree:

- Each answered row is shown, in path order, with all of its ancestors.
  Ancestors that are not themselves answered rows are drawn open and dimmed
  (the faint text colour of `theme.ts`), and stay in place: they can be
  focused and closed, but they are there to show where a match stands.
- An answered folder is drawn closed. Opening it lists its contents as today
  (`/api/list`, folder by folder), all of them, not filtered.
- An answered file is shown with its ancestors only; the other files of its
  folder are not shown.
- Rows keep today's columns, keys, selection, actions menu, details pane and
  reader. Drag-and-drop and drops onto the tree keep working on the rows that
  are shown.
- Turning the filter off (Clear, emptying the name with Recent off, or the
  chip's ×) brings back the normal tree as it was before the filter was
  turned on: the same folders open. The focus stays on the row it was on
  where that row exists in the normal tree, opening its ancestors as needed;
  otherwise it goes back to where it was before.
- After something the explorer already reloads its listings for (a drop
  carried in, a move, a rename, a delete, a refresh), the filter is asked
  again with the same name and n.

**The status line.** Only while a filter is on, a line above the tree's rows
says how many matched and offers **Clear**, which turns off both the name and
Recent:

- `12 matches` (`1 match`), or `No matches` where the answer is empty (the
  tree then shows no rows, and no "empty Library" message).
- Where the answer is `truncated`: `Showing the first 2,000 of 8,412 matches
  — narrow the name to see the rest`, numbers from the answer, grouped with
  commas.
- While a request is out, the previous rows stay shown; a failed request says
  so on that line with the same retry wording the tree's other failures use
  (`TRY_AGAIN` in `Region.tsx`), and keeps the previous rows.

**The Added column.** Only while Recent is on, the tree gets an `Added`
column after `Modified`: on an answered folder, its `deposited_at` from the
answer, formatted like `Modified` (`time` in `humanize.ts`); on a file, its
`deposited_at` from the listing; blank elsewhere. The rows stay in path
order; they are not sorted by it.

**Not in this change.**
- Keeping the reader's `↑`/`↓` read-on inside the filtered tree (the reader
  still reads on through `getBeside` as today). That is a separate change.
- Keeping the filter in the address or across a reload: a reload forgets it.
- Any change to the server's matching or to the spec's LA rules.

**States**, each covered by a test (unit tests in `apps/web`, and the
journey below):
- name only: folders matched (closed, with dimmed ancestors), a file matched
  alone, Japanese full-width / half-width names;
- a name that matches nothing (`No matches`, no rows);
- Recent only, with the Added column;
- name and Recent together (the chip and the name both shown);
- a truncated answer (the status line's wording);
- Clear, the chip's ×, and emptying the name: the normal tree comes back with
  the folders that were open before;
- a request in flight replaced by a newer one (the older one aborted, its
  answer not drawn);
- a failed request (the status line says so, rows kept);
- the keys: `Ctrl/Cmd+F` and `/` focus the input (and not while a dialog is
  open or another input has the focus), `↓`/`Enter` go to the first row, the
  two `Escape`s.

**The journey.** Add `frontend/packages/apps/e2e/journeys/13-filter.spec.ts`
(run by `make e2e-it`, in CI's `e2e` job), following the other journeys: in
the journey's Library, type a name, see the filtered rows and the status line,
open a matched folder and see its contents, turn on Recent with a small n and
see the Added column, then Clear and see the normal tree back.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `getFilter` and its answer type are in `@coffret/api`, and a filter
      answer written by the server's contract case is read in
      `contract.test.ts`.
- [x] Unit tests under `frontend/packages/apps/web/src/` cover every state
      listed in the Overview.
- [x] `frontend/packages/apps/e2e/journeys/13-filter.spec.ts` walks the
      journey described in the Overview, and `make e2e-it` passes.
- [x] `make check` passes.
