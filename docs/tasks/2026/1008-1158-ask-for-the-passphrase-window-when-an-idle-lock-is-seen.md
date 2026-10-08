---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1008-1158-ask-for-the-passphrase-window-when-an-idle-lock-is-seen
created_at: 2026-10-08T02:58:08Z
updated_at: 2026-10-08T03:33:33Z
---

# feat(explorer): ask for the Passphrase window as soon as an idle lock is seen, without waiting for a press

## Overview

After an idle lock the explorer shows the locked sentence and an *unlock*
button; only pressing it asks the server (`POST /api/unlock`,
`frontend/packages/gateway/api/src/unlock.ts`, `askForUnlock` in
`frontend/packages/apps/web/src/lock.ts`) to have the desktop app bring its
Passphrase window forward. A person who comes back to the explorer has to
notice the sentence and press the button before the window appears.

Have the explorer ask on its own, under these rules:

1. **When.** When the page sees the Library locked (the work answer's
   `library` turning to `locked`, `lockLanded` in `lock.ts`, or a first answer
   of `locked` once the page is in front), it calls the same unlock request the
   button calls — but only while the page is visible and has focus
   (`document.visibilityState === 'visible'` and `document.hasFocus()`). An idle
   lock happens while the person is away, so asking at that moment would push
   the window in front of whatever they are doing elsewhere. If the page is not
   visible or not focused, ask the next time it becomes visible and focused
   (`visibilitychange` / `focus`).
2. **Once per lock.** Ask at most once for each lock. If the person closes the
   Passphrase window without unlocking, do not ask again for that lock; the
   *unlock* button and the tray's *Unlock…* stay available. A new lock (after
   an unlock) allows one new automatic ask.
3. **Only where it can work.** When the server answers that this run has no
   prompt to bring forward (a server started without the desktop app, e.g.
   `make dev`), do not ask automatically at all for the rest of the page's
   life: the answer would only be the "start the server again" sentence. Use
   whatever the server already reports to tell these apart; if it reports
   nothing before the first ask, treat that refusal as the signal and stop.
4. **Same path.** The automatic ask is the same request the button sends; the
   Passphrase never touches the page (spec: DK-1, LA-3, LA-6). The button's
   state (asking / refused sentence) reflects an automatic ask the same way it
   reflects a press.

Keep the decision logic DOM-free and unit-tested in `lock.ts` (or a sibling
module) the way the existing lock helpers are; wire it in the component that
owns the locked state.

5. **A page that does not poll still notices.** A page with the reader closed and
   no work running does not ask `/api/work` while it sits in front (DK-4), so a
   lock that lands while it stays in front is first seen when a folder listing,
   tree or reader request is refused with kind `locked`. On such a refusal, ask
   `/api/work` once so the page learns the Library is locked; the rules above
   then bring the window forward and show the *unlock* button.

Out of scope: any change to the server or the desktop app.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] Seeing a lock while the page is visible and focused asks for the unlock once, with a unit test
- [x] Seeing a lock while the page is hidden or unfocused asks nothing until it becomes visible and focused, then asks once, with a unit test
- [x] After a declined or dismissed ask, the same lock asks nothing more; a later lock asks again, with a unit test
- [x] A server with no prompt to bring forward is not asked automatically after its first refusal, with a unit test
- [x] A request refused with kind `locked` while the page believes the Library unlocked makes the page re-read its state once, with a test
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Needs a person: under `make desktop-dev`, with a short idle interval, coming back to the explorer after the lock brings the Passphrase window forward without pressing *unlock*; closing it does not bring it back until the next lock
