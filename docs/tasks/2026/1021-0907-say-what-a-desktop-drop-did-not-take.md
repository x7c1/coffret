---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rqi "import" docs/concepts/library/README.md && grep -rqi "drop handle" docs/concepts/library/README.md'
assignee: null
branch: task/1021-0907-say-what-a-desktop-drop-did-not-take
created_at: 2026-10-21T00:07:37Z
updated_at: 2026-10-21T01:13:42Z
---

# feat(explorer): say what a desktop drop did not take, and name the import in the concepts

## Overview

The desktop app now takes a drop as paths. The app walks dropped folders on the Rust side, holds the result under a one-time drop handle, and the explorer imports it through `POST /api/import` (spec LA-26–LA-29). That change left four things behind. This change finishes them.

### Decisions (already made; implement them)

1. **Show what a partly taken drop did not take.** Today `heldBy` in `frontend/packages/apps/web/src/drop.ts` sends the not-taken items of a partly taken drop only to the console. A person who drops three photos and a symbolic link sees three rows in the confirmation and never learns the link was left out. LA-26 says these are "reported, not silently dropped".
   - Carry the not-taken rows from `heldBy` to the confirmation. `DropConfirm` shows one line built from `notTakenLine` (for example "1 thing in the drop was not taken: <name> is a link"). Do this both when the drop goes straight in and when it asks for the Pack choice.
   - Show the same line as a notice after a drop that needed no confirmation.
   - The browser path never has not-taken rows and shows nothing new.
2. **A drop where nothing was taken.** Today it reads "nothing was added — that drop could not be read: <reason>". The prefix comes from `App.tsx`'s `unreadable` (around line 912), which was written for a failed browser walk and does not fit "a name coffret keeps for itself" or "is a link".
   - Give the desktop case its own error type from `heldBy`. `App` shows "nothing was added — <notTakenLine>" for it, and keeps the existing sentence for a browser walk that failed.
3. **The import's events name their own operation.** `outran_as` in `backend/crates/apps/coffret-server/src/routes/upload/outran.rs` writes `operation = "upload"` for every caller, so a refused import logs as an upload. Pass the operation in, so import refusals log `operation = "import"`. Pin it with a test.
4. **Concept docs.** Bring the concept docs up to date with what the import does (`docs/concepts/`):
   - **Entry**, Domain Rules: "A dropped file's times are its own or absent" says a dropped file's Entry records no birth time. Split it. A browser's drop records none; an import by path records the person's file's own birth time where its filesystem reports one, never the copy's (FM-9, EP-11).
   - **Library**, Domain Rules: the added-file sub-bullet "The device keeps a note of the drop so that the run carrying the file in records no birth time" becomes "keeps the birth time to record — none, or the person's file's".
   - **Library**, Collocations:
     - add **import**: the files a drop onto the desktop app's window let go of, walked on this device by their paths, held under a one-time **drop handle**, and placed into a mapped folder for a later run to carry in, as a browser's drop is (spec: LA-26–LA-29);
     - extend **drop**, **add** and **stamp** to cover the import;
     - make **take** say its object, since a drop is taken as well as the turn at pending work. Keep the drop handle's lifetime and cap in LA-26 and only cite it.
   - Keep the copies' wording consistent with the spec's (LA-26: "takes a drop", "held").

### Out of scope

- Progress for a large import (the adding row shows a count only).
- Changing the handle's lifetime or cap.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Web vitest: a desktop drop with one not-taken item among taken ones shows the not-taken line in the confirmation. It shows the same line in the notice when no confirmation is needed. A browser drop shows no such line.
- [x] Web vitest: a desktop drop where nothing was taken shows "nothing was added — " followed by the not-taken line, not "could not be read". A failed browser walk still shows "could not be read".
- [x] A server test shows a refused import (e.g. over the file count) logs `operation = "import"`, and a refused upload still logs `operation = "upload"`.
- [x] `docs/concepts/library/README.md` names the import and the drop handle (gated). The Entry and Library Domain Rules no longer say a dropped file never records a birth time.
- [x] `make spec-citations` passes with the concept docs' new citations.

### Before merge (verified outside the check command)

- [ ] On macOS, the Electron app over the agent Library, driven with Playwright's `_electron` and a real-path CDP drag:
  - a drop of a JPEG with a symbolic link beside it shows the not-taken line in the confirmation or notice and lands the JPEG;
  - a drop of only a symbolic link shows "nothing was added — …".
  The merging session records this in the PR.
