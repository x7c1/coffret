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
branch: task/1026-1520-answer-and-close-every-loopback-request-in-a-test
created_at: 2026-10-25T15:20:53Z
updated_at: 2026-10-25T15:34:59Z
---

# test: show that the loopback server answers and closes a request that is not the redirect

## Overview

SA-2 (`docs/spec/storage-authorization/README.md`) says that anything else that
arrives on the loopback port "is answered and ignored rather than mistaken for
the redirect". The tests in
`backend/crates/gateway/google-drive-store/src/oauth/authorization/loopback_redirect.rs`
only call `parse_redirect`, so they show the "ignored" half; nothing shows that
such a request is answered and its connection closed, which is what keeps a
browser asking for `/favicon.ico` from hanging while the flow waits on.

1. Add a test, named `// SA-2: …` like its neighbours, that binds a listener
   on `127.0.0.1:0`, runs `wait_for_code` on it, and as a client:
   - sends `GET /favicon.ico HTTP/1.1` and reads the answer to the end:
     it is a `200` response and the server closes the connection (the read
     reaches end of stream rather than timing out);
   - then sends the redirect carrying the state, and `wait_for_code` returns
     its code — the earlier request was not taken for the redirect and did
     not end the wait.
   Bound every await in the test with a timeout so a regression fails rather
   than hangs. Keep it in the existing `tests` module unless the module
   structure calls for otherwise.
2. `read_redirect` discards the results of `write_all` and `shutdown`
   with `let _ =`. Not stopping the flow there is right — a browser that
   went away must not end the wait for the real redirect — but the reason is
   not written down and the failure leaves no trace. Keep the flow going,
   and make the choice visible: say why in a comment, and record the failure
   with `tracing` (the crate already depends on it) at a level and with
   fields that follow how the crate logs elsewhere and the spec's
   event-logging rules (`docs/spec/event-logging/`; log no request target,
   code or state).

Do not change SA-2's text or any other behavior.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test named `SA-2` drives `wait_for_code` over a real loopback socket
      and shows that a request that is not the redirect is answered with
      `200` and closed, and that the redirect after it still yields its code.
- [x] The write and shutdown failures in `read_redirect` are no longer
      discarded silently, and the flow still continues past them.
- [x] `make check` passes.
