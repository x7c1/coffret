---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "/api/unlock" backend/crates/apps/coffret-server/src/router.rs && grep -rq "unlock" frontend/packages/gateway/api/src && grep -q "fn unlock" backend/crates/apps/coffret-server/src/lock/custody.rs && grep -rq "fn unlock" backend/crates/apps/coffret-server/src/state.rs && grep -rq "Unlock" backend/crates/apps/coffret-desktop/src/tray.rs && ! grep -q "only way back to unlocked is starting it again" docs/spec/device-key-custody/README.md && grep -q "desktop app" docs/concepts/passphrase/README.md && ! grep -rq "has no way back to unlocked" backend/crates/apps/coffret-server/src'
assignee: null
branch: task/1007-0359-unlock-a-locked-library-in-place-from-the-desktop-app
created_at: 2026-10-07T03:59:00Z
updated_at: 2026-10-07T04:36:00Z
---

# feat: unlock a locked Library in place from the explorer and the desktop app

## Overview

A server locks its Library after the idle interval (spec: DK-4) and today has
no way back: `lock_when_idle` returns once it has locked, the explorer's
refusals say to start the server again with the Passphrase, and the desktop
shell's second launch only reopens the explorer. The person quits the app and
starts it again every time they come back after half an hour. DK-1 names the
two states and says the correct Passphrase moves locked to unlocked, though
its sub-bullet still says a server's only way back is a restart (amended below,
section 6). This change adds the in-place unlock,
with the Passphrase typed only in the desktop app's own window — never in the
explorer's page (the server key stays off the page too, spec: LA-3, LA-6).

**1. `ServerState::unlock(OpenLibrary)` and an idle lock that re-arms.**
`Custody` (`coffret-server/src/lock/custody.rs`) gains the inverse of `lock`:
put an `OpenLibrary` back into the empty cell (refuse, or replace, if it is not
empty — decide and document; replacing while another unlock is in flight must
not leave two sets of keys alive). `lock_when_idle` currently returns after
locking because "this server has no way back"; make the idle lock an arming the
server does on every unlock — at `launch` and again after `unlock` — so a Library
unlocked in place locks again after the interval (spec: DK-4). The `idle.rs`
`seen` / `taken` / `released` accounting starts afresh from the unlock. Route
tests: a locked server answers `unlock` by holding the Library again, a request
after that is served, and the idle lock fires again after the interval (the
existing idle-lock test shows how time is driven).

**2. The server asks the shell for the Passphrase; a server without a shell says so.**
Add to `Launch` an optional *unlock prompt*: something the shell hands in that
the server can ask to put the Passphrase window in front (`tokio::sync::mpsc`
or a `watch`, your choice — the shell side only needs to be woken). A new keyed
route `POST /api/unlock` (admitted like every route, spec: LA-2) does one of:
when the Library is unlocked, answer that it is; when locked and a prompt is
present, wake the prompt and answer `202` with a sentence saying the app is
asking for the Passphrase; when locked and no prompt is present (the CLI's
server under `make dev`), answer a refusal whose sentence says the Passphrase
can only be given by starting the server again — the same words the CLI prints
today, so a person on `make dev` is not told about a window that does not
exist. The Passphrase itself never travels over this route. Add the route to the
API contract the explorer reads (`frontend/packages/gateway/api`) and to the
list `make deps` checks against the router.

**3. The explorer shows the locked state and offers the unlock.**
The explorer already polls `/api/work` and knows `library: locked | unlocked`
(`lock.ts`, `lockLanded`). Add the other direction: when a `locked` answer is
followed by `unlocked`, reload the listing and whatever the reader had, the way
the explicit lock's discard does in reverse, so a page that was refused comes
back without a manual "try again". Show the locked state once, plainly, in the
status bar — "the Library is locked" with one button, *unlock* — in place of
the retry buttons that today linger after a lock (`StatusBar.tsx`); the button
calls `POST /api/unlock` and shows the sentence the server answered with. In a
`make dev` session that sentence tells the person to start the server again,
which is still true there. Unit tests for the transition rule (`unlocked` after
`locked` reloads; a first `unlocked` does not) and for the status bar's locked
line, in the style of the existing `lock.test.ts` and `StatusBar.test.tsx`.

