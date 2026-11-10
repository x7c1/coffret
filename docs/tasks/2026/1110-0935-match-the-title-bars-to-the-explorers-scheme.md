---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1110-0935-match-the-title-bars-to-the-explorers-scheme
created_at: 2026-11-10T09:32:35Z
updated_at: 2026-11-10T09:47:30Z
---

# feat(desktop): draw the windows' title bars in the explorer's colour scheme

## Overview

The explorer's colour scheme is chosen in its header (System / Light / Dark)
and kept by the server as the device's state (`explorer.json`, LA-38). The
desktop app already reads it to pick each window's background
(`explorerGround` in `frontend/packages/apps/desktop/src/main.ts`, `groundOf`
in `windows.ts`, through `addon.appearance()`), but the windows' title bars
and other native chrome still follow the operating system: with the OS in
light mode and the explorer set to Dark, a dark page sits under a light
title bar.

Set Electron's `nativeTheme.themeSource` from the explorer's appearance:
`'system'` for System, `'light'` for Light, `'dark'` for Dark.

- Set it when the app reads the appearance before showing the explorer's
  window (where `explorerGround` reads it today), so the first window opens
  with matching chrome.
- Set it again when the person switches the scheme in the explorer, without
  reloading the page or reopening the window. The main process does not see
  that switch today (the page tells the server, not the app). Give the app a
  way to learn it — for example a message from the explorer's page through
  the preload bridge (`coffretDesktop` in `preload.cts`) after the server has
  accepted the new appearance, whereupon the main process reads
  `addon.appearance()` again rather than trusting the message's content. Only
  the explorer's own page, from the served origin, may trigger it, as with
  the existing channels. Keep the browser (`make dev`) path unchanged: the
  page sends the message only where the bridge exists.
- A failed read keeps today's fallback (follow the system) and logs a warning,
  as `explorerGround` does.

`nativeTheme.themeSource` also changes `nativeTheme.shouldUseDarkColors`, which
`groundOf` reads for System; make sure the background colour picked for
System is still the operating system's, not one forced by the theme source.

On Linux, whether the title bar follows `themeSource` depends on the window
manager and GTK theme. Where it cannot follow, nothing breaks; say in the PR
body which environments are expected to follow and which are not. A person
confirms the look on macOS and Linux after merge, so no Before merge item is
needed for it.

Update the desktop app section of `docs/guides/environments.md` (where it
describes the windows) in a sentence, if it says anything the change makes
untrue, and the comments around `explorerGround` / `groundOf`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test under `frontend/packages/apps/desktop/src/` shows the
      mapping from each appearance (`system`, `light`, `dark`) to the
      `themeSource` value, and that a failed read leaves `'system'`.
- [x] A unit test shows that the switch message is taken only from the
      explorer's served origin, and that on it the app reads the appearance
      afresh and sets `themeSource` from that read.
- [x] A unit test shows the explorer's page sends the switch message through
      the bridge after the server accepted a new appearance, and sends nothing
      where `window.coffretDesktop` is absent.
- [x] `make check` passes.
