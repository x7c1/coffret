---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [user-experience, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "no name of anything the drop was" backend/crates/apps/coffret-server/src/routes/upload/outran.rs && grep -q "outran_as" backend/crates/apps/coffret-server/src/routes/upload/receive.rs && grep -q "says_which_file_was_over_it" backend/crates/apps/coffret-server/tests/routes.rs && ! grep -q "(refused: unknown) => setNotice(said(refused))" frontend/packages/apps/web/src/App.tsx && grep -rq "landed" frontend/packages/apps/web/src/dropped.test.ts'
assignee: null
branch: task/0913-1611-let-a-drop-that-broke-say-what-it-left-behind
created_at: 2026-09-13T16:11:43Z
updated_at: 2026-09-13T16:57:22Z
---

# fix(frontend): let a drop that broke say what it left behind

## Overview

A drop of many files is one request, and it can be refused **after** some of
those files are already on disk. The gateway says so in as many words
(`frontend/packages/gateway/api/src/upload.ts`, `addFiles`'s doc):

> The others are not about where it was going but about what it costs: a budget
> of the server's that the drop passed … Any of those may be met after part of
> the drop has landed, and then those parts are in the folder with nothing armed
> to carry them in.

and it warns, in the paragraph that follows, that the answer may never be read at all:

> All of them are answered while the browser may still be sending the body, so a
> transfer that fails before the answer is read is thrown out as `unreachable`
> rather than as the refusal: `unreachable` out of this function is not proof the
> server is gone.

**The screen honours neither sentence.** `App.tsx`'s drop handler
(`App.tsx:358`–`380`) reloads the listing on the resolved path and, on the
rejected path, does one thing:

```
(refused: unknown) => setNotice(said(refused)),
```

So a drop that breaks mid-transfer puts `the coffret server did not answer` in
the notice area — the sentence `request.ts:55` mints for *every* non-abort
`fetch` rejection — and never asks the listing again. The files that did land
are on disk, in the folder, absent from the screen, with nothing armed to carry
them in. A person is told the server is gone, sees no new rows, and drops the
same files again.

Three defects share that one story, and a fourth is the sentence that
caused one of them.

### 1. `unreachable`'s sentence is asserted as fact

`asked` (`request.ts:49`–`57`) turns any non-abort `fetch` rejection into
`Refusal('unreachable', 0, 'the coffret server did not answer')`. For a `GET`
that is fair: nothing was sent, nothing came back. For an upload the browser was
still streaming, it is a guess, and the gateway's own doc says it is a guess.

The screen is the one place that knows which request it made. `said(refused)`
(`useRemote.ts:73`) renders a refusal's message verbatim and is right to — the
message is the server's own — but nothing reaching it from a broken upload came
from the server at all.

Say what is known instead of what is guessed: the transfer did not finish, so
what became of the drop is not known here, and the folder is the thing to look
at. The sentence must not claim the server is absent, and must not claim the
files were refused.

### 2. Nothing re-asks the listing when a drop is refused

The resolved path calls `reloadListing()` unconditionally — correct, because
even an answer full of refusals may carry `written` — and `activity.follow()`
when `written` is non-empty. The rejected path calls neither.

Both belong on the rejected path too, for the same reason the gateway states:
files may be in the folder. The listing is what puts them on the screen; the
activity is what would show a sync if one were armed. (On the budget paths
nothing *is* armed — the server says so: "leaving what landed before it where a
passed budget leaves it: in the folder, with nothing armed" — which is itself
worth the person knowing, and is a second reason the rows must appear: they are
the only evidence the drop left anything.)

**This has to be unit-testable, and `App.tsx` is not.** There is no `App` test
in `frontend/packages/apps/web/src/` and this change should not invent one.
Follow what `drawn.ts` and `lock.ts` did for the reader: put the decision — given
an outcome, what does the screen show, and what does it re-ask — in a module of
its own with the browser left out, and let `App` hold only the wiring. Name it
`dropped.ts`, with `dropped.test.ts` beside it.

The module decides for all three outcomes, not two: an answer with `written`, an
answer with `refused`, and a rejection. The third is the one this change exists
for, and the one a test must pin.

### 3. A file over the per-file budget is refused without being named

`receive.rs:83`–`87` stops the request when one part would pass
`envelope.part_bytes`, with:

> one file in it is over that on its own, so dropping fewer beside it changes
> nothing

The sentence is true and useless: a person who dropped three hundred scans is
told one of them is too large and left to find it. The name is in hand at that
line — `incoming` is the part being read — and the refusal is person-facing.

**The reason it is withheld is a misreading of the register.** `outran.rs`'s doc
says:

> What goes in is the sentence and nothing else — no name of anything the drop
> was carrying (spec: EL-1).

EL-1 (`docs/spec/event-logging/README.md:19`–`27`) says the opposite of what
that sentence takes it to say. Its first half governs the **event**: a
diagnostic event must not contain a filename. Its second half is explicit about
the other side:

> A person-facing refusal may identify a file, a Library, or a mapping that
> person owns … that rendering is not reused for an event.

So the rule is *two* renderings, not one silence. `outran` mints both the log
event and the `ApiError` from a single sentence, which is what fused them. Split
them: the event keeps exactly what it has today, and the answer names the file.

Check every other person-facing refusal this module mints while the rule is in
front of you — the parts budget at `mod.rs:233` is about the request's shape and
names nothing, which is correct, because there is no one file to name.

### 4. The doc that caused it

Whatever shape the fix takes, `outran.rs`'s doc must stop stating EL-1's first
half as though it were the whole of EL-1. It is the sentence that made a
person-facing refusal withhold something the register permits, and it will do so
again to the next reader.

## Out of scope

- **The browser still waits for a page's whole blob before showing any of it.**
  That is how the reader works today rather than something this change broke,
  and giving it up means streaming into the reader — a feature, not a fix, and
  not this story.
- `frontend/packages/apps/e2e/journeys/08-lock.spec.ts` asserts the lock from
  the listing but never opens the reader first, so the journey does not cover
  what a person loses when the key goes. It belongs with the lock rather than
  with the drop, and `make check` does not run the journeys, so an assertion
  added here would go unrun.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A drop whose request is rejected re-asks the listing, so files that landed
      before the refusal appear on the screen; a test drives the rejected
      outcome and fails if the reload is dropped.
- [x] The decision about what a drop's outcome means lives in
      `frontend/packages/apps/web/src/dropped.ts` with tests beside it, and
      covers all three outcomes — written, refused, and a rejection — rather
      than only the two that resolve.
- [x] A drop that breaks mid-transfer is not reported with a sentence claiming
      the server did not answer, and not with one claiming the files were
      refused.
- [x] A file over the per-file budget is refused by a person-facing sentence
      that names it, asserted by a test named
      `a_part_past_the_part_budget_says_which_file_was_over_it`.
- [x] The diagnostic event for that refusal still carries no filename (spec:
      EL-1), asserted in the same test.
- [x] `outran`'s documentation no longer states EL-1's log half as the whole
      rule, and says which of the two renderings each of its outputs is.

### Manual / on-hardware (verified by a human before merge)

- [ ] Drop a folder of images large enough that one of them passes the per-file
      budget, and confirm from the browser that the sentence names that file and
      that the rows which landed before it are on the screen without a manual
      refresh.
