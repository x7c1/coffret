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
branch: task/1015-0400-authenticate-the-explorer-page-and-serve-development-through-the-explorer-host
created_at: 2026-10-14T19:00:00Z
updated_at: 2026-10-14T23:16:55Z
---

# feat(web): authenticate the explorer page, and serve development through the explorer host

## Overview

This is the third of four changes on the `feat/explorer-client-auth`
integration branch. The explorer host now admits `/api` only with a browser
token, which a page gets by exchanging a one-time launch ticket at
`POST /auth/launch` (see the explorer-host rules in
`docs/spec/loopback-access/`). This change does two things that have to land
together, because CI's `e2e` job exercises both: the page learns to
authenticate, and every development and test path reaches the server through
the explorer host instead of Vite's proxy.

### The page

1. **Launch.** At load, if the URL has `#launch=<ticket>`, take the ticket,
   remove the fragment at once with `history.replaceState`, and call
   `POST /auth/launch` with `Authorization: Launch <ticket>` and a JSON body.
   Store the returned token in `sessionStorage` under a key that includes
   `instance_id`. If storage throws or is unavailable, keep the token in
   memory only. Never use `localStorage`, cookies, IndexedDB, a Service
   Worker or Cache Storage.
2. **Resume.** Without a fragment, use a stored token for this `instance_id`
   only after `GET /auth/session` accepts it. Otherwise discard it.
3. **No token.** Show only a message that the explorer is opened from the
   app's tray (or, in development, from the launch file the dev host printed),
   and read no Library data.
4. **Every request.** In `frontend/packages/gateway/api/src/request.ts`, both
   the shared `fetch` and the upload `XMLHttpRequest` send
   `Authorization: Bearer <token>`, only to the page's own origin under
   `/api/` and `/auth/`. The helper refuses any other URL. `fetch` uses
   `credentials: 'omit'`, `redirect: 'error'` and `cache: 'no-store'`.
   FormData keeps its browser-set Content-Type, and upload progress keeps
   working.
5. **401 versus 423.** `401` means the session is gone: discard the token and
   show the no-token message. `423` means the Library is locked: show the
   existing locked state, and poll `/auth/session`, not `/api/work`, until it
   is unlocked. Polling must not keep the Library unlocked. Never resend a
   mutation (upload, delete, rename) automatically after either.
6. **Discarding.** On lock, revocation or expiry, drop the listing, work
   details and every object URL, and ignore answers that arrive late.
7. **Images** stay fetch → Blob → object URL (`getFile`). Do not add a token
   to any URL, `img src` or download link.
8. Add a "Disconnect this tab" control that calls `/auth/session/revoke`.
   Open external links with `noopener`.

### Development and tests

1. **Remove Vite's proxy.** Delete the `/api` proxy and the server-key reading
   from `frontend/packages/apps/web/vite.config.ts`. Vite no longer holds any
   credential or serves `/api`. HMR is dropped on purpose: the explorer host is
   the one browser entrance in every mode.
2. **A development host.** A binary that serves the explorer from
   `frontend/packages/apps/web/dist` on disk (not embedded), on a fixed or
   given loopback port. It forwards to a running server with that server's
   key, read from the key file as Vite's proxy did, with the same admission as
   the desktop app. Serving from disk must refuse traversal, dotfiles and
   symlinks out of `dist`. Keep `make deps` happy about where it sits.
3. **Launch files.** Put the launch-file writer in `coffret-explorer-host`, so
   that the desktop app can reuse it in the next change. A launch file is an
   HTML document that only refreshes to
   `http://127.0.0.1:<port>/#launch=<ticket>`, with a fallback link and no
   script. It is written as an exclusive temporary file (0600) in an
   owner-only directory (0700; refuse one owned by another account or wider),
   renamed into place, and removed once used or expired. The development host
   writes one at start and prints its path, and writes a fresh one whenever it
   retires sessions because the server's key changed. A person opens that
   file. No HTTP request, environment variable or command-line argument
   carries a ticket.
4. **`make web` / `make dev`.** Rework them around a frontend build in watch
   mode plus the development host, so that a change is picked up by reloading
   the tab. `make down` stops them. Update the comments on those targets and
   `docs/guides/environments.md`.
5. **`e2e-it`.** Serve the journeys through the development host rather than
   `vite preview`. The test reads the launch file the host wrote, the way any
   process of the owning account can, and opens it or navigates to its target.
   The journeys that kill and restart the server pick up the fresh launch file
   and open it again. Add no test-only bypass and no flag that turns
   authentication off.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Frontend unit tests: the fragment is removed and exchanged once; a
      stored token is used only after `/auth/session` accepts it; storage
      that throws falls back to memory; no token shows the message and makes
      no `/api` request
- [x] Frontend unit tests: fetch and XHR both carry the bearer token; the
      helper refuses a foreign URL; `401` discards the token; `423` shows the
      locked state and polls `/auth/session` only; no mutation is resent
- [x] Frontend unit test: late answers after lock or revocation are not
      rendered and object URLs are revoked
- [x] `vite.config.ts` reads no key file and defines no `/api` proxy (a test
      or a grep gate in `make check`)
- [x] Rust tests: the launch-file writer's modes, exclusive creation,
      refusal of a foreign or too-wide directory, removal after use and
      expiry, and content with no executable part
- [x] Rust tests: the development host refuses unauthenticated `/api`, serves
      nothing outside `dist`, and writes a fresh launch file when the server's
      key changes
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] CI's `e2e` job passes on the PR, running every journey through the
      development host
- [x] On this Linux machine, `make dev` on a development Library prints a
      launch file; opening it in Chrome (deb) lists the Library, opens a book
      and takes a drop; reloading the tab keeps working; a second tab opened
      by typing the URL shows the no-token message
