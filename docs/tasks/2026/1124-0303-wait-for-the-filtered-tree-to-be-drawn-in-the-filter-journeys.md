---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: null
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && make e2e-it"
assignee: null
branch: task/1124-0303-wait-for-the-filtered-tree-to-be-drawn-in-the-filter-journeys
created_at: 2026-11-23T18:03:18Z
updated_at: 2026-11-23T18:37:32Z
---

# test(e2e): wait for the filtered tree to be drawn before reading its rows

## Overview

The journey `frontend/packages/apps/e2e/journeys/17-filter-kept.spec.ts` fails intermittently in the `e2e` CI job, on `main` and on many branches. It always fails at line 73, `await expect.poll(() => drawn(page)).toEqual(rows)`, after the reload: the rows read before the reload are only `folder:runs` and `folder:runs/album-000`, while the tree after the reload also draws the two books Recent kept under the album (for example `folder:runs/album-000/read-on-a-kept` and `folder:runs/album-000/read-on-c-kept`). The rows after the reload are right; the ones read before it are read too early.

The cause, as read from the code (confirm it, and state the confirmed cause in the PR description; if it turns out to be different, fix the actual cause and say so):

- Before the reload, the journey waits for the `/api/filter` response with `recent=2` and for the filter's status line, then reads the drawn rows. Waiting for the response does not wait for the page to draw it, and the status line already matched `ANSWERED` from the answer for the name alone.
- Once the answer for both is drawn, `answeredTree` (`frontend/packages/apps/web/src/filter.ts`) opens every folder above a kept row, here `runs/album-000`, which was kept closed under the name alone. The tree then asks for that folder's listing, and until it arrives `treeRows` (`frontend/packages/apps/web/src/tree.ts`) draws a `note` row (`loading…`, no `data-key`) in place of the folder's rows. So the rows read in that window hold neither the kept books nor anything that says they are missing.

Fix the journey so that the rows it compares are read only once the tree is drawn from the answer it waited for: for example, take the rows of the `recent=2` answer from its response body, wait until every kept row (`folder:<path>` / `file:<path>`, the Library root aside) is drawn and no `loading…` row is left in the tree, and only then read the status line and the rows. Keep what the journey asserts after the reload as it is.

Then apply the rule "a journey that reads the filtered tree's rows or line waits until the tree is drawn from the answer it means, with the listings of the folders that answer opened" to every journey that breaks it, not only this one. Look through `13-filter`, `14-filter-read-on`, `15-filter-over-reader`, `16-filter-reanswer` and the rest of `frontend/packages/apps/e2e/journeys/` for a read of the drawn rows, or of the status line, right after waiting for a response or for a status that an earlier answer already satisfied. Where a helper serves more than one journey, put it in `journey.ts`.

Do not change product code unless the confirmed cause is in it. This changes nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] `make e2e-it` passes, `17-filter-kept` included

### Before merge (verified outside the check command)

- [x] The PR description states the confirmed cause of the flake
- [x] Every journey that reads the filtered tree's rows or line after waiting for an answer waits for the tree drawn from that answer
