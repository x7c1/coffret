---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, user-experience, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -qE "\"(locked|unlocked)\"" backend/crates/apps/coffret-server/src/routes/activity.rs && grep -q "unlocked" frontend/packages/gateway/api/src/activity.ts && test "$(sed -n "/^- \*\*DK-4/,/^- \*\*DK-5/p" docs/spec/device-key-custody/README.md | grep -c "^  - ")" -ge 2 && grep -qi "activity" docs/concepts/master-key/README.md && grep -rqli "unlocked" frontend/packages/apps/web/src/ --include="*.test.ts" && grep -q "fn steady_polling_for_activity_does_not_keep_the_library_unlocked" backend/crates/apps/coffret-server/tests/routes.rs'
assignee: null
branch: task/0914-1150-let-a-tab-left-open-learn-that-the-idle-lock-landed
created_at: 2026-09-14T11:50:59Z
updated_at: 2026-09-14T12:40:20Z
---

# feat(explorer): let a tab left open learn that the idle lock landed

## Overview

When a person presses the lock control, `frontend/packages/apps/web/src/lock.ts`
gives up the pages this device decrypted and asks the screen's questions again,
so nothing in plaintext outlives the key (spec: DK-3). The idle lock has no
such moment: `lock_when_idle` (`backend/crates/apps/coffret-server/src/lock/`)
empties the custody cell on the server's own clock (spec: DK-4) and tells no
browser. A tab left open in the reader keeps the decrypted page on the screen
and in memory, and meets the lock only at the next page turn. `lock.ts` states
this limitation in its header; this task removes it.

The road already exists. While the reader is open, `useActivity`
(`frontend/packages/apps/web/src/useActivity.ts`) asks `GET /api/activity`
every `ACTIVITY_INTERVAL_MS`, and that route answers while the server is
locked: it needs no key, and asking it is not activity (spec: DK-4, and the
router test `steady_polling_for_activity_does_not_keep_the_library_unlocked`).

Server side (`backend/crates/apps/coffret-server/src/routes/activity.rs`):
add to `ActivityDto` one field stating the Library's state on this device, in
the two words DK-1 uses — `locked` or `unlocked`. Read it off the custody cell
without taking a `KeyHandle`: it must record no presence and defer no lock,
so the polling test above stays as it is. Add a router test in
`backend/crates/apps/coffret-server/tests/routes.rs` next to the idle tests:
before the interval passes the activity answer says `unlocked`; once it has
passed, the same route still answers `200` and says `locked`; and the explicit
lock route produces the same answer afterwards.

API side (`frontend/packages/gateway/api/src/activity.ts`): carry the field
on `Activity` with the same two literals, documented as what DK-1 calls the
two states and read only from this route.

Explorer side: when an activity answer says `locked` after the previous answer
this page held said `unlocked`, do what the explicit lock does after the
server answers — discard the decrypted pages, then reload the Library, the
folders and the listing — without asking the server to lock. Put the decision
in a pure function beside `shouldPoll` / `shouldAsk` in `fill.ts` or in
`lock.ts`, and unit-test it: a first answer that already says `locked`
discards nothing (the page came up to a shut Library and holds no plaintext);
`unlocked` followed by `locked` discards once; `locked` followed by `locked`
does not discard again; the answer that follows a lock this tab asked for does
not discard a second time, or, if it does, the test shows the second discard is
harmless to the reader (`ReaderView` reads `discarded` as one instruction to
let go). Wire it in `App.tsx` where `useActivity` and the explicit lock meet.
Rewrite the header of `lock.ts` and the comment above `discarded` in `App.tsx`
so they no longer say the idle interval says nothing to the browser; say what
it now says and how the screen hears it.

Register and concept: add to DK-4 in `docs/spec/device-key-custody/README.md`
one sub-bullet, *(Form: test)*, stating that a device that has locked itself
says so when asked what it is doing, so that a window left open can give up
what it decrypted, and that asking is not activity. Add to the lock bullet in
`docs/concepts/master-key/README.md` the clause that an explorer learns of the
lock from the server's answer about what it is doing and gives up what it
decrypted. Do not change the meaning of any other rule; the idle interval
itself stays a policy parameter and is not made configurable in seconds.

No end-to-end journey is added: the idle interval's floor is one minute
(`COFFRET_IDLE_MINUTES`), too long for a journey to wait on. The router test
drives the interval with the paused clock the existing idle tests use, and the
unit tests cover the screen's rule.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The activity answer carries the Library's state as `locked` or
      `unlocked`, and a router test shows it turning from `unlocked` to
      `locked` when the idle interval passes with nothing asked, while the
      route still answers `200`.
- [x] Reading that state records no presence:
      `steady_polling_for_activity_does_not_keep_the_library_unlocked` is
      unchanged and passes.
- [x] `Activity` in the API package carries the same two literals, and a unit
      test in the explorer covers the rule: no discard on a first `locked`
      answer, one discard on `unlocked` → `locked`, none on `locked` →
      `locked`.
- [x] DK-4 gains a sub-bullet about the answer a locked device gives, and the
      Master Key concept's lock bullet names the explorer's use of it.
- [x] `make check` passes with the new tests.
