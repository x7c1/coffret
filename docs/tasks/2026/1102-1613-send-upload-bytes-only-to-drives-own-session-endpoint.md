---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1102-1613-send-upload-bytes-only-to-drives-own-session-endpoint
created_at: 2026-11-02T07:13:00Z
updated_at: 2026-11-02T08:03:40Z
---

# fix: send upload bytes only to Drive's own session endpoint

## Overview

A Drive upload opens a resumable session with a POST to
`https://www.googleapis.com/upload/drive/v3/files`, and Drive answers with a
`Location` header naming where the bytes go. The gateway takes that string as
it comes (`backend/crates/gateway/google-drive-store/src/upload.rs`,
`open_session`) and PUTs the body there with `Authorization: Bearer <token>`
(`send_bytes`). Nothing checks the scheme, host, port or path first, so an
answer that names another origin, plain HTTP, or a loopback port receives the
token and the encrypted body. With a stub transport, all three of
`https://collector.invalid/upload`, `http://collector.invalid/upload` and
`http://127.0.0.1:49123/upload` received the Bearer token.

Changing that header needs the session answer from Drive's HTTPS API to be
replaced (a misbehaving or compromised intermediary, for instance), so this
is hardening: such a fault must not carry the token anywhere else.

The session URLs real Drive answers with have the origin
`https://www.googleapis.com`, no explicit port, and the path
`/upload/drive/v3/files`, with the session in an `upload_id` query
parameter.

## Decided rules

- A session URL is accepted only when all of these hold:
  - the scheme is `https`;
  - the host and port are those of the upload endpoint
    (`Endpoints::upload`, today `www.googleapis.com` on the default port);
  - the path is the upload endpoint's path (`/upload/drive/v3/files`);
  - there is no userinfo and no fragment;
  - the query carries a non-empty `upload_id`.
- Derive the allowed origin and path from `Endpoints`, so that where the
  gateway talks to stays stated once. Do not add a second hard-coded copy.
- A URL that breaks a rule is refused before any token is attached and
  before the body's reader is read. The refusal is a malformed answer from
  Storage, not a transport failure. It is not retried and does not read as
  "the account is out of space" or similar.
- The upload PUT does not follow HTTP redirects. A 3xx answer to it is a
  failure, classified like any other unexpected answer. Calls other than the
  upload PUT keep their current redirect behaviour.
- Neither the refusal message nor the logs carry the whole URL or its query.
  The scheme, host, port and path, or which rule failed, are enough.

## Changes

- **Session type.** Represent an accepted upload session as a type that can
  only be built by the check above, and have `send_bytes` take that type
  instead of `&str`.
- **Redirects.** Make the upload PUT go through a path that does not follow
  redirects (`reqwest::redirect::Policy::none()` on the client that sends it,
  or an equivalent). Keep the change local to the gateway's transport. The
  stub transport must be able to answer a 3xx, so the test below can drive it.
- **Regression tests.** Write the cases below as refusals: each bad session
  URL is refused before the token or the body leaves.
- **Docs.** If the gateway's module docs or a spec item describe where
  uploads go, state the rule there. Check `docs/spec/` for the Drive gateway
  and the logging rules (EL-*) before adding new text.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A real-shaped session URL
      (`https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&upload_id=…`)
      still uploads. The existing upload, digest and streaming tests pass
      unchanged or only adapted to the new type.
- [x] Each of these session URLs is refused, the transport records no
      request carrying `Authorization`, and the body's reader is never
      polled:
      `https://collector.invalid/upload`,
      `http://collector.invalid/upload`,
      `http://127.0.0.1:49123/upload`,
      `http://www.googleapis.com/upload/drive/v3/files?upload_id=x`,
      `https://www.googleapis.com:8443/upload/drive/v3/files?upload_id=x`,
      `https://user:pw@www.googleapis.com/upload/drive/v3/files?upload_id=x`,
      `https://www.googleapis.com.evil.example/upload/drive/v3/files?upload_id=x`,
      `https://www.googleapis.com/upload/drive/v3/files?upload_id=x#frag`,
      `https://www.googleapis.com/drive/v3/files?upload_id=x`,
      `https://www.googleapis.com/upload/drive/v3/files` (no `upload_id`),
      and a value that is not a URL.
- [x] A 3xx answer to the upload PUT is a failure, and no second request is
      sent.
- [x] A test shows that the refusal's message and its log events do not
      contain the session's query.
- [x] `make check` passes.
