---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rqE "<th[^>]*>Name</th>" frontend/packages/apps/web/src/ && ! grep -rqE "<th[^>]*>name</th>" frontend/packages/apps/web/src/'
assignee: null
branch: task/1014-0326-put-row-actions-in-a-menu-select-several-files-and-lead-a-locked-list-to-unlock
created_at: 2026-10-13T18:26:12Z
updated_at: 2026-10-13T19:58:00Z
---

# feat(explorer): put row actions behind a menu, delete several files at once, close the reader beside the image, and lead a locked list to unlock

## Overview

Five small changes to the explorer (`frontend/packages/apps/web/src`), all
found while using it day to day. They touch the same two components, so they
land together.

### 1. A row's actions go behind a ⋮ menu

Every file and folder row ends in its own `Delete…` button
(`FileList.tsx:369-428`, `DeleteButton` at `:731-745`), so a list of twenty
rows shows twenty buttons and looks restless. Replace the button with one
`⋮` button per row (`aria-label="actions for <name>"`) that opens a small
menu holding `Delete…`. The menu is the only item today, and it is built so
more items can be added later: a list of `{ label, onPress }` entries rendered
the same way. A click outside, `Escape`, or choosing an item closes it. The
menu button, like the Delete button before it, stops the click so the row does
not open (`FileList.tsx:736-739`). `onDelete` keeps its signature
(`DeleteTarget`, `:170`) and the folder row's item still sends
`{ folder, paths: [] }`. Only files for which `deleteOffered` holds
(`deleteEntries.ts:34`) get a Delete item, as today.

### 2. Several files can be selected and deleted at once

Deleting one page of a Pack at a time repeats the Pack's rebuild (PK-10) for
every page. `deleteEntries.ts` and the server already take several Entry Paths
(`DeleteTarget.paths`; `routes/delete.rs` `target_of` reads repeated
`entry=`), and the usecase's header comment at `deleteEntries.ts:11-13` says
nothing there changes when a selection names more. What is missing is the
selection:

- A checkbox cell at the start of each file row for which `deleteOffered`
  holds (`aria-label="select <name>"`), and one in the header that selects or
  clears every selectable file in the listing. Folder rows have no checkbox:
  the server takes one folder per target, and a folder is deleted from its own
  row's menu.
- Clicking the checkbox toggles the row without opening it. Clicking the row
  elsewhere still opens the file.
- While at least one file is selected, a line above the table says
  `N selected` and offers one `Delete…` button that calls
  `onDelete({ folder: null, paths })` with the selected paths in listing
  order. `targetName` already renders N paths as "N files"
  (`deleteEntries.ts:65-73`), so the confirmation reads right without change.
- The selection is cleared when the listing changes folder, when a deletion
  is armed, and when the listing reloads without those paths. Keep the
  selection state in `FileList` (or a sibling hook) rather than in `App.tsx`,
  so `App.tsx` only learns about it through `onDelete`.

Update the header comment of `deleteEntries.ts` (`:11-13`): the explorer now
has a selection.

### 3. The reader closes on a click beside the image, not on it

`ReaderView.tsx:186-198` closes on any click in the overlay, including on the
`<img>`. Change it so a click on the image does nothing and a click anywhere
else in the overlay closes, keeping `Escape` and the arrow keys
(`:154-169`). Fix the footer text (`:207`) and the doc comment (`:27-28`) to
say so. The failed-state block already stops its click (`:247-249`); keep it.

### 4. Column names start with a capital

`FileList.tsx:361-364` labels the columns `name`, `size`, `modified`,
`state`. Make them `Name`, `Size`, `Modified`, `State`. No test or journey
asserts on the lowercase forms. The gate in `check_command` greps for the
capitalised `Name` header and the absence of the lowercase one; the other
three follow the same edit.

### 5. A locked list leads to unlock instead of a dead "try again"

After an idle lock, when the Passphrase window has been closed, the listing
and folder regions show the server's `locked` refusal and a `try again`
button (`App.tsx` `Region` at `:1135-1178`, used at `:943` and `:1016`).
`retry` (`:157-162`) only reloads, so pressing it returns the same refusal and
nothing changes. The failed state already carries `locked`
(`useAsked.ts:26-29,81`), and `App.tsx:251-274` already has `unlock`, the
callback that asks the desktop app for its Passphrase window and that the
status bar's `unlock` button uses (`StatusBar.tsx:141-163`).

When `state.locked` is true, `Region` renders the refusal and an `unlock`
button that calls `unlock`, in place of `try again`, followed by the same
said/refused lines the status bar shows for the lock (`unlockSaid`,
`unlockRefused`), so a server without a window (one started from the CLI,
whose `POST /api/unlock` is refused with `locked`) tells the person where to
unlock instead of looping. Pass what `Region` needs through props; do not reach
for module state. Move `Region` into its own file (`Region.tsx`) with a test,
since `App.tsx` has no render test of its own.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `FileList.test.tsx`: a file row and a folder row each render one
      `actions for <name>` button and no `delete <name>` button; opening the
      menu and choosing `Delete…` calls `onDelete` with the same
      `DeleteTarget` the old button sent, and `onOpenFile` is not called
- [x] `FileList.test.tsx`: a file whose `container` is null gets no Delete
      item and no checkbox
- [x] `FileList.test.tsx`: selecting two files and pressing the selection's
      `Delete…` calls `onDelete({ folder: null, paths: [the two paths] })` in
      listing order; the header checkbox selects every selectable file and
      clears them again; the `N selected` line is absent when nothing is
      selected; the selection is cleared when the listing's folder changes
- [x] `ReaderView.test.tsx` (new): a click on the overlay outside the image
      calls `onClose`; a click on the image does not; `Escape` still does
- [x] The header cells read `Name` / `Size` / `Modified` / `State`
      (`check_command` greps for `>Name</th>` and the absence of
      `>name</th>` under `frontend/packages/apps/web/src/`)
- [x] `Region.test.tsx` (new): a failed state with `locked: true` renders an
      `unlock` button that calls the given unlock callback and no `try again`
      button; a failed state without `locked` renders `try again` as before
- [x] `make check` passes (the frontend's `pnpm -r build`, `typecheck`,
      `test`, `lint` and the backend stages)

### Before merge (verified outside the check command)

- [x] On the agent's Library (`.tmp/agent/up.sh 1`, headless Chromium through
      `.tmp/agent/shot.mjs` or a one-off Playwright script): a row's `⋮`
      opens the menu; `Delete…` from it reaches the confirmation; selecting
      two pages of `lib/book-000` and deleting them removes both in one run
      (the status bar shows one deletion, the Pack is rebuilt once); a click
      beside the page image closes the reader and a click on the image does
      not
- [x] With the agent server started with `COFFRET_IDLE_MINUTES=1`, after the
      lock the listing region shows the refusal and an `unlock` button, and
      pressing it shows the server's answer (the CLI-started server refuses
      with `locked`, and the line says so) rather than the same refusal again
- [ ] The look of the menu, the checkbox column and the `N selected` line —
      needs a person: whether it reads calm beside Google Drive's layout is a
      matter of taste
