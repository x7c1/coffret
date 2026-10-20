---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && test -f frontend/packages/apps/desktop/package.json && grep -q "contextIsolation: true" -r frontend/packages/apps/desktop/src/ && ! grep -rqE "nodeIntegration: true|contextIsolation: false|sandbox: false|webSecurity: false" frontend/packages/apps/desktop/src/ && ! grep -rq "ipcMain" frontend/packages/apps/desktop/src/ && grep -q "ELECTRON_SKIP_BINARY_DOWNLOAD" .github/workflows/ci.yml'
assignee: null
branch: task/1021-0449-open-the-explorer-in-an-electron-window
created_at: 2026-10-20T19:49:25Z
updated_at: 2026-10-20T21:03:34Z
---

# feat(desktop): an Electron app that opens the explorer in its own window

## Overview

The previous change put the server, the explorer host and a Passphrase door into a Node-API addon (`backend/crates/apps/coffret-napi`, built by `make napi-addon`, over `coffret-desktop-core`). This change adds the Electron app that loads it.

- The explorer runs inside the app's own window instead of the system browser.
- The Passphrase is typed into a second window that the door serves.
- The Tauri app stays in place and keeps working. A later change removes it. Until then the two live side by side, and the Electron app is started only through a development target.

### Decisions (already made; implement them)

1. **Package.** A new pnpm package `frontend/packages/apps/desktop` (`@coffret/desktop`), written in TypeScript like the rest of the frontend.
   - Main-process code is compiled with `tsc` to `dist/`. Pin `electron` to an exact version (44.5.1 unless a newer patch of 44 is current; say which in the PR).
   - Add `electron: true` to `allowBuilds` in `frontend/pnpm-workspace.yaml`.
   - The package joins `pnpm -r build/typecheck/test/lint`. Its unit tests must not need the Electron binary: keep main-side logic in plain modules that take Electron objects as parameters, and test those with vitest.
   - CI jobs that install the frontend but never run Electron set `ELECTRON_SKIP_BINARY_DOWNLOAD=1`, so `pnpm install` does not fetch it. These are the frontend, interop, spec, s3-store and e2e jobs, and any other job that runs `pnpm install` (`.github/workflows/ci.yml`).
   - If `pnpm audit` then reports an Electron advisory that no release clears, follow the rule in `pnpm-workspace.yaml`'s comment. Otherwise take the release that clears it.
2. **No IPC at all.** No window has a preload in this change, and main exposes nothing to any renderer: no `ipcMain` and no `contextBridge`.
   - The Passphrase page talks only to the door, over same-origin `fetch`.
   - Main learns the outcome from the addon's events (`LibraryOpened`, `LibraryUnlocked`, `UnlockRequested`, `ServerStopped`).
   - Every window has `contextIsolation: true`, `sandbox: true` and `nodeIntegration: false`, and its own `partition`: one for the Passphrase window, a persistent one for the explorer.
   - `webSecurity` stays on. Each session denies every permission request.
3. **Startup.**
   - Take the single-instance lock first. A second launch focuses the open window.
   - Then `start(identifier, idleMinutes)` with `COFFRET_IDLE_MINUTES` as today. The identifier is `io.github.x7c1.coffret`, or `io.github.x7c1.coffret.dev` when the development target runs it.
   - Then open the Passphrase window on `openDoor(Open)`.
   - A failure before anything is served shows `dialog.showErrorBox("Coffret could not start", <sentence>)` and exits with status 1, as the Tauri app does. The sentence is the addon's exception message.
