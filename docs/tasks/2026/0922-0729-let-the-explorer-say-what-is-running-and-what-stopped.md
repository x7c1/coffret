---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [user-experience, completeness, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0922-0729-let-the-explorer-say-what-is-running-and-what-stopped
created_at: 2026-09-21T22:29:21Z
updated_at: 2026-09-22T03:20:00Z
---

# feat(explorer): let the explorer say what is running and what stopped

## Overview

Eight places where the explorer runs work a person cannot see, or stops
work without telling them. Each was found in use. None of them is a failure
the code detects and hides — they are silences, and states a person can reach
but not leave.

The command line was given the same treatment in the change just before this
one: a run that takes minutes now says which phase it is in, and a zero that
means "nothing is set up" is told apart from a zero that means "nothing is
there". The explorer drives the same usecases. Where a question below has an
answer on the CLI side already, take that answer rather than inventing a
second one, and say in a comment that the two are the same thing.

### 1. A device fresh from `join` shows an empty Library, and says nothing

Two paths reach this. `join` does not catch the catalog up, so the first
explorer a person opens after joining has nothing to list. And when the
server's startup catch-up fails, it serves anyway, so the page shows an empty
Library with no indication that the emptiness is a failure rather than a fact.

A person cannot tell "this Library is empty" from "this device has not caught
up yet" from "catching up failed". Make the difference visible. The server
knows which of the three it is; decide where that belongs on the wire and how
the page says it. Being told to wait, or being told to try again, are both
better than being shown a lie.

### 2. Reloading the list loses sight of a running fill

`shouldPoll` never asks about activity while the reader is closed, so a person
who reloads during a fill sees a still page and no sign that anything is
happening. The activity is still running; only the page has forgotten.

### 3. A worker panic drops the next folder in the queue, silently

When a fill worker panics, the folders queued behind it are discarded. The
activity line and the retry button both still point at the folder that died,
so a person retries that one and never learns the rest were dropped. Say what
was dropped, and make retrying reach them.

### 4. A finished sync's line owns the status bar forever

A sync that completes carrying findings leaves its line in the status bar with
no way to dismiss it, and every later fill's progress is hidden behind it. A
person who has read the findings has no way to say so.

### 5. A fill that declined something looks like a fill that stopped

A fill that finishes having declined some files ends at "28/30" and its line
disappears. A fill that stopped also leaves "28/30". The two are different
things — one is done and one is not — and a person is shown the same picture.
The stopped line and the failed chip also have no dismiss.

### 6. A tab that fails to fetch activity once never asks again

If the activity request fails while the page is mounting, that tab stops
asking for activity for the rest of its life. `App.tsx`'s retry covers the
listing but not this. A person sees a page that will never again tell them
what is running, and reloading is the only way out.

### 7. A second freeze waits in the queue with nothing said about it

`FreezeActivity` carries only the most recent freeze, so a person who starts a
second one sees nothing about it until the first finishes. Decide what the
wire carries — the queue's length is the least of it — and what the page says.

### 8. Two uploads say nothing while they run

Dropping a nested folder produces no line at all until the sync commits,
because the `uploading` line is composed from the open folder's rows and a
nested drop has none there. And a freeze of several hundred pages shows a
fixed sentence with no progress; the note in the ledger says real progress
needs the upload moved to `XHR`, so find out whether that is still true before
assuming it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] a device whose catalog has not caught up is distinguishable on the wire
      from one whose Library is empty, and a test pins both
- [x] activity is polled while the reader is closed, and a test pins it
- [x] folders dropped by a panicking worker are reported, and a test pins what
      the person is told
- [x] a status bar line carrying findings can be dismissed, and a fill that
      declined something is distinguishable from one that stopped
- [x] a failed activity fetch at mount does not stop later ones

### Manual / on-hardware (verified by a human before merge)

- [ ] `make e2e-it` is green
- [ ] the explorer was opened on a device fresh from `join`, and what it said
      was true of what was happening
- [ ] a fill was watched through a reload, a drop of a nested folder was
      watched from the drop to the commit, and a second freeze was queued while
      the first ran

## Out of scope

- The vocabulary questions these areas raise — the concept document `mapping`
  does not have, the addresses of `present` / `remote` / `supersede` / `add`,
  and the word for a file that arrives from outside the Library. They are one
  docs pass and get their own change
- What the list shows about a folder this device has not mapped, and what
  clicking one of its rows does. That is its own change
- The shape of the server's error answers — the unknown route that escapes the
  one JSON form, the listing limit classified as Storage not answering, and
  `EpochActivated` reaching the page as a 502. Its own change
- Splitting `tests/routes.rs`, and the native root certificates the device
  tests read at run time
- The race between `arm()` and a supersede, which is a few instructions wide
  and self-corrects on the next click
