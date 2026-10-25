---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1026-1550-read-a-loopback-requests-head-before-answering-it
created_at: 2026-10-25T15:50:03Z
updated_at: 2026-10-25T16:29:16Z
---

# fix: read a loopback request's headers before answering, so closing it does not reset the connection

## Overview

The loopback server in
`backend/crates/gateway/google-drive-store/src/oauth/authorization/loopback_redirect.rs`
reads only the request line of each arrival, answers it, shuts its side down
and drops the stream. A browser sends headers after the request line, and on
Linux closing a socket whose received data was never read sends a TCP reset
instead of a clean close. The reset can reach the browser before it has
rendered the answer, so the completion page — or the answer to a
`/favicon.ico` request — can show as "connection reset". The authorization
itself is unaffected (the code has been read by then), but SA-2 says such
requests are answered, and a reset answer is not one a person can see.

1. Before answering, read the rest of the request head, up to and including
   the blank line that ends it, and discard it. Bound what is read — both the
   total bytes of the head and the time spent waiting for it — so a client
   that never sends the blank line, or sends without end, cannot hold the
   wait for the real redirect or grow memory. Pick the bounds from what a
   browser's redirect request plausibly carries and say why in a comment. A
   head past the bound, or cut off, is still answered and closed as now and
   does not end the wait; record it with `tracing` the way the answer and
   close failures already are (fields `operation`, `step`, `io_error_kind`,
   or a fixed reason; never the target, code or state — EL-1, EL-3).
2. The test `a_request_that_is_not_the_redirect_is_answered_and_closed` sends
   only the request line, with a comment explaining it avoids headers because
   of the reset. Make the clients in that test send a browser-shaped request
   (request line, a few headers such as `Host`, `User-Agent`, `Accept`, then
   the blank line) and remove that comment, so the test samples what a
   browser sends. Add a case for a head past the bound: it is answered and
   closed, and the redirect after it still yields its code.

3. A connection that sends nothing before the wait for its head runs out
   is closed without an answer: nothing was asked, and an answer written
   into a connection a browser opened ahead of need could be paired with the
   request it sends there later. SA-2 says anything else that arrives "is
   answered", so bring its text to what the code does: any other *request*
   that arrives is answered and ignored, and a connection that sends nothing
   before the server stops waiting for it is closed without an answer. Keep
   the rest of SA-2 as it is, and bring the doc comments of `wait_for_code`
   and `read_redirect` to the same words.
4. A request line that is not valid UTF-8, a target that does not parse, and
   an I/O error while reading the head currently end `wait_for_code` with an
   error, so any process on the machine that can reach the port can abort
   the authorization with one connection. That contradicts SA-2: such an
   arrival is not the redirect, so it is answered where a request was begun
   (or closed where the connection failed), recorded with `tracing` like the
   other cases (a fixed `reason` or `io_error_kind`; never the target, code
   or state), and the wait goes on. Only a failure to accept on the listener
   still ends the wait. A redirect that does parse keeps every outcome it
   has now: the code, the provider's refusal, and the refusal of a redirect
   without this run's state. Add socket tests for a request line that is
   not UTF-8 and for a target that does not parse, each followed by the
   redirect still yielding its code, and remove error variants or
   `RedirectStep` values that no longer have a producer.

Do not change any other spec rule.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A request carrying headers is answered with `200` and closed cleanly,
      and the redirect after it yields its code, in a test that sends
      browser-shaped requests over a real loopback socket.
- [x] A request head past the bound is answered and closed without ending
      the wait, and the bound is tested.
- [x] SA-2 says a connection that sends nothing is closed without an
      answer, and a test names SA-2 for it.
- [x] A request line that is not UTF-8 and a target that does not parse are
      answered and do not end the wait, in socket tests; only an accept
      failure ends it with an error.
- [x] `make check` passes.

