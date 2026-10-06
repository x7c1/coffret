---
status: awaiting_review
pipeline_phase: work
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "/api/unlock" backend/crates/apps/coffret-server/src/router.rs && grep -rq "unlock" frontend/packages/gateway/api/src && grep -q "fn unlock" backend/crates/apps/coffret-server/src/lock/custody.rs && grep -rq "fn unlock" backend/crates/apps/coffret-server/src/state.rs && grep -q "unlock" docs/guides/environments.md && grep -rq "Unlock" backend/crates/apps/coffret-desktop/src/tray.rs'
assignee: null
branch: task/1006-1932-unlock-a-locked-library-in-place-from-the-explorer-and-the-shell
run_log: null
created_at: 2026-10-06T19:32:33Z
updated_at: 2026-10-06T19:35:31Z
---

# feat: unlock a locked Library in place from the explorer and the desktop shell

## Overview

A server locks its Library after the idle interval (spec: DK-4) and today has
no way back: `lock_when_idle` returns once it has locked, the explorer's
refusals say to start the server again with the Passphrase, and the desktop
shell's second launch only reopens the explorer. The person quits the app and
starts it again every time they come back after half an hour. DK-1 names the
two states and says the correct Passphrase moves locked to unlocked; nothing in
the spec requires a new process for that. This change adds the in-place unlock,
with the Passphrase typed only in the shell's own window — never in the
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

Spec: no rule changes. DK-1 covers the state transition, DK-4 the re-armed idle
lock, DK-10 the Passphrase entry (the shell's window is a prompt that does not
echo), LA-2/LA-3/LA-6 the route and the key. If you find a rule that forbids an
in-place unlock, stop and report `needs_review` rather than editing the spec.

Out of scope: reconnecting an expired Drive grant (next task), mapping folders
from the explorer, the explicit lock (dropped on purpose; do not reintroduce a
*Lock now* item).

## Acceptance criteria

### Automated (pipeline-verified)
- [ ] `Custody` can be refilled and `ServerState::unlock` exists; a routes test locks a server, unlocks it in place, serves a keyed request afterwards, and sees the idle lock fire again after the interval
- [ ] `POST /api/unlock` is a keyed route with tests for its three answers (already unlocked; locked with a prompt → `202` and the prompt woken; locked without a prompt → the restart sentence), and it is in the explorer's API contract and the `make deps` route list
- [ ] The explorer reloads on a `locked` → `unlocked` transition and shows the locked line with an *unlock* button, with unit tests for the transition rule and the status bar
- [ ] The shell's `unlock` window has an unlock mode driven by the server's prompt and by the tray's *Unlock…* item, and `coffret-desktop` builds and passes clippy under `make check`
- [ ] The two restart-only sentences and the guide's desktop section no longer say a restart is the only way
- [ ] `make check` passes

### Before merge (verified outside the check command)
- [ ] Under `make dev` for the development Library: with `COFFRET_IDLE_MINUTES=1`, wait for the lock, press *unlock* in the explorer, and see the sentence saying the server has to be started again (no shell there) — an agent runs this with a person's help for the Passphrase at start, or reports the exact lines otherwise
- [ ] Needs a person: under `make desktop-dev` with `COFFRET_IDLE_MINUTES=1`, after the lock the explorer's *unlock* brings the app's window forward, the Passphrase unlocks the Library in place, the explorer's listing and reader come back without a manual reload, and the tray's *Unlock…* does the same

## Last subagent report

I stopped before editing anything because the spec has a rule that forbids an in-place unlock. The task says to report `needs_review` in that case rather than change the spec. The working tree has no changes.

The conflict is in `docs/spec/device-key-custody/README.md`, in DK-1's own sub-bullet, inside the Rules section:

> "The same holds for a server: stopping it ends its hold on the keys, which is why there is no lock to ask a running one for — its only way back to unlocked is starting it again with the Passphrase, which stopping it already asks for."

So DK-1 says a server can only get back to unlocked by being started again. `ServerState::unlock` and `POST /api/unlock` would add a second way back. The task's premise that "nothing in the spec requires a new process" overlooks this sub-bullet.

Two other documents say the same thing. They are not rules, but would need to change with it:
- `docs/concepts/passphrase/README.md` (about line 31): "a locked server is unlocked by starting it again".
- The server crate's docs (`coffret-server/src/lib.rs` "There is no route back", `lock/mod.rs` "What it does not do") present a restart as the only design.

Suggested path: amend DK-1's sub-bullet so a server holding the server lock may be unlocked in place, with the Passphrase taken only from the shell's own non-echoing window and never through a page (DK-10, LA-3, LA-6). Update the concept doc to match, then re-run this task unchanged.
