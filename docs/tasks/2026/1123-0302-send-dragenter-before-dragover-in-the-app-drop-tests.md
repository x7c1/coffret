---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: null
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && cd frontend/packages/apps/web && for i in $(seq 20); do pnpm exec vitest run src/App.test.tsx || exit 1; done"
assignee: null
branch: task/1123-0302-send-dragenter-before-dragover-in-the-app-drop-tests
created_at: 2026-11-22T18:02:40Z
updated_at: 2026-11-22T18:43:42Z
---

# test(web): send dragenter before dragover in the App drop tests

## Overview

The web test `draws the files being added as rows at once` in `frontend/packages/apps/web/src/App.test.tsx` fails intermittently: it failed on `main` in CI, and locally about once in 15 runs. The confirm dialog it waits for (`Add to albums`) does not appear, because the drop lands in another folder.

The suspected cause: the test fires `dragOver` on the `albums` row without a `dragEnter` on the tree first. `TreeTable` keeps the row under the drag in `over`, and an effect clears it whenever `drag.dragged` is false:

```ts
useEffect(() => {
  if (!drag.dragged) {
    setOver(null);
  }
}, [drag.dragged]);
```

Without a `dragenter`, `drag.dragged` stays false. When the passive effect of a recent mount of the tree is still pending at the moment the test fires `dragOver`, React flushes it while processing the `setOver('albums')` update, so `over` can end up `null` and the drop lands in the focused folder (the Library root) instead of `albums`. A real browser always fires `dragenter` before `dragover`, so the product code is not at fault; the test drives an event sequence a browser never produces.

Confirm the cause before fixing it (for example, by reproducing the failure with repeated runs and checking where the drop lands, or by reasoning from React's effect flushing), and write what you found in the PR description. If the cause turns out to be different, fix the actual cause instead and say so in the PR.

Then apply the rule "a test that drags files over a row fires `dragEnter` on the tree first, as a browser does" to every test that breaks it, not just this one. Known sites:

- `App.test.tsx`: `draws the files being added as rows at once`
- `App.test.tsx`: `adds nothing, and asks nothing more, where the step is cancelled`

Grep the web package's tests (`fireEvent.dragOver` without a preceding `fireEvent.dragEnter` in the same test) for any other site. Use the same form the neighbouring tests already use: `fireEvent.dragEnter(screen.getByRole('tree'), { dataTransfer: { types: ['Files'] } })`. Do not change product code unless the cause turns out to be in it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] `App.test.tsx` passes 20 times in a row (the loop in `check_command`)

### Before merge (verified outside the check command)

- [x] The PR description states the confirmed cause of the flake
- [x] Every web test that fires `dragOver` over a row for a file drop fires `dragEnter` on the tree first
