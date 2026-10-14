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
branch: task/1015-0358-specify-the-explorer-hosts-browser-credentials-and-hold-them-in-one-state
created_at: 2026-10-14T18:58:00Z
updated_at: 2026-10-14T20:46:12Z
---

# feat(explorer-host): specify the explorer host's browser credentials, and hold them in one state

## Overview

This is the first of four changes that close one hole, landed on the
`feat/explorer-client-auth` integration branch and merged to `main` together.

**The hole.** The explorer host (`coffret-explorer-host`), which the desktop
app binds on a free `127.0.0.1` port, forwards all of `/api` to the server
and attaches the server key it read from the Library's directory. It checks
neither the Host it was reached by nor who is calling. It also builds "its
own origin" from the incoming Host header (`forward/headers.rs`), so a request
whose Host and Origin agree on any name looks like the host's own page. Two
consequences have been reproduced against the real router and the server's
real `authorize` module (GET and POST both answered `200` without a key):

1. DNS rebinding. An attacker's hostname that resolves to loopback is not
   refused, and the server sees the host's own name and key.
2. Another OS account on the same machine. It cannot read the server-key file
   (LA-3), but it can call the host, which attaches the key for it. Host and
   Origin are forgeable outside a browser, so checking them alone does not
   close this path.

LA-2 and LA-3 promise a boundary that the host drops. The Vite dev proxy
(`frontend/packages/apps/web/vite.config.ts`) attaches the key in the same
way, unconditionally.

**The design.** The host admits a browser only with a **browser token**, a
credential for the explorer page alone, distinct from the server key. The
page gets that token by exchanging a one-time **launch ticket**. Only the
account that owns the Library can read the ticket, because it arrives through
a file. This is the established pattern for a local server opened in the
system browser: Jupyter Server opens a redirect file (`use_redirect_file`, on
by default) rather than a URL with a token on the browser's command line, and
refuses non-local Host headers against DNS rebinding.

1. When the app opens the explorer (at start, after an unlock, from the tray),
   the host issues a ticket through an in-process call. **No HTTP request can
   issue a ticket.** A ticket is 256 bits from the OS CSPRNG, and the host
   keeps only its SHA-256 digest.
2. The app writes an HTML file that refreshes to
   `http://127.0.0.1:<port>/#launch=<ticket>` into an owner-only directory
   (0700), as an exclusive temporary file (0600) renamed into place, and opens
   that file's `file:` URL in the system browser. The browser's argv carries a
   path, never the ticket. Another account can read argv (`ps`) but cannot read
   the file.
3. The page takes the ticket from the fragment, removes it from the URL with
   `history.replaceState`, and calls `POST /auth/launch` with
   `Authorization: Launch <ticket>`. The host consumes the ticket atomically,
   so of concurrent exchanges exactly one succeeds. It answers with a new
   256-bit browser token, `expires_in`, and a public `instance_id` drawn per
   host/server generation. Expired, used and unknown tickets get the same
   `401`.
4. The page sends `Authorization: Bearer <token>` on every `/api` request. The
   host checks it before reading the key or connecting upstream.

The person does nothing extra. A tab with no token (a bookmark, a typed URL,
an expired session) reads no Library data and says to open the explorer from
the app's tray.

**What this task does.** It writes that contract into the spec and builds the
state that holds tickets and sessions, as a module of `coffret-explorer-host`
that nothing calls yet. The next changes wire it into the router (admission
and `/auth`), into the desktop app (ticket file, tray), and into the page and
the development setup.

### Spec

Add rules to `docs/spec/loopback-access/` for the explorer host. Number them
after the existing LA rules, and give each its *(Form: …)*. Keep LA-1 to LA-12
as the contract of the server itself. Where their wording now reads wrongly,
reword it to say what the server key is and what a browser holds instead.
LA-3's "a page in a browser is not one" and LA-6's "never on a URL and never
in a cookie" are examples. The rules to state:

- **Fixed authority.** The host binds first, then builds its router around the
  `127.0.0.1:<port>` it actually bound. It never derives its own origin from
  a request's Host. It refuses a missing, duplicated or malformed Host,
  `localhost`, another port, another name, and an absolute-form authority that
  differs. It ignores `Forwarded` and `X-Forwarded-*`. It listens on IPv4
  loopback only.
