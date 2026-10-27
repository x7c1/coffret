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
branch: task/1028-0230-number-names-around-a-spread-written-last-page-first
created_at: 2026-10-27T17:10:00Z
updated_at: 2026-10-27T17:31:18Z
---

# fix(explorer): number names around a spread written last page first

## Overview

"Number the pages" in the folder-drop confirmation
(`frontend/packages/apps/web/src/renumber.ts`) keeps a joined spread's two
numbers in the order they were written, so `p4+p3.jpg` — a spread written
last page first, as a book read right to left may name it — becomes
`004+003.jpg`. A name read after that spread is put under the spread's
*last* number: `x.jpg` becomes `003_x.jpg`, which a listing puts before
`004+003.jpg`. The order recheck at the end of `judgePages` then leaves the
whole folder as it was.

Put names around a spread by the spread's numbers in value, not in the
order they were written:

- A name read after a spread goes under the larger of its two numbers:
  after `p4+p3.jpg`, `x.jpg` becomes `004_x.jpg`, which sorts after
  `004+003.jpg` (`+` sorts before `_`) and before `005.jpg`.
- A name read before the first page, where that page is a spread, goes
  under the number the spread is written with first, less one, since the
  spread's new name starts with that number: before `p2+p1.jpg`,
  `cover.jpg` becomes `001_cover.jpg`; before `p1+p0.jpg`, `000_cover.jpg`.
- The spread's own new name keeps its numbers in the order they were
  written (`004+003.jpg`), as today.

The test that covers the order recheck uses exactly this input
(`p1, p2, p4+p3, x`) to reach it, and the comment on the recheck names the
same case. After this change that input no longer reaches it: turn that
test into one for the new numbering, and find another input that still
reaches the recheck — or, if none can, keep the recheck as the guard it is,
with a comment saying so and a test that drives it directly.

Update the `renumber.ts` header comment where it says a name goes after a
page "by taking that page's last number".

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `renumber.test.ts`: `p1.jpg, p2.jpg, p4+p3.jpg, p4x.jpg, p5.jpg`
      become `001.jpg, 002.jpg, 004+003.jpg, 004_p4x.jpg, 005.jpg` (a name
      read right after the spread; `x.jpg` would be read after `p5.jpg`)
- [x] `renumber.test.ts`: `cover.jpg, p2+p1.jpg, p3.jpg, p4.jpg` become
      `001_cover.jpg, 002+001.jpg, 003.jpg, 004.jpg` (with `p3` alone, the
      spread's frame and the pages' frame tie and the folder is left whole),
      and `cover.jpg, p1+p0.jpg, p2.jpg, p3.jpg` become `000_cover.jpg,
      001+000.jpg, 002.jpg, 003.jpg` as before
- [x] `renumber.test.ts`: a spread written first page first is numbered as
      before (existing tests pass unchanged)
- [x] The order recheck is still covered by a test, by an input that
      reaches it or by driving it directly
- [x] `make check` passes
