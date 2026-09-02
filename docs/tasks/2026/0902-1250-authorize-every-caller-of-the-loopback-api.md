---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rqi 'authoriz' backend/crates/apps/coffret-server/src && grep -rq 'Sec-Fetch\\|sec-fetch' backend/crates/apps/coffret-server/src"
assignee: null
branch: task/0902-1250-authorize-every-caller-of-the-loopback-api
created_at: 2026-09-02T03:50:31Z
updated_at: 2026-09-02T06:50:00Z
---

# feat(server): authorize every caller of the loopback API

## Overview

`coffret-server` binds `127.0.0.1` and treats everyone who can reach the
socket as the device's owner. That equation is wrong in one important case:
the owner's own browser runs other people's pages, and any of them may send
requests to a loopback port — cross-origin `fetch` (whose response they
cannot read, but whose side effects still happen), form/navigation-shaped
POSTs, or a DNS-rebound hostname that resolves to `127.0.0.1`. Browser-side
protections (Private Network Access) are still shipping across browsers and
are not this product's to rely on. Today none of the eleven routes checks
who is asking, and they collectively hand out Entry Paths and plaintext
bytes, start Storage traffic, and write attacker-supplied bytes into the
mapped folders.

Make the server authorize every request itself:

1. **A per-startup capability.** On startup the server draws a
   high-entropy secret and writes it to a `0600` sidecar file in the
   Library's state directory (next to the other per-Library files). Every
   route — reads and mutations alike, `/api/library` through `/api/upload`
   — requires the secret in a **custom request header**; a request without
   it is refused before any handler logic runs, with a body that says how
   the legitimate UI obtains it. A web page in a browser cannot read local
   files and cannot set custom headers via forms or `no-cors` requests, so
   the header requirement alone shuts down navigation CSRF and blind
   cross-origin fetch; the file location keeps the secret off the URL, out
   of `Referer`, and out of shell history. The secret must never appear in
   a query string, a log line, or an error body — extend the existing
   log-hygiene discipline to it.
2. **Host allowlist against DNS rebinding.** Reject any request whose
   `Host` is not the loopback authority the server bound (host or
   host:port form), before the capability check. A rebound hostname
   carries the attacker's domain in `Host` and fails this test even though
   the TCP connection reaches the socket.
3. **Browser-metadata second fence.** When `Origin` or `Sec-Fetch-Site`
   headers are present and assert a cross-site browser context, refuse —
   the legitimate callers are the same-machine UI (whose dev/preview
   server proxies `/api` and strips/sets these) and same-machine tools.
   This is defense in depth behind the capability, not a replacement.
4. **Getting the secret to the legitimate callers.** The web UI reaches
   the API through the vite `/api` proxy (dev and E2E alike), so the proxy
   is the right place to attach the header: it runs on the device, can
   read the sidecar file, and the browser never holds the secret. Wire the
   vite config (dev + preview as used by the E2E) to read the sidecar and
   inject the header on proxied requests. The E2E environment plumbing
   passes the state directory already; extend it as needed.
5. **Tests.** Router tests: a request without the header is refused (one
   representative read and one mutation); a wrong secret is refused; a
   correct secret passes; a foreign `Host` is refused even with the
   correct secret; a cross-site `Origin` / `Sec-Fetch-Site` is refused; a
   refusal body never echoes the expected secret. E2E: the existing seven
   journeys keep passing with the proxy injecting the header, which is
   itself the proof that the legitimate path still works end to end.

Update the server's crate-level docs: the "whoever can reach the socket"
paragraph is the thing this task retires — the boundary is now the
capability, with the OS file permissions deciding who can read it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Every route under `/api/` requires the startup capability header;
      router tests cover missing/wrong/correct secret on a read and a
      mutation, and the refusal body does not contain the secret (the
      `grep -rqi 'authoriz'` gate on the server crate matches nothing
      today).
- [x] A request with a non-loopback `Host` is refused before the
      capability check, and a request asserting a cross-site browser
      context via `Origin` / `Sec-Fetch-Site` is refused (the
      `Sec-Fetch` gate matches nothing today).
- [x] The secret lives in a `0600` sidecar file in the Library state
      directory, is never logged and never accepted via query string; a
      test asserts the file mode and a log-capture test asserts no leak.
- [x] The E2E suite passes with the vite proxy injecting the header — the
      seven existing journeys run unchanged from the user's point of view.

## Out of scope

- TLS on the loopback socket, and multi-user OS accounts (the file mode is
  the boundary; a same-account process reading the sidecar is the device
  owner by definition here).
- Authentication of a future remote or multi-device UI.
- Rate limiting and request/response resource budgets.
- The CLI, which talks to Storage directly and does not call this API.
