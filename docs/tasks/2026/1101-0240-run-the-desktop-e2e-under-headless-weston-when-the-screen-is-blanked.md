---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1101-0240-run-the-desktop-e2e-under-headless-weston-when-the-screen-is-blanked
created_at: 2026-10-31T23:58:44Z
updated_at: 2026-11-01T00:27:57Z
---

# test(e2e): run the desktop journey under headless weston on a blanked screen, and keep one failed journey from failing the next

## Overview

1. **A blanked screen.** On a development machine whose GNOME session has
   blanked the screen, `make e2e-electron` stops at its first screenshot:
   Chromium's `Page.captureScreenshot` does not return. Make
   `scripts/e2e-electron.sh` run the suite under a headless weston
   (`--ozone-platform=wayland` on a weston started with the headless
   backend) when weston is installed and either no display is usable or the
   session reports the screen blanked. Otherwise, when it cannot run, say why
   and exit non-zero rather than hang. CI's `xvfb-run` path is unchanged.
   Document the behaviour where the script and the Makefile describe how the
   suite is run.
2. **The Makefile's `e2e-electron` comment** says everything is under
   `.tmp/e2e-electron/`, rebuilt each run; a failed run is also kept under
   `.tmp/e2e-electron-failed/<when>/`, which is not rebuilt. Say both.
3. **One failed journey fails the next.** In `make e2e-it`, when a journey
   fails Playwright starts a new worker, which restarts the server and ends
   every launch ticket; the next journey then opens the launch file written
   before the restart, whose ticket is spent (`POST /auth/launch` answers 401,
   the host logs `authentication_required, reason: ticket`), and fails at its
   first step for no fault of its own. Make a journey read the launch file the
   running server wrote (wait for the one written after the server started),
   so a failure stays with the journey that had it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make shell-lint` (part of `make check`) passes on the changed script.
- [x] A script test (or the existing shell test harness) covers choosing
  weston, choosing the current display, and refusing with a message.
- [x] The Makefile comment names `.tmp/e2e-electron-failed/<when>/`.
- [x] A journey started after a worker restart opens the explorer with a
  ticket the running server issued (covered by a test of the helper, or by a
  journey that forces a restart).

### Before merge (verified outside the check command)

- [x] On this development machine, the agent runs `make e2e-electron` once
  with the display usable or blanked (whichever it is) and records which path
  the script chose and that the suite passed.
