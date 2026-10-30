---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1031-0048-finish-the-explorer-small-gaps-folder-icons-and-the-linux-menu-bar
created_at: 2026-10-30T15:48:00Z
updated_at: 2026-10-30T16:46:03Z
---

# feat(web): close the explorer's small gaps, tint the folder icons and hide the Linux menu bar

## Overview

The rebuilt explorer is on main. Using it turned up a set of small gaps, and
two touches to its look were decided. Close them in one change. Each item
below names where it is; check each one against the code first, and where an
item turns out to be already resolved, say so in the PR rather than changing
anything for it.

**Small gaps**

1. *"on this device" with two meanings.* The tree's column says
   `On this device` of a file this device has. Any line that uses "on this
   device" for a folder this device has no folder chosen for (the banner over
   such a folder, `mapping.ts`, `Banner.tsx`) must not: it says
   `No folder chosen for …` / `Choose a folder on this device for …` instead.
   Make the wording consistent wherever the second meaning still appears.
2. *"Pack this folder…" says a folder holds no files when its files are
   still on their way in.* The freeze preview leaves out the paths reserved by
   a drop that is in progress or was cut short (spec: PK-17), so a folder that
   holds only such files previews as `files === 0` with no reason, and the
   dialog says `it holds no files` (`packFolder.ts`). Add a count of those
   paths to the preview the server answers (`FreezePreview`, for example
   `receiving`), and have the dialog's "Left as they are" say
   `<n> files not in the Library yet`.
3. *Right-clicking a selection with nothing to delete does nothing.* With a
   selection whose rows offer no action (for example only prepared folders),
   no menu and no ⋮ opens and the browser's own menu is suppressed too. Open
   the menu with a disabled `Delete <n> items…`, or a single disabled
   `Nothing here to delete`, so the click is answered.
4. *The folder dialog's placeholder is not drawn in the faint colour.* The
   input in `frontend/packages/apps/web/src/ChooseFolder.tsx` (and any other
   input with a placeholder) draws its placeholder in `COLOR.faint`.
5. *The tree's focus is lost after unlocking.* After an idle lock and an
   unlock the tree is drawn again and the row focused before the lock is not
   focused. Put the focus back on that row (or its nearest surviving
   ancestor) once the Library is unlocked.
6. *A server line says "dropped" of a file picked with **+**.* The lines for a
   file reserved to be added as a Pack (`coffret-server/src/finding.rs`,
   `coffret-device/src/finding_reason.rs`, and the `work.json` contract) say
   `this file was dropped to be added as a Pack …`. A file picked with **+**
   goes through the same reservation. Say `added` (or another word true of
   both) instead, and update the contract and its tests.
7. *A deleted folder's path stays in the tree's expanded set.* `expanded` in
   `frontend/packages/apps/web/src/tree.ts` keeps the path of a folder a
   delete removed, which asks once more for a listing that answers
   `held: false`. Drop paths that no longer stand from `expanded` when a
   listing shows they are gone.
8. *`OpenLibrary::wholly_present` asks only about the Entries directly in a
   folder, and its name does not say so.* It sits beside
   `ChildFolder::present_throughout` (every depth). Rename it to say its reach
   (for example `directly_present`) in
   `coffret-device/src/browse/wholly_present.rs` and its callers
   (`coffret-server/src/routes/file.rs`).

**Folder icons in pale blue.** The folder icon is a stroke-only outline in
dim. Fill it pale blue with a slightly deeper blue outline:

- add two named colours to `COLOR` in `frontend/packages/apps/web/src/theme.ts`:
  `folderFill: '#cddcfb'` and `folderStroke: '#5b8def'`;
- the tree's folder icon (`RowIconMark` in `TreeTable.tsx`) is filled with
  `folderFill` and stroked with `folderStroke`;
- on the focused row it stays a white outline with no fill, as now;
- the folder icon a dialog draws (`Dialog.tsx`) uses the same two colours;
- file icons are unchanged (accent outline).

**The menu bar on Linux.** The desktop app's windows show a menu bar (File,
Edit, Explorer) on Linux that the app hardly needs. Hide it there with
`autoHideMenuBar` (Alt still shows it), keeping the menu itself so that
Ctrl+R (Reload the explorer) and Ctrl+Q keep working. macOS is unchanged: its
menu bar is the system's, and the Edit menu is what carries Cmd+C / Cmd+V /
Cmd+Z to the inputs. The window options live in
`frontend/packages/apps/desktop/src/` (`explorerWindowOptions`,
`passphraseWindowOptions`).

Write the code and its comments so they do not tie the explorer to one kind
of content: say "a book whose pages are image files" or "a folder of page
images" rather than "scanned book". Where this change touches a comment that
says "scanned", reword it the same way.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it`
and `make e2e-electron`, and in CI) must keep passing: when a change moves
something a journey relies on (a role, a label, a row it counts), update the
journey in this same change so it still proves what it proved.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test covers the freeze preview's count of files still on their way
  in, and the dialog line `<n> files not in the Library yet` built from it.
- [x] A test covers the menu a selection with nothing to delete opens.
- [x] A test covers the expanded set dropping a path a listing shows is gone.
- [x] A test covers the Linux window options hiding the menu bar and the macOS
  ones not doing so, and the menu still carrying Ctrl+R and Ctrl+Q.
- [x] A test covers the folder icon's fill and stroke on an unfocused and on a
  focused row.
- [x] `theme.test.ts` still finds no colour literal outside `theme.ts`.
- [x] No server or device line for a reserved Pack file says "dropped".

### Before merge (verified outside the check command)

- [x] On `make local`, the agent looks at the tree with folders unfocused,
  focused and selected, a dialog with a folder icon, the folder dialog's
  placeholder, a right-click on a selection of prepared folders, and the
  focus after an idle lock and an unlock, and records in the PR what it saw.
