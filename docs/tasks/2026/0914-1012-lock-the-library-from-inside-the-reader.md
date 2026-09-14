---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && grep -q "getByRole(.img." frontend/packages/apps/e2e/journeys/08-lock.spec.ts && ls .tmp/e2e/screenshots/08-lock | grep -qi reader && git diff --quiet origin/main -- backend frontend/packages/apps/web/src'
assignee: null
branch: task/0914-1012-lock-the-library-from-inside-the-reader
created_at: 2026-09-14T10:12:02Z
updated_at: 2026-09-14T10:50:24Z
---

# test(e2e): lock the Library from inside the reader

## Overview

The lock journey, `frontend/packages/apps/e2e/journeys/08-lock.spec.ts`,
presses the lock control from a folder's list of rows and asserts that the rows
go and the server's sentence about the Passphrase (spec: DK-2) takes their
place. The case a person is most likely to be in when they lock — a page of a
book open in the reader — is covered only by unit tests
(`frontend/packages/apps/web/src/lock.test.ts`, `drawn.test.ts`) and by a
one-off check at a real browser, not by the journeys. The unit tests prove the
order of discard and reload; what they cannot see is a real Chromium showing a
decrypted page, and whether that page is gone from the screen once the lock
answers.

Make the lock journey lock from inside the reader. Walk to the book the way
`01-browse-and-read.spec.ts` does (the tree helpers in `journey.ts`), open a
page so that `getByRole('img', { name: bookPage(n) })` is visible, move to the
next page, and press the lock control — it stands in the status bar, which
stays on the screen while the reader is open. Then assert what a person would
see: no page image remains (`toHaveCount(0)` on the image role), the
Passphrase sentence is visible, the rows of the folder are gone, and the
Library's name is still shown in the status bar. Photograph the reader before
and after the lock with `shot`, naming the checkpoints so that the folder
`.tmp/e2e/screenshots/08-lock/` lists them in order and at least one carries
`reader` in its name.

Keep the journey's existing assertions — the rows going, the sentence, the
name surviving — so the list-side reading of the lock is still proven, either
before opening the reader or after it comes off the screen. Keep the journey
last and one-way: nothing in it starts the server again, and no other journey
is added or reordered, so the count of journeys the script
`scripts/e2e-it.sh` states stays true. Update the file's header comment so
it describes the journey that now runs; do not describe the reader case as
covered only by hand anywhere it is no longer true.

Do not change product code to make the journey pass. If the screen does not
behave as the assertions above expect, that is a finding for the report, not a
reason to edit `App.tsx`, `ReaderView.tsx` or `lock.ts`. The idle lock
(spec: DK-4) is not part of this journey; it says nothing to the browser
today, and the journey must not wait on it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `08-lock.spec.ts` opens a page in the reader (an `img` located by role
      with a book page's name) before pressing the lock control, and `make
      e2e-it` runs the journey green against MinIO in Chromium.
- [x] After the lock, the journey asserts that no page image remains, that the
      Passphrase sentence is visible, that the folder's rows are gone, and that
      the Library's name is still shown.
- [x] The journey photographs the reader around the lock: a checkpoint whose
      name contains `reader` exists under `.tmp/e2e/screenshots/08-lock/` after
      the run.
- [x] Nothing under `backend/` or `frontend/packages/apps/web/src/` differs
      from `origin/main`: the journey observes the explorer as it is.
