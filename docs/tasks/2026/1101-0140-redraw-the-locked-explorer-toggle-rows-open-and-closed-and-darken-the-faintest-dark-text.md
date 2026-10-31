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
branch: task/1101-0140-redraw-the-locked-explorer-toggle-rows-open-and-closed-and-darken-the-faintest-dark-text
created_at: 2026-10-31T16:38:00Z
updated_at: 2026-10-31T17:40:57Z
---

# feat(web): redraw the locked explorer, open and close rows with one gesture, and darken the faintest dark text

## Overview

Three changes to the explorer (`frontend/packages/apps/web/`), plus one stale
doc comment.

1. **The faintest dark text.** In the dark set of `theme.ts`, `dim` (a file's
   size) and `faint` (a folder's size) are too close to tell apart (ratio
   1.21). Set the dark `faint` to `#7c838e` (about 1.68 against `dim`, 4.69:1
   on the ground). Leave `dim` and every light value as they are: a brighter
   `dim` blurs into `text`. This value was chosen knowingly: it holds 4.5:1
   on the ground but not on the raised surfaces (panel, hover, picked), where
   it stays above 3:1. Every other dark text colour keeps 4.5:1.

2. **The locked explorer.** Today the lock shows in three places at once: a
   red refusal and an `unlock` button at the top left of the tree region
   (`Region.tsx`), a red "the Library is locked" with `unlock` in the Activity
   row (`ActivityPanel.tsx`), and a `LockedBar` below it (`Explorer.tsx`)
   saying "the Library is locked; this tab waits for it to be unlocked".
   Replace them with one designed state:
   - The main area (where the tree and details pane stand) shows, centred: a
     56px round tile in the accent's tint with a lock icon in the accent, the
     heading **The Library is locked** (18px, 600), two lines in `dim` —
     "Its files stay hidden until it is unlocked." then, on its own line,
     "Unlock it to browse and read again." — and a primary **Unlock**
     button below them. No red: a lock is not an error.
   - The Activity row reads a lock icon and **Locked** in `dim`, followed by
     an outlined **Unlock** button. The row's other controls that need the
     Library (Look for what is new) are drawn disabled; so is **+** in the
     header.
   - Remove `LockedBar` and the word "tab" wherever it remains in what the
     person reads.
   - Both Unlock buttons do what the current `unlock` buttons do. When the
     Library is unlocked the explorer returns to its normal state without a
     reload, as today.

3. **Open is a toggle.** Opening a row (double-click, Enter, or a tap — all
   three the same) becomes a toggle:
   - Opening a collapsed folder expands it and opens the reader on it, whether
     or not it holds an image (drop the `firstImage` gate in `App.tsx`
     `openFolder` that only says "holds nothing the reader can show"; the
     reader's sheet can already draw a folder with no images).
   - Opening an expanded folder collapses it; if the reader is showing that
     folder or anything under it, the reader closes too.
   - Opening the file the reader is showing closes the reader; opening any
     other file opens it in the reader as today.
   - The notice for a folder this device has not mapped (the reader cannot
     fetch from Storage there) stays as it is.

4. **Stale comment.** The doc comment in `ActivityPanel.tsx` (around line 68)
   still describes a `?` button that shows the list on hover or focus. Today
   the list appears only when the `?` key is pressed and nothing on screen
   marks it; say that.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it`
and `make e2e-electron`, and in CI) must keep passing; update any that relied
on the old lock texts or on opening an expanded folder.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test pins the dark `faint` at `#7c838e`, at 4.5:1 or better on the
  ground and 3:1 or better on the raised surfaces, and the contrast tests
  still hold every other dark text colour at 4.5:1 or better.
- [x] A test covers the locked state: the centred heading, the two lines, the
  Unlock button, the Activity row's Locked and Unlock, and no `LockedBar` and
  no "tab" text.
- [x] A test covers unlocking from either button returning the explorer to its
  normal state.
- [x] Tests cover the toggle: a collapsed folder opens (with and without
  images) and the reader opens on it; an expanded folder collapses and closes
  a reader that shows it or something inside it, and leaves a reader showing
  something elsewhere alone; the file the reader shows closes it; Enter
  behaves like double-click.
- [x] A test covers the unmapped-folder notice still appearing.
- [x] The desktop e2e journey that waits for the idle lock (`e2e/electron/desktop.spec.ts`)
  sees the new locked state before it unlocks.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent looks at the locked state and the toggle in
  Light and Dark, and records in the PR what it saw, including folder and file
  sizes in Dark.