- **Admission order for `/api/**`** (every path, unknown ones included, through
  one layer so that no route can be left out):
  1. Host equals the fixed authority, otherwise `403 origin_refused`.
  2. `Origin`, when present, equals the fixed origin exactly. `null` and
     multiple values are refused. `Sec-Fetch-Site`, when present, must be
     `same-origin`. These are not authentication: a non-browser client forges
     them.
  3. `Authorization: Bearer <browser token>` is valid. Missing, duplicated,
     malformed, unknown, expired and revoked tokens are all the same
     `401 authentication_required`. The comparison does not stop at the first
     differing byte.
  4. The token is of this host's current generation and Library. While the
     Library is locked, only the unlock request and minimal status pass, and
     the rest get `423 library_locked`.
  5. Only now is the request forwarded. Incoming `Authorization`, `Cookie`,
     the server-key header and `Forwarded*` are removed, and the trusted key
     and the fixed upstream Host are attached.

  Before a refusal, the host neither reads the key file nor connects
  upstream. It never forwards a browser token to the server, and the server
  never accepts one. The host never accepts the server key as a browser's
  credential. If the key cannot be read, the generation has changed or the
  server has stopped, the host fails closed and revokes every session; it
  never re-reads a newer key file on behalf of an old token. It does not
  follow redirects, does not pass `Set-Cookie` through, and does not emit a
  `Location` that carries a credential.
- **Public entrances.** `/` and the bundled assets (correct Host; framing
  forbidden). `POST /auth/launch` (Host and Origin checks, a ticket).
  `GET /auth/session` (token; answers expiry and locked/unlocked only).
  `POST /auth/session/revoke` (token; revokes its own session). `/auth` is
  handled by the host and never forwarded, and an unknown `/auth` path is a
  fixed `404`. No CORS: no `Access-Control-Allow-*` and no permissive
  `OPTIONS`. `/auth/launch` is JSON `POST` only. Malformed JSON is `400`, a
  wrong Content-Type is `415`.
- **Tickets.** Issued only in-process. One-time use. They expire 60 seconds
  after issue. At most 4 are live, and issuing past that expires the oldest
  unused one. Exchanges are rate-limited host-wide: 10 per second, burst 20,
  then `429` with `Retry-After`.
- **Sessions.** At most 8. They expire 12 hours after issue, with no refresh
  token. Every token and ticket is discarded on host or server restart and on
  a change of Library. A locked Library keeps its sessions but refuses data
  (above). Revocation: per session from the page, and all at once from the app.
- **Limits.** Auth bodies at most 1 KiB, headers at most 8 KiB, and auth reads
  cut off at 5 seconds. A large upload body is not read before it is refused,
  and the connection is closed.
- **Encoding.** Tickets and tokens are 32 bytes in unpadded base64url. Length
  and alphabet are validated. Both are held only as SHA-256 digests in host
  memory, never on disk, never as a log identifier.
- **Ticket file.** Owner-only directory 0700, file 0600, created exclusively
  and renamed into place, removed once the ticket is exchanged or has expired.
  A failure to remove the file does not make a used ticket usable.

Extend EL-1's list of credentials to name the browser token and the launch
ticket explicitly, and say that events about authentication carry only a
kind, a fixed reason and counts, never a ticket, a token, their digests,
`Authorization`, a URL query or fragment, or the ticket file's path.

Update the concepts that say the page holds no key (search `docs/concepts/`
for the wording) so that they tell the cryptographic keys and the server key
apart from the page's browser token, and say what the token lets a page do:
everything the explorer can do, for one Library, until it expires, is revoked,
or the host restarts. Do not introduce read-only scopes or roles.

### State

In `coffret-explorer-host`, add a module that owns tickets and sessions behind
one lock, as a single state machine:

- `issue_ticket()`. In-process only. Not reachable from any handler type.
- `exchange(ticket) -> token`. Atomic consume; exactly one of concurrent
  exchanges wins.
- `authenticate(token) -> Session`, `revoke(session)`, `revoke_all()`, and
  generation change (`retire()`).
- Expiry, the caps (4 tickets, 8 sessions) and the exchange rate limit.
- Secrets in types that zero on drop and have redacted `Debug`/`Display`.
  Constant-time digest comparison.
- An injectable clock so that tests can reach expiry without sleeping.

Keep HTTP out of this module. The next change maps its outcomes to statuses.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The new LA rules and the EL-1 wording say what the Overview says, each
      rule has its Form, and `spec-rule-ids` / `spec-citations` pass
- [x] The concepts no longer say the page holds no credential at all; they
      distinguish the keys from the browser token
- [x] Unit tests: a ticket exchanges once; a second exchange, an expired
      ticket, an unknown ticket, and a ticket after `retire()` all fail the
      same way
- [x] Unit test: of N concurrent exchanges of one ticket exactly one succeeds
- [x] Unit tests: the ticket cap expires the oldest unused ticket; the session
      cap refuses a new session rather than evicting one; the exchange rate
      limit refuses past its burst and recovers
- [x] Unit tests: a token authenticates until expiry, `revoke`, `revoke_all`
      or `retire()`, and not after
- [x] A test asserts the `Debug` and `Display` of every secret type contain
      no part of the secret
- [x] `make check` passes
