---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1110-0950-move-and-rename-in-the-background
created_at: 2026-11-10T09:48:27Z
updated_at: 2026-11-10T11:07:41Z
---

# feat: carry a move or a rename on as background work instead of holding the explorer until it lands

## Overview

A move of files and folders — a drag in the tree, a row's **Rename…** or
**Move to…** — is a single `POST /api/move` that answers only once all of it
is done (LA-31): it waits for this device's turn at its pending work (LA-12),
catches up to the head (CK-9), commits every pair in one Journal record
(FM-15, EP-18) and relocates this device's files (EP-10). The explorer holds
its confirmation dialog open with the button busy for that whole time, which
on a Storage service with a long round trip, or behind a sync or a freeze
holding the turn, looks as though the explorer has frozen.

Make the move background work, like an add or a freeze already is:

- **Before yes, nothing changes.** The preview (`GET /api/move`, LA-30) and
  every refusal it answers with are still shown in the confirmation before
  the person says yes, as today: an edited file, a file still uploading, a
  file the Library does not hold yet, a name already taken, the files that
  will leave this device and their bytes.
- **After yes, the dialog closes at once.** The server accepts the move as
  work of its own and answers without waiting for the turn, the catch-up, the
  commit or the relocation; that work then runs in this device's queue as
  the other pending work does, and appears in `GET /api/work` (LA-12) while
  it waits and runs, and in the finished work once it ends, with what it did
  (Entries renamed, files that followed and that left, findings) or why it
  stopped.
- **The tree shows the move at once.** The moved rows are drawn at their new
  place, marked as being moved (a state a person can tell apart, with an
  accessible name), and are no longer drawn at their old place, until the
  work ends. Once it has committed they are ordinary rows of the Library at
  the new place. If it is refused or fails before its commit — the refusals
  asked again under the turn (LA-31), a rename the plan refuses over the
  caught-up catalog, Storage unreachable — the tree puts them back where they
  were, and the Activity panel says why. A move whose record committed but
  whose files could not all be relocated is a committed move, as today.
- **Progress and outcome are in the Activity panel**, one entry per move,
  named by what was moved and where to, as an add is named by its folder.
- While a move of a set is pending, another move, rename, drop or delete that
  touches the same rows is refused up front with a reason saying a move of
  them is under way, rather than queued behind it.

Change the rules that say otherwise, and only those: LA-31 (the move is
accepted and answered before it is carried out, and how its outcome is
reported), and FM-15 / EP-10 only where they say the move happens within the
request. Add to LA-12 or LA-31 what `GET /api/work` shows for a move. Keep
what the move does unchanged: one Journal record for every pair, the same
refusals, the same relocation and discards.

Update the e2e journeys that walk a move (`09-drag-move`, `10-rename`) to
the new flow, and add a journey in which the move is held behind other work
(or a slow Storage) long enough to see the dialog close, the rows drawn as
being moved, and then settle, and one in which a move refused under the turn
puts the rows back and the Activity panel says why.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A server route test shows `POST /api/move` answers before the move
      commits (while this device's turn is held by other work), that the move
      then appears in `GET /api/work` as pending and, once done, in the
      finished work with what it did.
- [x] A server route test shows a move refused under the turn (an edited
      file found only then) commits nothing, and is reported in the finished
      work with its refusal.
- [x] Unit tests in the explorer show the moved rows drawn at the new place
      as being moved while the move is pending, ordinary once it committed,
      and back at the old place with the reason in the Activity panel when it
      was refused or failed before its commit.
- [x] The e2e journeys described above pass in `make e2e-it`, the updated
      `09-drag-move` and `10-rename` included.
- [x] LA-31 (and FM-15 / EP-10 where they say otherwise) describe the move as
      background work, and `make check` passes, the spec checks included.
