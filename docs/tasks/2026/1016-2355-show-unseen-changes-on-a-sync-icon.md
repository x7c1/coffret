---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, user-experience]
max_refine_rounds: 2
retries_remaining: 1
check_command: "make check && ! grep -rqF --exclude='*.test.ts' --exclude='*.test.tsx' 'look for what is new' frontend/packages/apps/web/src"
assignee: null
branch: task/1016-2355-show-unseen-changes-on-a-sync-icon
created_at: 2026-10-16T14:55:14Z
updated_at: 2026-10-16T16:29:36Z
---

# feat(explorer): turn "look for what is new" into a sync icon that shows when the Library has changes this device has not caught up with

## Overview

The status bar's "look for what is new" button (`frontend/packages/apps/web/src/StatusBar.tsx`, label `ASKING` in `refresh.ts`) runs a catch-up (`POST /api/refresh` → `run::catch_up` → `commit::catch_up`) when pressed, but nothing tells a person that there is something to catch up with: the explorer never looks at the remote head. Replace the text button with a sync icon that says when another device has committed.

- **Server: a names-only check.** Add a read-only route (e.g. `GET /api/remote-head`) answering whether Storage holds a newer head than the catalog: the newest `head-*` generation from a `ControlListing::read(...)` (`domain/coffret-usecase/src/commit/control_listing.rs`; `newest_head()` is `pub(super)` today) compared with `index.checkpoint().head_generation()`. It reads object names only — no object body, no Keyring, no key — and does not take the run turn or change the catalog. Remember the listing in `FetchMemory` the way `catch_up_catalog` does, so the next fetch or catch-up can reuse it. While the Library is locked it answers as the other read routes do. Name the outcome in the answer (`newer` / `current` — not `behind`, which LA-12 already uses for a catch-up that did not finish), not a raw generation pair, unless the explorer needs the numbers.
- **When the explorer asks.** On load, when the tab comes back to the front (the existing `visibilitychange` / `focus` handling in `App.tsx`), and every few minutes while the tab is visible (pick an interval, e.g. 3 minutes, as a named constant with its reason). Never while a catch-up is running. A failed check is not shown as an error banner; the icon keeps its last state and the next check tries again (log the failure on the server as the other routes do).
- **The icon.** An inline SVG sync icon in the status bar (both themes, `currentColor`), `title` / `aria-label` "Check for changes". When the last check said `newer`, it is drawn in an accent color with an accessible label saying changes are waiting. Pressing it runs the existing catch-up (`askWhatIsNew`), during which it spins or otherwise shows progress (reuse the current "looking…" state); after a catch-up lands it returns to the plain state. The banner sentence in `catalogLine` that names the button ("Press "…" to try again") is reworded to name the icon.
- **Spec and concepts.** If a loopback-access or library rule lists the server's routes or says the explorer never polls the remote head (see the `routes/refresh.rs` doc), update it to describe the names-only check. Describing present behaviour is in scope; do not change what a catch-up does.

Out of scope: catching up automatically, push notifications, showing what changed.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A server test shows the new route answering `newer` after another writer commits a head and `current` after a catch-up, without reading any object body (a store wrapper that counts reads proves only `list` was called).
- [x] The route does not take the run turn and leaves the catalog unchanged (test).
- [x] Component tests show the icon in its plain and its changes-waiting state, with the "Check for changes" label, and pressing it calls the catch-up.
- [x] A unit test shows the explorer asks on load, on returning to the front, and on its interval, and not while a catch-up runs.
- [x] No non-test source in the web app still says "look for what is new" (a grep appended to the check).

### Before merge (verified outside the check command)

- [ ] Against the agent's two-device MinIO Library in Playwright's Chromium: after device 2 commits, device 1's icon shows changes waiting after returning to the tab; pressing it catches up and the icon returns to plain.
- [ ] The icon and its colors look right — needs a person: a visual judgement.
