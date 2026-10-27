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
branch: task/1027-1332-number-a-spread-whose-second-number-drops-the-leading-text
created_at: 2026-10-27T04:32:13Z
updated_at: 2026-10-27T04:39:02Z
---

# feat(explorer): number a spread whose second number drops the leading text

## Overview

"Number the pages" in the folder-drop confirmation
(`frontend/packages/apps/web/src/renumber.ts`) numbers a joined spread only
when both of its numbers carry the most common frame's text around them:
beside `foobar19_0017.jpg`, `foobar19_0018+foobar19_0019.jpg` becomes
`018+019.jpg`. Real books also write the second number bare:

```
foobar19_0017.jpg, foobar19_0018+0019.jpg, foobar19_0020.jpg
```

Today `foobar19_0018+0019.jpg` is not read as a spread and is prefixed instead
(`017_foobar19_0018+0019.jpg`). Read it as the spread it is.

### The rule

In `spreadOf`, the second number may be written either as today, with the
frame's text before and after it, or without the frame's leading text: the
digits followed by the frame's trailing text. The first number keeps the full
frame.

- `foobar19_0018+0019.jpg` → `018+019.jpg`
- `foobar19_0018-0019.jpg` → `018-019.jpg`
- `p0018+0019.jpg` → `018+019.jpg`
- `p0018+p0019.jpg` → `018+019.jpg`, as before

Only the digits after the frame's leading text are read as a number, so the
`19` in `foobar19_` is never taken for one. Everything else in the page rule
stays as it is: the spread's numbers count toward the width and toward "the
same value twice", and a name after the spread takes its last number. Three or
more joined pages stay out of scope.

Update the `renumber.ts` header comment and the `spreadOf` doc comment, which
describe a spread as two numbers each with the frame's text around them.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `renumber.test.ts`: `foobar19_0017.jpg, foobar19_0018+0019.jpg,
      foobar19_0020.jpg` become `017.jpg, 018+019.jpg, 020.jpg`
- [x] `renumber.test.ts`: the same with `-` as the joint, and `p0018+0019.jpg`
      among `p`-framed pages, become `018-019.jpg` and `018+019.jpg`
- [x] `renumber.test.ts`: a spread with both numbers framed is numbered as
      before (existing tests pass unchanged)
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] On the agent's Library, headless Chromium with a one-off Playwright
      script that drops a folder the way `07-freeze.spec.ts` `dropFolderOnto`
      does (`.tmp/agent/v368-drop.mjs` is a working example): dropping
      `foobar19_0001.jpg … foobar19_0017.jpg, foobar19_0018+0019.jpg,
      foobar19_0020.jpg` with "Number the pages" ticked lands `001.jpg …
      017.jpg, 018+019.jpg, 020.jpg`
