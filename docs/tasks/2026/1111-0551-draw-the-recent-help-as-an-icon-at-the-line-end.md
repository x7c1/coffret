---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1111-0551-draw-the-recent-help-as-an-icon-at-the-line-end
created_at: 2026-11-10T20:51:12Z
updated_at: 2026-11-10T21:01:56Z
---

# fix(web): draw the Recent explainer's button as an icon at the end of its line

## Overview

In the filter's More filters panel, the Recent line reads
`Recent [n] folders [?]`. The `[?]` that shows what Recent does is the
literal text "[?]" in a small button right after "folders"
(`frontend/packages/apps/web/src/FilterField.tsx`, the `aria-label="About Recent"`
button and its `HELP` style), which looks unfinished.

Draw that button as an icon instead — a question mark in a circle, an inline
SVG in the style of the explorer's other icons (see `SyncIcon.tsx` and the
icons in `TreeTable.tsx`), coloured from the theme tokens so it reads in both
colour schemes — and move it to the right end of the Recent line, apart from
"folders". Keep everything about how the explainer is shown as it is: it shows
while the button is hovered or focused, a press pins and unpins it, a press
elsewhere unpins it, and the button stays `aria-label="About Recent"` and
`aria-describedby` the explainer. The button keeps a hit area no smaller than
today's and a visible focus ring. Update the doc comments that call it the
`[?]`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test renders the panel and finds the About Recent button holding
      an SVG icon and no "[?]" text, still described by the explainer.
- [x] A unit test finds the About Recent button as the last item of the Recent
      line, after the count and "folders".
- [x] The existing tests of showing, pinning and unpinning the explainer pass
      unchanged in what they assert.
- [x] `make check` and `make e2e-it` pass.
