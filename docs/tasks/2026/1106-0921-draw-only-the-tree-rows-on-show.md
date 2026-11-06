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
branch: task/1106-0921-draw-only-the-tree-rows-on-show
created_at: 2026-11-06T09:21:09Z
updated_at: 2026-11-06T09:55:55Z
---

# perf(web): draw only the tree rows on show

## Overview

The explorer's tree (`frontend/packages/apps/web/src/TreeTable.tsx`) puts every
open row in the DOM (`rows.map` in the `<tbody>`) and draws them all again
whenever the focus moves. `make large-measure` shows what that costs: with
about 5,000 rows open a `↓` takes about 380 ms from the key press to the
painted focus, and with about 20,000 rows about 1.6 s.

Draw only the rows on show, plus a few dozen rows before and after them, with
spacers above and below that keep the scroll height of the whole list. Every
row has the same height, so the range follows from `scrollTop`, the area's
height and the row height alone. The cost of a `↓` then no longer grows with
the number of open rows.

What must keep working as it does today, each under a test:

- **Scrolling ahead of the focus.** `scrollAhead` keeps the focused row
  `ROWS_AHEAD` (3) rows from either edge. Today it measures the focused
  row's DOM box. A focused row may now be outside the drawn range (after
  `↓` at the last drawn row, after `End`-like jumps, after a folder opens
  above it), so compute its position from its index and the row height, and
  scroll so it is drawn and kept 3 rows from the edge.
- **Keys.** `↑` `↓` `←` `→` `Enter` `Escape`, Shift-arrows and the
  reader's read-on act on the row list, not on what is drawn, so they reach
  rows that are not yet drawn.
- **Dragging.** A drag over the tree still targets the row under the
  pointer. Holding a drag near the top or bottom edge of the tree scrolls it
  (if it does today, it must still reach rows that were not drawn when the
  drag began; if it does not today, say so in the PR and leave it).
- **What a screen reader is told.** Each drawn row still says its level, and
  the tree says how many rows it holds in all, not how many are drawn
  (`aria-setsize` / `aria-posinset` on the rows, or `aria-rowcount` /
  `aria-rowindex`, whichever fits the tree's current roles).
- **The columns.** The value columns' widths are measured over all rows
  (`values` / `columnWidth`), not only the drawn ones, so they do not jump
  while scrolling.
- **Notes and refusals.** "loading…" rows and refused-listing rows with
  their Ask again button are rows like the others.

The measuring harness in `frontend/packages/apps/e2e/scale/measure.spec.ts`
counts open rows by counting `tr[data-key]` in the DOM. With virtualization
that count is only the drawn rows: change it to count open rows in a way that
still holds (for example the tree's total from the ARIA attributes above), so
`make large-measure` keeps reporting "↓ with N rows" for N open rows.

Out of scope: rows of different heights, and virtualizing anything other than
the tree.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Web unit tests show that with many rows open (thousands) only a bounded
      number of rows is in the DOM, that the drawn range follows the scroll
      position, and that the spacers keep the full scroll height.
- [x] Unit tests cover `↓` / `↑` from the last / first drawn row reaching
      an undrawn row and scrolling it into view 3 rows from the edge, and a
      folder opening or closing above the focus.
- [x] Unit tests cover the ARIA total and position on drawn rows, and the
      column widths taken over all rows.
- [x] Existing tree, reader and drag tests pass unchanged in what they
      assert, or are changed only where they counted DOM rows.
- [x] `make check` passes.

### Before merge (verified outside the check command)

- [x] `make large-measure` against the large Library reports `↓` with about
      5,000 and about 20,000 rows open; the PR body records both medians and
      worsts beside the figures before this change (383.6 / 477.4 ms and
      1583.6 / 1628.5 ms).
