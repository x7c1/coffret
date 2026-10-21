---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && test -n "$(ls frontend/packages/apps/desktop/assets/tray* 2>/dev/null)" && grep -rq "requestSingleInstanceLock" frontend/packages/apps/desktop/src/ && grep -rq "StatusNotifierWatcher" frontend/packages/apps/desktop/src/ && ! grep -rqE "nodeIntegration: true|contextIsolation: false|sandbox: false" frontend/packages/apps/desktop/src/'
assignee: null
branch: task/1021-1026-keep-the-desktop-app-resident-with-a-tray
created_at: 2026-10-21T01:26:26Z
updated_at: 2026-10-21T02:03:30Z
---

# feat(desktop): keep the Electron app resident with a tray, and tell its development build apart

## Overview

Today the Electron app (`frontend/packages/apps/desktop`) quits when its explorer window closes. There is no tray, and the development build looks the same as the installed one. This change makes the app resident, as the Tauri app is today, gives it a tray, and makes the development app recognisable. One app instance per identifier stays the rule.

### Decisions (already made; implement them)

1. **Closing the explorer window hides it; the app stays.** The app ends through:
   - the tray's *Quit Coffret*;
   - the application menu's *Quit* (Cmd+Q on macOS, Ctrl+Q elsewhere, present on every OS);
   - or the OS ending the session.

   On quit the app retires the addon, as today. Closing the Passphrase window before a Library is open still quits, since nothing is served yet.
2. **Bringing the explorer back.** The explorer window comes to the front on:
   - a second launch (the single-instance lock is already taken first);
   - a click on the Dock icon (macOS `activate`);
   - the tray's *Open the explorer*, and a click on the tray icon where the platform delivers one.

   If the Library is locked when it comes back, the Passphrase window is brought forward as for an unlock request. If the explorer window was destroyed, a new one is opened with a fresh launch ticket.
3. **The tray menu.**
   - *Open the explorer*.
   - *Unlock…*: brings the Passphrase window in unlock mode while the Library is locked. It is disabled, or says the Library is unlocked, while unlocked.
   - A separator, then *Quit Coffret*.

   The Tauri tray's *Disconnect all browser tabs* has no meaning with one app window and is not carried over. Update LA-18's mention of the app ending all sessions at once to say they all end when the app quits or retires.
4. **Tray icons and tooltip.**
   - Use the existing tray artwork from `backend/crates/apps/coffret-desktop/icons-tray/`, copied into `frontend/packages/apps/desktop/assets/` because the Tauri crate is removed later:
     - macOS: the black template image (`tray.png` / `@2x`, marked as a template);
     - Linux: the light image (`tray-light`);
     - the development app: the `-dev` variants.
   - Tooltip: "Coffret", or "Coffret (dev)" for the development identifier.
5. **No tray host on Linux.** A tray icon there is shown only if something on the session bus owns `org.kde.StatusNotifierWatcher`.
   - Ask with `gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner org.kde.StatusNotifierWatcher`, bounded by a timeout of about one second. Treat a missing `gdbus`, a timeout or an error as "no host".
   - Without a host there is no tray, and closing the explorer window quits the app.
   - Log which way it went (one info event, no paths). macOS and Windows always have a tray.
6. **Telling the development app apart.** With the development identifier (`io.github.x7c1.coffret.dev`):
   - the window icon (Linux) and the Dock icon (macOS) use the badged set in `backend/crates/apps/coffret-desktop/icons-dev/`, copied into the package's assets;
   - window titles read "Coffret (dev)";
   - on Linux the window class / app ID is the identifier, as the Tauri app's `glib::set_prgname` does, so GNOME groups and names it apart from the installed app.

   Keep the installed identifier's look unchanged.
7. **A spent Passphrase window.** After 5 refused tries the door ends a window's token (LA-24). Today the page says to open the window again, but main does not know this, so pressing *unlock* only brings the spent window forward.
   - Add a door event for "this window's token ended unused", carrying no token, kind only, per LA-25.
   - Expose it through the addon. Main then reloads the Passphrase window fresh the next time it is brought forward.
   - Remove the workaround paragraph that `docs/guides/environments.md` gained for this case.
8. **Docs.** Update `docs/guides/environments.md`'s Electron section for:
   - residency;
   - the tray and its menu;
   - how to quit;
   - the Linux fallback;
   - how the development app looks.

### Out of scope

- Packaging and the installed bundle.
- Removing the Tauri app.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Tray assets exist under `frontend/packages/apps/desktop/assets/` (gated), and the app uses `requestSingleInstanceLock` and checks `StatusNotifierWatcher` (gated).
- [x] Unit tests cover what closing the explorer window does, for each combination of tray present or absent × quitting or not × platform: hide, or quit.
- [x] Unit tests cover what bringing the explorer back does, for each state:
  - window present and visible;
  - window hidden;
  - window destroyed (a fresh ticket);
  - the Library locked (the Passphrase window brought in unlock mode).
- [x] Unit tests cover the tray menu for unlocked and locked: *Unlock…* enabled only while locked, and *Quit* quits.
- [x] Unit tests cover the tray-host answer parsing: `(true,)` gives present; `(false,)`, a timeout, a missing binary and malformed output each give absent.
- [x] Unit tests cover the identity mapping for the installed and development identifiers: tray icon, window icon, tooltip and title.
- [x] Unit and Rust tests cover the spent-window event:
  - the door raises it after the fifth refusal, and not after a success;
  - its event carries no token;
  - main reloads a spent Passphrase window when bringing it forward, and keeps a live one as it is.
- [x] The edited LA-18 and any new or edited door rule pass `make spec-rule-ids` and `make spec-citations`.

### Before merge (verified outside the check command)

- [x] On macOS, driving the development app with Playwright's `_electron` over the agent Library:
  - closing the explorer window leaves the app running, with the tray present;
  - a second-instance launch and a Dock `activate` bring the explorer back;
  - the tray menu's *Open the explorer* and *Quit Coffret* work (invoked through the menu object);
  - after 5 wrong Passphrases in unlock mode, pressing *unlock* in the explorer shows a fresh Passphrase window that accepts the right Passphrase;
  - the development app's window title reads "Coffret (dev)".

  The merging session records this in the PR. The tray icon's look in the menu bar is left to a person, since it needs eyes on the menu bar.
