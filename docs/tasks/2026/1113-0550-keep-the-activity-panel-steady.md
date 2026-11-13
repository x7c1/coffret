---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1113-0550-keep-the-activity-panel-steady
created_at: 2026-11-12T20:45:06Z
updated_at: 2026-11-13T07:03:38Z
---

# fix(web): keep the Activity panel steady while a run turns into a fetch, and draw no page scrollbar while the reader slides in

## Overview

Two layout faults a person sees in daily use of the explorer, both around the
Activity panel at the bottom (`frontend/packages/apps/web/src/ActivityPanel.tsx`).
They are in one task because both are the same kind of fault — the page's
layout moving where nothing the person did should move it — and both are
checked the same way, by measuring element boxes in the browser.

1. **The In progress row jumps when it switches to a fetch.** While something
   runs, the collapsed row's summary (and the expanded panel's **In progress**
   line) momentarily switches to the reader's fetching text and back. On each
   switch the row's height or width changes — the text, the progress bar, the
   Stop button or the ending label coming and going — so the panel visibly
   shakes. Find what changes size in the switch (the summary text's width, the
   `Bar` appearing and disappearing, the `StopButton` or the `ENDING` label, a
   line wrapping) and lay the row out so that a change of what it says does not
   change its height, nor move the parts beside it: for example a fixed row
   height, a summary that truncates with an ellipsis instead of wrapping, and a
   reserved slot for the bar and the stop. Do not change what the row says or
   when, only how it is laid out.
2. **A horizontal scrollbar flashes when the reader opens with the Activity
   panel expanded.** The reader slides in from the right over the explorer
   (`ReaderView.tsx` and the layout that hosts it); while it is part-way in,
   the page gets a horizontal scrollbar for a moment. Find what overflows the
   viewport during the slide (the reader's off-screen start position, the
   Activity panel's width, a transform that still counts towards the scroll
   width) and keep it from widening the page — for example clip the
   overflow on the container the reader slides within. The page must still
   scroll vertically where it did, and the reader's slide must look the same.

Add a journey (or extend an existing one) under
`frontend/packages/apps/e2e/journeys/` that checks both in headless Chromium:
for 1, the Activity panel's bounding box keeps its height (and the parts
beside the summary keep their x) across the switch between a run's line and
a fetch's line; for 2, `document.documentElement.scrollWidth` never exceeds
`clientWidth` while the reader opens with the Activity panel expanded,
sampled over the slide (e.g. every animation frame). Each must fail on the
tree before the fix — say in the report how you saw it fail.

Do not change `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A journey shows the Activity panel's height, and the position of the
      parts beside the summary, unchanged across a switch between a run's
      line and a fetch's line.
- [x] A journey shows no horizontal overflow of the page at any frame while
      the reader opens with the Activity panel expanded.
- [x] `make check && make e2e-it` passes.

### Before merge (verified outside the check command)

- [ ] A person sees, in the desktop app, the Activity panel stay still while
      a book is fetched, and no horizontal scrollbar while the reader opens
      with the Activity panel expanded.
