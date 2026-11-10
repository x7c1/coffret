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
branch: task/1111-0537-split-the-drive-permission-notice-into-lines
created_at: 2026-11-10T20:37:46Z
updated_at: 2026-11-10T20:50:31Z
---

# fix(web): put each sentence of the Drive permission notice on a line of its own

## Overview

When Google Drive's permission for this device runs out, the explorer joins
several sentences into one long line that is hard to read:

- the catch-up notice from `catchUpNote` in
  `frontend/packages/apps/web/src/refresh.ts` ("this device has not caught up
  with the Library, so what is listed may not be all of it") runs straight on
  into the reconnect notice `RAN_OUT` in
  `frontend/packages/apps/web/src/reconnect.ts` ("Google Drive's permission
  for this device ran out; it has to be renewed every seven days");
- while a consent flow waits, the sentence that says so and the link
  `NO_TAB` ("no tab opened? open the consent page") follow on the same line,
  in `frontend/packages/apps/web/src/ReconnectOffer.tsx`.

Lay these out so that each sentence starts a line of its own (a block per
sentence, not a `<br>` inside a run of text), in every place the notice is
drawn, while keeping the words, their order, the reconnect button and the
consent link as they are. Keep the notice readable at a phone width and in
both colour schemes. Do not change what the server answers or when the
notice is shown.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test renders the notice for a Library behind because the
      permission ran out, and finds the catch-up sentence and the ran-out
      sentence in separate block elements.
- [x] A unit test renders the offer while a consent flow waits, and finds the
      waiting sentence and the "no tab opened?" link in separate block
      elements, the link still pointing at the consent page.
- [x] Existing tests of the reconnect offer and the catch-up notice pass
      unchanged in what they assert about the words.
- [x] `make check` and `make e2e-it` pass.