**4. The shell answers the prompt and gains an *Unlock* tray item.**
In `coffret-desktop`, the shell passes its prompt into `Launch` when it opens a
Library, and a task on its runtime waits on it: when woken it brings the
`unlock` window forward (`unlock::bring_forward`) in *unlock* mode — the
Library name fixed to the one being served, the password field empty, the
button reading *Unlock*. Submitting it reopens the Library with the Passphrase
(`open_library` the device crate's way, same closure shape as at launch) and
calls `ServerState::unlock`; a wrong Passphrase shows the server's sentence and
the window stays; success hides the window. The tray gains *Unlock…* above
*Open the explorer*, which does the same without waiting for the explorer to
ask. Keep the window's page one HTML file: the mode is a flag the shell sets on
the page (an initialization script or a query on the window URL), not a second
page. The window's *Open* mode (choose a Library, start the server) is unchanged.

**5. Words.** The server's startup line "start it again with the Passphrase to
unlock it" and the explorer's locked refusal (`frontend/packages/gateway/api/src/refusal.ts`)
are the two places that still say a restart is the only way; make them say what
is true for the process at hand (the CLI: start again; the app: unlock from the
app). `docs/guides/environments.md`'s desktop section says "quit from the tray
and start the app again" — replace that with the unlock.

**6. The spec and the concept say the same as the code.** DK-1's sub-bullet in
`docs/spec/device-key-custody/README.md` says of a server that "its only way back
to unlocked is starting it again with the Passphrase". That is what this change
makes untrue, so this change amends it. Rewrite
that sentence so it says: stopping a server still ends its hold on the keys; a
running server that has locked may be unlocked in place, but only with the
Passphrase taken from a prompt that does not echo and is not a page (DK-10, and
the server key and secrets never reach a page, LA-3, LA-6) — which is what the
desktop app's own window is; a server started from the command line has no such
prompt and is unlocked by starting it again. Keep the rule's *(Form: …)* note
true (add a test reference for the in-place unlock if the form says test).
In `docs/concepts/passphrase/README.md`, the sub-bullet "There is no route back
through a browser: a locked server is unlocked by starting it again, because a
Passphrase typed into a page would be a Passphrase carried through one (spec:
DK-2)" keeps its point — no route back through a page — and changes its
conclusion, along these lines: "There is no route back through a browser: a
locked server takes the Passphrase again from a prompt of the desktop app's own,
never from a page, because a Passphrase typed into a page would be a Passphrase
carried through one. A server started from the command line has no such prompt,
so it is unlocked by starting it again (spec: DK-1, DK-2)." Call the thing "the
desktop app" in prose, as `docs/guides/environments.md` does; do not call it a
"shell" in these two documents. The server crate's docs that say there is no
way back (`coffret-server/src/lib.rs` "There is no route back", `lock/mod.rs`
"What it does not do", `lock_when_idle.rs` "this server has no way back to
unlocked") are corrected in the same pass. `make spec-citations` and the
spec-rule-id checks must pass.

Spec: DK-1's sub-bullet is amended as above; no other rule changes. DK-4 covers
the re-armed idle lock, DK-10 the Passphrase entry, LA-2/LA-3/LA-6 the route and
the key.

Out of scope: the explicit lock (dropped on purpose; do not reintroduce a
*Lock now* item). Reconnecting a Drive grant and mapping folders from the
explorer are already in; leave them as they are.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `Custody` can be refilled and `ServerState::unlock` exists; a routes test locks a server, unlocks it in place, serves a keyed request afterwards, and sees the idle lock fire again after the interval
- [x] `POST /api/unlock` is a keyed route with tests for its three answers (already unlocked; locked with a prompt → `202` and the prompt woken; locked without a prompt → the restart sentence), and it is in the explorer's API contract and the `make deps` route list
- [x] The explorer reloads on a `locked` → `unlocked` transition and shows the locked line with an *unlock* button, with unit tests for the transition rule and the status bar
- [x] The shell's `unlock` window has an unlock mode driven by the server's prompt and by the tray's *Unlock…* item, and `coffret-desktop` builds and passes clippy under `make check`
- [x] The two restart-only sentences and the guide's desktop section no longer say a restart is the only way
- [x] DK-1's sub-bullet and the passphrase concept say a running server is unlocked in place from the desktop app's own prompt and a command-line server by starting it again, the server crate's docs no longer say there is no way back, and the spec checks pass
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Under `make dev` for the development Library: with `COFFRET_IDLE_MINUTES=1`, wait for the lock, press *unlock* in the explorer, and see the sentence saying the server has to be started again (no shell there) — an agent runs this with a person's help for the Passphrase at start, or reports the exact lines otherwise
- [ ] Needs a person: under `make desktop-dev` with `COFFRET_IDLE_MINUTES=1`, after the lock the explorer's *unlock* brings the app's window forward, the Passphrase unlocks the Library in place, the explorer's listing and reader come back without a manual reload, and the tray's *Unlock…* does the same
