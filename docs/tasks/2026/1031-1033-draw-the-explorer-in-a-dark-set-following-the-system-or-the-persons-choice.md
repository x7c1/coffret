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
branch: task/1031-1033-draw-the-explorer-in-a-dark-set-following-the-system-or-the-persons-choice
created_at: 2026-10-31T01:33:42Z
updated_at: 2026-10-31T02:38:19Z
---

# feat(web): draw the explorer in a dark set as well, following the system or the person's choice

## Overview

The explorer is drawn in one light set of named colours (`COLOR` in
`frontend/packages/apps/web/src/theme.ts`, which `theme.test.ts` holds every
component to). Add a dark set and a way to choose between them.

1. **The system's choice by default.** Follow `prefers-color-scheme`, and
   follow it live when the system switches.
2. **A switch in the header.** An icon button at the header's right end,
   beside **+**, opening a small menu of three: `System` (the default),
   `Light`, `Dark`, the chosen one marked. Its icon shows the current state (a
   sun, a moon, a half-filled circle for System). It has an accessible name.
   Nothing goes into the desktop app's menu bar (it is hidden on Linux) and
   there is no settings page.
3. **The dark set.** Give every colour of `COLOR` a dark value, the folder
   icon's `folderFill` / `folderStroke` included, keeping text contrast at
   4.5:1 or better and the focused, selected and drop-target rows told apart
   by lightness and not hue alone. The ground and hover states `index.html`
   paints follow the set too. The reader's sheet (`SHEET_COLOR`) stays dark in
   both. Keep every colour going through a name, as `theme.test.ts` requires.
4. **Where the choice is kept.** It is this device's preference about how the
   explorer looks, not Library data. The desktop app opens the explorer on a
   port chosen at each launch, so browser storage (per origin) may not survive
   a restart: find out whether it does in the desktop app and in `make dev`.
   If it does not, keep the choice on the server as this device's state (and
   say so in `docs/spec/` beside the other device-state routes); otherwise
   browser storage is enough, read and written so that a refusal to read or
   write falls back to `System`.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it`
and `make e2e-electron`, and in CI) must keep passing; add a journey that
switches to Dark and back.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test covers the choice resolving to light or dark from `System` with
  the system set either way, and from `Light` / `Dark` regardless of it.
- [x] A test covers every name in `COLOR` having a dark value, and
  `theme.test.ts` still finds no colour literal outside `theme.ts`.
- [x] A test covers the switch's menu, its marked item and its accessible
  name.
- [x] A test covers the choice surviving a reload, wherever it is kept.
- [x] An e2e journey switches to Dark and back to System.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent looks at the tree (unfocused, focused,
  selected and drop-target rows, folder icons), the details pane, the header,
  the Activity panel, a menu and a dialog in Dark, and records in the PR what
  it saw and the contrast of text on the ground.
- [x] In the desktop app (`make desktop-dev`), the agent finds whether the
  choice survives quitting and starting the app again, and records it.
