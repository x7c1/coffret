---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && grep -rqF "Folders that directly hold files, newest first by when their files were added." frontend/packages/apps/web/src/filter.ts'
assignee: null
branch: task/1109-0601-lift-the-more-filters-panel-and-tuck-its-explainer
created_at: 2026-11-09T06:01:42Z
updated_at: 2026-11-09T08:06:59Z
---

# fix(web): show the More filters panel above the reader and tuck its explainer behind [?]

## Overview

Two fixes to the More filters panel the explorer's search field opens with ⏷
(`frontend/packages/apps/web/src/FilterField.tsx`), both seen while using the
explorer day to day.

**The panel hides under the reader.** The panel (`PANEL`, `zIndex: 10`) and
the reader (`frontend/packages/apps/web/src/ReaderView.tsx`, also
`zIndex: 10`) share a stacking level, so pressing ⏷ while the reader is open
draws the panel underneath the reader, where it can be neither seen nor used.
The panel must be drawn above the reader (and anything else at that level,
such as the Activity panel's overlay in `ActivityPanel.tsx`), while staying
below the dialogs (`Dialog.tsx`, `zIndex: 30`) and the actions menu
(`ActionsMenu.tsx`, `zIndex: 20`) unless there is a reason to put it above the
menu; state the chosen order where the levels are defined. If the header that
holds the field forms a stacking context of its own that keeps the panel
under the reader whatever its `zIndex`, fix that instead, so the panel is
really in front.

**The explainer takes up the panel.** The panel now holds the Recent line
(`Recent [n] folders`) and, under it, the paragraph `RECENT_EXPLAINED` from
`frontend/packages/apps/web/src/filter.ts`. Keep only the line, followed by a
small `[?]` control: `Recent [n] folders [?]`. The explainer is shown when the
`[?]` is hovered, when it holds the keyboard focus, and when it is tapped
(a second tap, `Escape`, or a press elsewhere hides it again; `Escape` while
the explainer is shown hides the explainer before it closes the panel). The
words of `RECENT_EXPLAINED` do not change. The `[?]` is reachable by `Tab`,
has an accessible name (for example "About Recent"), and the explainer is tied
to it with `aria-describedby` (or an equivalent) so a screen reader reads it.
A tooltip built from the `title` attribute alone is not enough, because it
appears neither on keyboard focus nor on a tap.

Update the doc comment at the top of `FilterField.tsx` (it says the panel
holds "a line saying what it does") to describe the `[?]`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] An e2e journey under `frontend/packages/apps/e2e/journeys/` opens the
      reader, presses ⏷, and asserts that the More filters panel is visible
      and on top (for example, `elementFromPoint` at the panel's centre lands
      inside the panel) and that its Recent checkbox can be clicked and turns
      Recent on.
- [x] Unit tests (`FilterField.test.tsx`) assert that the panel does not show
      the `RECENT_EXPLAINED` text until the `[?]` is hovered, focused, or
      clicked, that it shows it in each of the three cases, and that `Escape`
      while it is shown hides the explainer and leaves the panel open.
- [x] `RECENT_EXPLAINED` in `filter.ts` keeps its words:
      `grep -rqF "Folders that directly hold files, newest first by when their files were added." frontend/packages/apps/web/src/filter.ts`
      (appended to `check_command`).

