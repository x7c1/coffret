---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqF "This tab" frontend/packages/apps/web/src/ && ! grep -rqF "This tab’s session" frontend/packages/apps/web/src/'
assignee: null
branch: task/1106-0842-keep-the-desktop-explorers-session-from-ending-under-it
created_at: 2026-11-05T23:42:33Z
updated_at: 2026-11-06T00:27:53Z
---

# feat(desktop): keep the explorer's session from ending under it

## Overview

The desktop app loads the explorer into its own window from a one-time launch
ticket (spec: LA-17, LA-21; `frontend/packages/apps/desktop/src/main.ts`, the
call that issues a ticket with `addon.issueTicket()` and loads
`explorerLaunchUrl(address, ticket)`). The page exchanges the ticket for a
browser token that expires 12 hours after it was issued, and nothing refreshes
it (LA-18). Today a person who keeps the app resident sees, after 12 hours, a
page saying the session ended, and has to use *Reload the explorer*. When the
server restarts or the Library served changes, every session is discarded
(LA-18) and the page says the same. In the desktop app that page has no reason
to exist: the app can always give its own window a fresh ticket.

Change the desktop app and the explorer so that the person never sees a
session end in the desktop window:

1. **Before a token expires, the desktop app hands the page a fresh ticket, and
   the page exchanges it and swaps its token in place, without loading the page
   again.** The page keeps what it shows (the tree, open folders, the reader,
   the details pane). The new token is a newly issued one; the old token's
   expiry is never extended (a token still lives at most 12 hours). The old
   session may be revoked by the page with its own token once the new one is
   held (LA-16 `POST /auth/session/revoke`), so the cap of 8 sessions is not
   filled by one window. Tickets are still issued only by a call within the
   process the host runs in (LA-17): no HTTP request and no page can ask for
   one. The way the ticket reaches the page is a one-way delivery from the main
   process into the explorer window (for example a message the existing
   preload, `frontend/packages/apps/desktop/src/preload.cts`, relays to a
   listener the page registers), sent by the main process on its own timer
   ahead of the expiry; the page gets no call that makes the main process issue
   a ticket. The design choice and its reason go in the PR body.
2. **When the session ends anyway** — the server restarted, the Library served
   changed, or the swap failed — the desktop app loads the explorer window
   afresh from a new ticket on its own, as *Reload the explorer* does today,
   instead of leaving the page that says the session ended. It must not loop:
   where a fresh ticket cannot be issued (no Library open, the Library locked
   and the Passphrase window in front, the app quitting), it does what it does
   today for that state.
3. **Rewrite LA-18 and LA-21** (`docs/spec/loopback-access/README.md`) to
   state this behaviour: who renews a session and how (a new token from a new
   ticket, never an extension), that the page swaps the token without
   reloading, and that the desktop app reopens its window when a session ends.
   Keep their *(Form: …)* notes true and add tests for the parts marked
   `test`. Update any guide that describes the ended-session page in the desktop
   app (`docs/guides/environments.md`, "The desktop app" and "The explorer in
   development").
4. **The page that says a session ended or was never held stays, for the pages
   outside the desktop app** (a bookmark, an address typed by hand, the
   development host in a system browser). Keep it minimal: plain text and the
   one thing it offers (reload the explorer from the app, or open the launch
   file). Reword it so it does not say "tab": the explorer is a window in the
   desktop app. `NO_SESSION` and `ENDED` in
   `frontend/packages/apps/web/src/Explorer.tsx` are the strings today.
5. **Rename "tab" where it means the explorer's page or window**, in code
   comments, test names, and documents under `docs/` and `frontend/`. Keep
   "tab" where it means a real browser tab, such as Google's consent page and
   the page shown after authorization, or a browser tab the development host's
   page is opened in. Grep for `tab` (word) across `frontend/packages/` and
   `docs/` (excluding `docs/tasks/`) and decide each site by this rule.

Out of scope: the development host's launch file and its behaviour (LA-21's
file part), which keep working as they do; a page in a system browser still
shows the minimal ended page and the person opens the launch file again.

States the swap can meet, each to be covered by a test:

- the Library is unlocked: the swap happens and the page keeps working;
- the Library is locked: the swap still happens (a locked Library keeps its
  sessions, LA-18) and the page keeps showing the locked state;
- the explorer window is hidden (closed to the tray): the swap still happens,
  or the window is reloaded afresh when it is shown again — either is fine,
  but the page the person sees on showing it holds a session;
- the window was destroyed: nothing is delivered, and a later show loads it
  from a fresh ticket as today;
- the exchange of the delivered ticket is refused (`401`, `429`): the page
  reports it, and the desktop app reloads the window from a fresh ticket.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A desktop unit test shows the main process delivers a fresh ticket to the
      explorer window ahead of the token's expiry, on its own timer, and that
      no channel the page can call issues a ticket.
- [x] A web unit test shows the page, given a delivered ticket, exchanges it,
      holds the new token for every later request, keeps it on its shelf in
      place of the old one, and does not reload or remount the explorer.
- [x] A desktop unit test shows that when the page reports its session ended
      (server restarted, Library changed, swap refused), the main process loads
      the explorer window afresh from a new ticket, and does not do so where no
      ticket can be issued.
- [x] LA-18 and LA-21 in `docs/spec/loopback-access/README.md` describe the
      renewal by a new token and the desktop app reopening its window, and
      `make check` (which runs the spec-citation and rule-ID checks) passes.
- [x] No string the explorer's page shows says "This tab" (the two `grep`
      gates appended to `check_command`).
- [x] `make check` passes.