4. **Passphrase window.** About 380×260, not resizable, loaded from the door's URL.
   - On `LibraryOpened`, main hides it and opens the explorer.
   - On `UnlockRequested` (the explorer's *unlock*, or idle lock), main reopens it from `openDoor(Unlock)` and brings it forward. It does not reload a window already showing in unlock mode, so typing in progress is not wiped.
   - On `LibraryUnlocked`, main hides it.
   - Closing it before a Library is served quits the app. Closing it afterwards only hides it.
   - Navigation away from the door's origin is refused. Every `window.open` is denied.
5. **Explorer window.** Opened at `http://<explorerAddress()>/#launch=<issueTicket()>`.
   - Navigation away from the explorer host's origin is refused.
   - `window.open` and target-blank links to an `https:` URL go to `shell.openExternal`, so the Drive re-consent opens in the system browser as today. Everything else is denied.
   - In this change, closing the explorer window quits the app; a later change makes the app stay resident with a tray.
   - When the page has lost its session (`Explorer.tsx`'s "no session" text), the window gets a fresh ticket from the app.
     - Give the app menu a *Reload the explorer* item (Cmd/Ctrl+R) that loads a new `#launch=` URL.
     - Change the no-session text so it no longer sends the person to a tray or the system browser. Make it say the app must be reloaded, without naming Tauri or the browser.
6. **Quit and stop.** On quit, call `retire()`. On `ServerStopped`, show an error box saying the Library's server stopped, then quit.
7. **Development target.** `make desktop-electron-dev`:
   - builds the explorer and the addon (`make napi-addon`) and the desktop package;
   - then runs Electron over the development state directory, refusing to run without `COFFRET_STATE_DIR` as `desktop-dev` does, with the `.dev` identifier.
   - Name it in `docs/guides/environments.md`'s target table, together with `make napi-addon`, which the previous change added without a row.
8. **Spec and concepts.**
   - LA-17 / LA-21: describe the second way a ticket reaches the explorer, `loadURL` with the ticket in the fragment, in the app's own window. The launch-file way stays for the Tauri app until it is removed.
   - DK-1: the Passphrase window may be the Electron app's window served by the door.
   - Update the concept docs whose wording assumes IPC is the only path:
     - `docs/concepts/passphrase/README.md` (around lines 31–45: name the loopback request to the app's own listener as the other path, and cite LA-22 and LA-23);
     - `docs/concepts/master-key/README.md` (around lines 53–55, likewise).

### Out of scope

- Drop-as-paths.
- Tray, residency, and the Linux tray fallback.
- Packaging, fuses and signing.
- Removing Tauri, the launch file, or the browser development flow (`make dev`).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `frontend/packages/apps/desktop` exists and passes `pnpm -r build/typecheck/test/lint` (in `make check`).
- [x] Every `BrowserWindow` sets `contextIsolation: true`. No source in the package sets `nodeIntegration: true`, `contextIsolation: false`, `sandbox: false` or `webSecurity: false`, and none uses `ipcMain` (gated in `check_command`).
- [x] Unit tests cover the window options builder: sandbox, contextIsolation, nodeIntegration and a distinct partition per window.
- [x] Unit tests cover the navigation guard: same origin allowed; other origins, `file:` and `javascript:` refused.
- [x] Unit tests cover the window-open handler: `https:` goes to openExternal; `http:`, `file:` and custom schemes are denied.
- [x] Unit tests cover the event dispatcher, for each state × event:
  - `LibraryOpened` with the Passphrase window open in open mode;
  - `UnlockRequested` when the window is hidden, and when it is already showing unlock mode (no reload);
  - `LibraryUnlocked`;
  - `ServerStopped` before and after a Library is served;
  - closing the Passphrase window before and after a Library is served.
- [x] Unit tests cover the idle-minutes parsing, matching the core's rule (empty → 30; non-integer or below 1 refused).
- [x] CI sets `ELECTRON_SKIP_BINARY_DOWNLOAD` (gated in `check_command`).
- [x] The LA and DK edits pass `make spec-rule-ids` and `make spec-citations`.

### Before merge (verified outside the check command)

- [x] On macOS, `make desktop-electron-dev` over the agent Library (recreated in the current format if it no longer opens cleanly):
  - the Passphrase window opens the Library;
  - the explorer loads its listing in the app's window;
  - with `COFFRET_IDLE_MINUTES=1`, the idle lock brings the Passphrase window back in unlock mode, and the Passphrase unlocks the explorer in place;
  - Cmd+R reloads the explorer with a fresh ticket;
  - quitting leaves no process behind.
  The merging session drives this with Playwright's `_electron` and records it in the PR.
