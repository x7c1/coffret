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
branch: task/1028-0101-test-the-order-recheck-and-sort-pages-once
created_at: 2026-10-27T16:01:00Z
updated_at: 2026-10-27T17:05:27Z
---

# refactor(explorer): test the page rule's order recheck and sort a group's names once

## Overview

Two loose ends in "Number the pages"
(`frontend/packages/apps/web/src/renumber.ts`) after #372.

1. **The order recheck has no test.** At the end of `judgePages`, the new
   names are checked once more with `readInOrder`, and a folder whose new
   names would not keep the reading order is left whole with
   `{ reason: 'order', earlier, later }`. Since names put after the same
   page gained a serial, the inputs the other tests use no longer reach
   that path. Find an input that does — a folder where the rule's own
   names would still invert — and test that the folder is left whole and
   that `earlier` and `later` name the two names. If no input can reach
   it, say so in a comment on the check and keep it as the guard it is,
   with a test that drives it directly.
2. **A group's names are sorted twice.** `renumber()` sorts `unchanged` into
   reading order, and `linesOf` sorts renamed and unchanged names together
   again with the same comparison. Sort in one place: keep the order where
   `Group.unchanged`'s doc promises it and let `linesOf` merge the two
   already-ordered lists, or drop the promise and sort only in `linesOf`.
   The preview's order must not change.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `renumber.test.ts`: a folder that reaches the order recheck is left
      whole with `reason: 'order'`, naming the two names that would invert
      (or the check is driven directly, with a comment saying why no input
      reaches it)
- [x] A group's names are put in reading order in one place, and the
      existing `renumber.test.ts` and `DropConfirm.test.tsx` cases pass
      unchanged
- [x] `make check` passes
