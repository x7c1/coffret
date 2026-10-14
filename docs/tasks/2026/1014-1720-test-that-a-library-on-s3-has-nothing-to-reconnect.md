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
branch: task/1014-1720-test-that-a-library-on-s3-has-nothing-to-reconnect
created_at: 2026-10-14T08:11:46Z
updated_at: 2026-10-14T08:29:31Z
---

# test(server): show that a Library on S3 has nothing to reconnect

## Overview

`POST /api/reconnect` refuses with a 409 (`ApiError::nothing_to_reconnect`) when the open
Library's Storage grant is not one a consent page can renew — an S3 Library
(`coffret-server/src/reconnect/run.rs`, `ApiError::nothing_to_reconnect`).
No route test covers it: the route tests' consent seam
(`tests/routes/support/consent.rs`, `ScriptedConsent`) never looks at the
Library's grant, and the served test Library is set up in a way that does not
let a case say "this Library is on S3". The explorer shows the reconnect button
only for a Drive Library, so the refusal is reached only by a direct call.

Add a route test in `tests/routes/reconnect.rs` that serves a Library whose
grant is S3's, posts `/api/reconnect`, and asserts the 409 the server sends
(`ApiError::nothing_to_reconnect`: a `bad_request` with no `reason`, saying there is
nothing to reconnect), that no consent flow was started
(`served.consent.asked() == 0` or the equivalent), and that `/api/work`
reports no reconnect. Extend the test support only as far as the case needs
(for example a constructor or option on `Served` that gives the Library an S3
grant), following how the support already builds its Library; do not change
the server's behaviour.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A route test shows `POST /api/reconnect` on an S3 Library answers the 409
      `bad_request` that says there is nothing to reconnect, starts no consent flow, and leaves the work
      answer's reconnect empty
- [x] `make check` passes
