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
branch: task/1015-0401-open-the-explorer-from-the-desktop-app-with-a-one-time-launch-ticket
created_at: 2026-10-14T19:01:00Z
updated_at: 2026-10-15T00:16:19Z
---

# feat(desktop): open the explorer from the desktop app with a one-time launch ticket

## Overview

This is the last of four changes on the `feat/explorer-client-auth`
integration branch. The explorer host now admits `/api` only with a browser
token. A page gets one by exchanging a one-time launch ticket at
`POST /auth/launch`. The host issues tickets only through an in-process call
(see the explorer-host rules in `docs/spec/loopback-access/`). The page
already reads a ticket from `#launch=` and exchanges it. The development host
already writes launch files the same way; reuse its writer rather than adding
a second one. This change makes the desktop app hand the browser a ticket in
a way only the owning account can read, after which the desktop app works end
to end again.

1. **Where the app opens the explorer.** Today it opens
   `http://127.0.0.1:<port>` in the system browser (`coffret-desktop/src/explorer.rs`)
   after an unlock and from the tray. Every such place now issues a ticket and
   opens a **launch file** instead.
2. **The launch file.** An HTML document whose only content is
   `<meta http-equiv="refresh" content="0; url=http://127.0.0.1:<port>/#launch=<ticket>">`
   plus a fallback link to the same URL. No script and no external resource.
   Write it as an exclusive temporary file (0600) in an owner-only directory
   (0700, created if missing; refuse to use one that is not owned by this
   account or is wider than 0700), then rename it into place. Open its
   `file:` URL with the system browser. The browser's command line then
   carries the path and never the ticket.
3. **Where the file lives.** On Linux, under `$XDG_RUNTIME_DIR` (a per-user
   0700 tmpfs that the session removes at logout) in a directory named for the
   app's identifier, so the production and development apps do not share one.
   Where `$XDG_RUNTIME_DIR` is unset, and on macOS, use a directory under the
   app's state directory. The target browsers are Chromium-based, installed
   from a deb on Linux or as the macOS app. Snap-packaged browsers, which may
   not read hidden directories, are not supported. Say so in the desktop
   guide.
4. **Clean-up.** Remove the file once its ticket has been exchanged or has
   expired (60 seconds), and remove any stale launch files at start. A failed
   removal is logged by kind only and does not matter for safety, since a used
   or expired ticket admits nobody.
5. **Tray.** "Open explorer" issues a fresh ticket and opens a fresh file, so
   a person can always get a working tab, including after the 12-hour expiry or
   from a bookmark. Add "Disconnect all browser tabs", which revokes every
   session (`revoke_all`). Name the items in the tray's existing voice.
6. **Lifetimes.** Host and server already live and die together in the
   desktop app. Make sure a server restart, a Library change and app quit
   retire every session and ticket. Make sure an idle lock keeps sessions, and
   that the host refuses data with `423` while locked, as the spec says.
   Unlocking from the page still asks the app for the Passphrase window. It
   never takes a Passphrase itself.
7. **Events.** Ticket issued, file written, file removed, and refusals, by
   kind only. Never the ticket, the URL or the file's path (EL-1).

Update `docs/guides/environments.md` (the desktop-app section) and any desktop
guide to say how the explorer is opened now, and that a tab without a token
should be reopened from the tray.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test writes a launch file and asserts directory 0700, file 0600,
      exclusive creation, and content that is a refresh to the host's fixed
      origin with the ticket in the fragment and nothing else executable
- [x] A test asserts the command the app would hand to the opener contains
      the file's path and no part of the ticket
- [x] A test asserts a directory owned by another account, or wider than
      0700, is refused rather than used
- [x] Tests: the file is removed after the exchange and after expiry; stale
      files are removed at start; a failed removal does not re-enable the
      ticket
- [x] Tests: retire on server restart and Library change; sessions survive an
      idle lock, and data is refused with `423` until unlock
- [x] "Disconnect all browser tabs" revokes every session (test of the
      handler the tray item calls)
- [x] A test captures the app's events around opening and finds no sentinel
      ticket, URL or path
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] On this Linux machine, with Chrome installed from its deb, the
      development app (`make desktop-dev`) on a development Library opens a
      tab that lands on the explorer with the fragment gone from the address
      bar, lists the Library, opens a book, and takes a drop. The launch file
      has been removed afterwards
- [x] Opening the same launch file a second time (copied aside before it was
      removed) shows the "open from the tray" message, and the tray's "Open
      explorer" then gives a working tab
- [x] After an idle lock the tab shows the locked state, unlocking through the
      app's Passphrase window resumes it, and "Disconnect all browser tabs"
      turns every open tab back to the "open from the tray" message
- [x] macOS is not checked in this change; the PR says so
