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
branch: task/1101-0230-open-in-the-chosen-colour-scheme-clear-leftover-overwrite-links-and-log-a-failed-check
created_at: 2026-10-31T22:48:22Z
updated_at: 2026-10-31T23:43:22Z
---

# fix: open in the chosen colour scheme, clear leftover overwrite links, and log a failed Check for changes

## Overview

Six small fixes.

1. **First paint in the chosen scheme.** The appearance choice is kept on
   the server (`explorer.json`, LA-38), and until `/api/appearance` answers
   the explorer is drawn in the system's scheme, so a person who chose Dark
   on a light system (or the reverse) sees a flash. When the host serves the
   explorer's page, write the chosen scheme into it (the
   `<meta name="color-scheme">` and whatever `index.html` paints the ground
   with) so the first paint is already right. No cookie. `System` serves the
   page as today.
2. **Desktop window background.** The desktop app's `BrowserWindow`
   (`windows.ts`) has no `backgroundColor`, so a dark explorer opens on
   white. Pass the ground colour of the scheme in effect: the person's
   choice, or `nativeTheme.shouldUseDarkColors` for System.
3. **Leftover overwrite links.** An add that overwrites a file keeps the old
   one as a hard link under a scratch name and removes it when the add ends;
   if the server dies first, the link stays. The sync's cleanup (OC-8) skips
   names an add holds a lock on. When a server opens the Library, before it
   serves anything, remove such scratch links whose lock is no longer held
   (no live add owns them), restoring nothing. Say so in the spec beside OC-8.
4. **A failed Check for changes in Activity.** A catch-up the person starts
   (Check for changes / Look for what is new) is not in LA-12's log today: its
   failure shows only at the Activity row's right end and on a band over the
   tree, and is gone after the next try. Add the catch-up the explorer starts
   to LA-12's subjects, so its failure appears in the Activity list like a
   failed add or fetch (and its success as other work's does). Update LA-12.
5. **The locked screen's leftovers.** While the Library is locked the header
   still shows the focused row's location (for example `comics / vol 1`),
   restored from the URL, beside a screen saying the files stay hidden: hide
   the location while locked, as the header already hides a name it could not
   fetch. The server's locked refusal (`api_error/admission.rs`, and its copy
   in `frontend/packages/gateway/api/src/contract/refusals.json`) still says
   "press unlock" in lower case; the button is now **Unlock**, so say
   "press Unlock" in both and in the tests that pin the text.
6. **"The next sync adds it."** A drop's sync now walks only the folders the
   drop wrote into (EP-10). The Activity line for an add that could not be
   taken back (`frontend/packages/apps/web/src/activity.ts`, around the
   sentence "…and the next sync adds it") and LA-36's "the next sync carries
   in" promise more than that: say instead that a sync whose scope covers the
   file carries it in (a full sync, or a later drop into that folder), in the
   Activity line, its test and LA-36.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it`
and `make e2e-electron`, and in CI) must keep passing.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A host test covers the page served with Dark, Light and System chosen.
- [x] A desktop test covers the window's background colour for each choice.
- [x] A server test covers a scratch link left by a dead add being removed as
  the Library opens, and one held by a live add kept.
- [x] A server test covers a failed catch-up the explorer started being in
  the activity log, and a web test covers it in the Activity list.
- [x] The spec changes (OC-8, LA-12) pass `make check`'s spec checks.
- [x] A web test covers the header showing no location while locked, and the
  refusal text reads "press Unlock" in the server and the contract fixture.

### Before merge (verified outside the check command)

- [x] On `make local` with Dark chosen on a light system (or forced through
  the browser's emulation), the agent reloads and records in the PR whether
  any light frame appears before the explorer is drawn.
