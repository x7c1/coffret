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
branch: task/1027-1130-edit-a-new-name-in-the-numbering-preview
created_at: 2026-10-27T02:30:47Z
updated_at: 2026-10-27T03:25:26Z
---

# feat(explorer): let a person correct a new name in the numbering preview

## Overview

When "Number the pages" or "Number the volume folders" is ticked in the
folder-drop confirmation (`frontend/packages/apps/web/src/DropConfirm.tsx`),
the preview shows each rename as `<from> → <to>`, for example
`foobar_0199.jpg → 200_foobar_0199.jpg`. The rule in `renumber.ts` cannot read
every book the way its owner meant, and today the only way out is to untick
the box and rename after the drop. Let the person click the new name in the
preview and correct it before anything is sent.

### What can be edited

1. **The new name on a rename line**, the `<to>` side, for both pages and
   volume folders. Lines shown as unchanged and the lines of a group left whole
   are not editable. Lines hidden behind "… N more …" become editable once
   "Show all" shows them.
2. **Only the last segment of the path.** The person edits a name, not where
   it lands. Editing a volume folder's name moves the pages under it with it,
   in the preview lines below it and in the paths sent.

### How it is edited

3. Clicking the new name turns it into a text input holding the current name.
   Enter or the input losing focus commits; Escape discards the edit. While the
   input is open, Escape must not reach the dialog's Escape-is-Cancel handler,
   and Enter must not press Add.
4. An edited line is marked as edited (for example `(edited)`) and offers a
   control that puts back the name the rule gave it.
5. The edited name is used everywhere the numbered name is used today: the
   paths sent on Add (`chooseAndAdd` in `packChoice.ts` renumbers again from
   `answer.numbering`, so the answer must carry the edits too), and the
   collision lines with the Library (`collisionsOf`). Keep the edits as data
   that `renumber`'s result is overridden with, kept free of DOM and React so
   it is unit testable as `renumber.ts` is.

### Names that cannot be committed

6. A name cannot be committed when it is empty, contains `/`, or equals another
   new or kept name in the same folder (among sibling files for a page, among
   sibling folders for a volume; a page's name also must not equal a sibling
   folder's name, as the rule already checks). The input stays open and the
   reason is said on that line. Add stays available and sends the last
   committed names.

### Names accepted with a warning

7. A committed name that breaks the reading order is accepted, but the line
   carries a warning, because it may be a typo. The order is the one the
   page rule keeps: the byte order (`inLibraryOrder`) of the final names must
   match the natural order of the original names (the same comparison
   `judgePages` makes for its `'order'` check; reuse it rather than writing a
   second one). Apply the same check among volume folders. Say the warning in
   the line, something like "read out of order: this name sorts before
   `<neighbour>`".

### Where the edits live

8. Edits live only in the open dialog. Ticking or unticking either box discards
   every edit and starts again from the rule's names. Nothing is remembered for
   the next drop.

### Collisions under an edited volume name

`heldUnder` lists the Library folders that any of the four numberings could
send a file into, before the dialog opens. An edited volume name can point at
a folder that was not listed, and then a replacement or refusal under it would
go unsaid. Make sure a drop under an edited volume name says its collisions as
any other drop does (for example by listing that folder when the edit is
committed), or, if that is not reasonable here, say in the preview that what
the Library holds under an edited folder name was not checked. Decide which
and record why in the PR.

### Docs

- Update the `DropConfirm` doc comment to describe editing.
- If `docs/spec` has rules for the drop numbering preview, add the rule for
  editing in the same style, and name it from the tests.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Unit tests: applying an edit to a page changes that file's path sent;
      applying an edit to a volume changes the paths of every file under it
- [x] Unit tests: an empty name, a name with `/`, and a name equal to a
      sibling's final name are refused with their reasons
- [x] Unit tests: a name that breaks the reading order is accepted and
      reported as out of order; a name that keeps it is not reported
- [x] `DropConfirm.test.tsx`: clicking a new name opens an input; Enter commits
      and the line shows the edited name with its mark; Escape in the input
      discards the edit without cancelling the dialog; the put-back control
      restores the rule's name
- [x] `DropConfirm.test.tsx`: a refused name keeps the input open with its
      reason; an out-of-order name shows the warning
- [x] `DropConfirm.test.tsx`: toggling either box discards the edits
- [x] `DropConfirm.test.tsx` or `packChoice.test.ts`: Add sends the edited
      paths, and a collision with the Library under an edited name is said
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] On the agent's Library, headless Chromium with a one-off Playwright
      script that drops a folder the way `07-freeze.spec.ts` `dropFolderOnto`
      does (`.tmp/agent/v368-drop.mjs` is a working example): drop a book with
      "Number the pages" ticked, edit one page's new name and one that breaks
      the order, see the warning, press Add, and see the edited names in the
      listing
- [ ] Whether editing feels right in the desktop app (needs a person)
