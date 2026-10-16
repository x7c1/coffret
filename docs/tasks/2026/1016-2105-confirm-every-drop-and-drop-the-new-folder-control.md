---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 2
retries_remaining: 1
check_command: "make check && ! grep -rqF --exclude='*.test.ts' --exclude='*.test.tsx' -- '+ new folder' frontend/packages/apps/web/src"
assignee: null
branch: task/1016-2105-confirm-every-drop-and-drop-the-new-folder-control
created_at: 2026-10-16T12:05:11Z
updated_at: 2026-10-16T13:43:56Z
---

# feat(explorer): confirm where every drop goes, accept drops on tree rows, and remove "+ new folder"

## Overview

The explorer has a "+ new folder" control that makes a folder on the screen
only (`frontend/packages/apps/web/src/newFolder.ts`): a Library has no empty
folders (EP-2), so the folder lives in the tab until something is dropped into
it, and is gone on reload. That is a place to lose work and a concept nobody
needs once the drop itself says where things go.

Replace it with a confirmation on every drop:

- **Files and folders alike are confirmed before anything is sent.** Today only
  a drop holding a folder is asked about (Pack / one by one / Cancel,
  `packChoice.ts`, `PackConfirm.tsx`), and a drop of loose files starts at
  once. Every drop now opens one confirmation that states:
  - the destination: the full folder path the files will land under, written
    as the Library will hold it, and marking the levels that do not exist in
    the Library yet (for example dropping folder `Vol 3` onto `comics/Foo`
    lands under `comics/Foo/Vol 3/`, and `Vol 3` is new);
  - for a drop holding a folder, the existing Pack / one by one choice and its
    counts and sizes, and the renumbering options it already has;
  - for loose files, that they are added one by one, with the count and size;
  - what the drop replaces, as it already says today;
  - Add / Cancel. Cancel sends nothing.
- **Drop onto a tree row.** A folder row in the left tree is a drop target with
  the same feedback the list gives while a drag is over it (the drag-over
  line, the refusal where no mapping reaches the folder), and the drop lands in
  that row's folder through the same confirmation.
- **Remove "+ new folder"** and the screen-only folder machinery it alone
  needs (`newFolder.ts` and its use). Keep what still applies: folders of a
  freeze that has not committed must still show in the tree until it commits
  (`strandedFolders`) — move that rule to where it now belongs rather than
  dropping it.
- Update the e2e journeys (`frontend/packages/apps/e2e/journeys/`), notably
  `03-drop.spec.ts` and `07-freeze.spec.ts`, to answer the confirmation, and
  any that use "new folder".

Out of scope: choosing or editing the destination inside the confirmation,
renaming, moving.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A drop of loose files opens the confirmation and sends nothing until
      Add; Cancel sends nothing (component / unit tests).
- [x] The confirmation states the destination path and marks the levels that
      do not exist yet; a unit test covers a drop of a folder onto a nested
      existing folder and onto the root.
- [x] A drop holding a folder still offers Pack / one by one with counts and
      sizes, and the renumbering options (tests carried over).
- [x] A tree row accepts a drop and shows the same drag-over line and refusal
      as the list (component tests).
- [x] No "new folder" control remains in the tree, list or explorer shell (a
      grep appended to the check), and folders of an uncommitted freeze still
      show in the tree (test).

### Before merge (verified outside the check command)

- [ ] In Playwright's Chromium against the agent's Library: dropping a folder
      onto a tree row and dropping loose files onto the list each show the
      confirmation with the right destination, and Add commits them.
- [ ] The confirmation reads clearly — needs a person: a UX judgement.
