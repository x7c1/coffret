---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/explorer-client-auth
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1015-0359-admit-only-authenticated-requests-through-the-explorer-host
created_at: 2026-10-14T18:59:00Z
updated_at: 2026-10-14T21:51:09Z
---

# fix(explorer-host): admit only authenticated requests through the explorer host

## Overview

This is the second of four changes on the `feat/explorer-client-auth`
integration branch. The first wrote the explorer host's admission rules into
`docs/spec/loopback-access/` and added the state that holds launch tickets and
browser sessions. This change puts that state in front of every request the
host serves. Read the new LA rules first; they are the contract, and this
task's Overview only points at where they land.

Today `coffret-explorer-host/src/lib.rs` forwards all of `/api` with no
admission. `forward/headers.rs` builds "its own origin" from the incoming Host
and rewrites Host and Origin to the server's. It removes the caller's key and
attaches the one it read from the key file, so any caller that reaches the
port is served as the owner.

1. **Fixed authority.** The router is built from the address the listener
   actually bound. Change the host's construction so that a caller cannot get
   a router without one. Then change the desktop app
   (`coffret-desktop/src/unlock/open_library.rs`, `serve`) to bind first and
   pass the bound address. Remove every place the host derives an origin from
   a request.
2. **One admission layer for `/api/**`,** unknown paths included, in the order
   the spec gives: Host → Origin / `Sec-Fetch-Site` → bearer token →
   generation and lock → forward. Map the state's outcomes to `403
   origin_refused`, `401 authentication_required`, `423 library_locked` and
   `429` with `Retry-After`, with fixed bodies that name no credential.
   Nothing before the forward step reads the key file or opens a connection
   upstream.
3. **`/auth`, handled by the host itself:** `POST /auth/launch`,
   `GET /auth/session`, `POST /auth/session/revoke`, and a fixed `404` for
   any other `/auth` path. No CORS headers, and no permissive `OPTIONS`.
   Enforce the size, header and time limits the spec gives. Refuse a large
   upload without reading its body, and close the connection.
4. **Forwarding.** Strip incoming `Authorization`, `Cookie`, the server-key
   header and `Forwarded*`. Attach the key and the fixed upstream Host. Rewrite
   Origin only when it was verified as the fixed origin. Do not follow
   redirects, and drop `Set-Cookie` from answers. When the key cannot be read
   or has changed, fail closed and retire every session. Never re-read a
   newer key file on behalf of a token issued under the old one.
5. **Lock.** The host needs to know whether the Library is locked and when the
   server's generation changes. Take that from the server's existing public
   state (`coffret-server/src/lock/`) rather than inventing a second source.
   The server's own key custody still enforces the lock; the host's `423` is
   for the page's sake and is not the only guard.
6. **Page and asset responses.** A CSP of at least `default-src 'self'`,
   `connect-src 'self'`, `img-src 'self' blob:`, `object-src 'none'`,
   `base-uri 'none'`, `frame-ancestors 'none'`, `form-action 'none'`, with
   any style exceptions the bundled explorer needs listed explicitly, and no
   `unsafe-eval`. Also `Referrer-Policy: no-referrer`,
   `X-Content-Type-Options: nosniff`, and `Cache-Control: no-store` on every
   `/auth` and `/api` answer. Serve only the embedded `dist`: no traversal,
   no dotfiles, nothing through a symlink.
7. **Events.** Issue, exchange, refusal and revocation are logged by kind,
   fixed reason and counts only, under EL-1 as the first change extended it.

The page and the development setup do not send tokens yet; a later change on
this branch does. So the desktop app's explorer stops loading data after this
change until then. That is expected on the integration branch. CI's `e2e` job
reaches the server through Vite, not through this host, and stays green.

### Regression cases to port

These cases reproduced the hole against the real host router and the server's
real `authorize` module, with the server's business handlers replaced by a
fixed GET and POST and nothing sending a key. Before the fix they all answered
`200` except the cross-site control. Port them as tests of the real router,
expecting refusal:

| Request | GET | POST |
|---|---|---|
| loopback Host, same Origin, `same-origin` | 401 | 401 |
| loopback Host, other Origin, `cross-site` | 403 | 403 |
| Host `evil.invalid:5173`, matching Origin, `same-origin` | 403 | 403 |
| Host `evil.invalid:5173`, matching Origin, no Fetch Metadata | 403 | 403 |
| loopback Host, no Origin and no Fetch Metadata | 401 | 401 |

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The five cases above are tests of the real router and get the statuses
      shown, with zero upstream connections and zero key-file reads (count
      them with test doubles)
- [x] Unauthenticated GET, POST, multipart upload and an unknown `/api` path
      get `401`, and the upload body is not read
- [x] Missing, duplicated, `localhost`, other-port and other-name Host values,
      a differing absolute-form authority, and `Forwarded` /
      `X-Forwarded-Host` attempts are refused
- [x] `null` Origin, multiple Origins, `same-site` and `cross-site` Fetch
      Metadata, a form POST and an `OPTIONS` preflight are refused, and no
      answer carries `Access-Control-Allow-*`
- [x] A forged Origin and Fetch Metadata without a token, as a native client
      would send, are refused
- [x] The happy path: an issued ticket exchanges at `/auth/launch`, then GET
      and POST with the token reach the upstream double, and the upstream sees
      the server key and not the browser token
- [x] A used, expired or post-restart ticket gets `401`; concurrent exchanges
      of one ticket yield one token
- [x] A token from another generation, and the server key shown as a bearer
      token, are refused; when the key file changes, existing tokens stop
      working
- [x] While locked, data routes get `423`, while `/auth/session` and the
      unlock request pass
- [x] Rate and size limits answer `429` and the documented refusals
- [x] Asset serving refuses traversal, dotfiles and symlinks out of `dist`,
      and pages carry the CSP and the other headers above
- [x] A test captures events across the success and refusal paths and finds
      no sentinel ticket, token, digest, `Authorization` value, query or path
- [x] The desktop app builds and its tests pass with the bind-first order
- [x] `make check` passes
