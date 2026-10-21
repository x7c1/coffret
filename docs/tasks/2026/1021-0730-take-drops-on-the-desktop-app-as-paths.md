---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "getPathForFile" frontend/packages/apps/desktop/src/ && grep -rq "api/import" backend/crates/apps/coffret-server/src/ && ! grep -rqE "nodeIntegration: true|contextIsolation: false|sandbox: false" frontend/packages/apps/desktop/src/'
assignee: null
branch: task/1021-0730-take-drops-on-the-desktop-app-as-paths
created_at: 2026-10-20T22:30:02Z
updated_at: 2026-10-21T00:06:48Z
---

# feat(desktop): take drops onto the explorer window as paths, walking folders on the Rust side

## Overview

Today a drop onto the explorer goes through the browser. `frontend/packages/apps/web/src/drop.ts` walks the dropped folders with the File System Entry API and sends each file's bytes as a multipart part to `POST /api/upload`. The server writes each part to a scratch file and then into the mapped folder (`backend/crates/apps/coffret-server/src/routes/upload/`). A book of several hundred megabytes is copied over HTTP before it lands.

In the Electron app the explorer runs in the app's own window, so the drop can be taken as filesystem paths instead. The Rust side walks folders on disk and copies from the source file directly. The explorer keeps its confirmation screen and Pack choice; only where the list comes from and how the files arrive change. The browser development flow (`make dev`, the dev host) keeps the multipart upload as it is.

### Decisions (already made; implement them)

1. **Who walks.** The explorer window gets a preload: its first, with `contextIsolation`, `sandbox` and its own partition unchanged.
   - The preload exposes one function through `contextBridge`. It takes the dropped `File`s, maps each to `webUtils.getPathForFile`, and invokes main over IPC with only the top-level paths.
   - Main accepts the IPC only from the explorer window's webContents and passes the paths to a new addon export. Rust walks them:
     - files under a dropped folder, depth first;
     - symbolic links neither followed nor taken;
     - names the Library refuses (the scratch prefix, the management area) reported as refused rather than taken;
     - an empty or unreadable path reported, not silently dropped.
   - Rust holds the walked set under a one-time **drop handle**: random, short-lived (minutes), at most a few held. The answer to the renderer is the handle plus, per file, its path relative to the dropped item's parent, its size and its mtime. No absolute path is returned to the renderer.
   - Expose the walk through `coffret-device` (or the layer `make deps` allows apps to use). Build it on the confined, no-follow listing the mapped-roots port already has, rather than adding a new unconfined walker.
2. **What the explorer does with it.** `drop.ts` / `FileList.tsx` / `FolderTree.tsx` use the bridge when `window.coffretDesktop` is present, and the File System Entry walk otherwise.
   - `Added` becomes a record of `{ path, size, mtime }` plus where its bytes come from: a `File` (browser) or a drop handle and index (desktop).
   - `packChoice.ts`, `destination.ts`, `renumber.ts` and `DropConfirm` keep working on that record; they read only path and size today. Renumbering keeps working: the final relative names are what the import names.
3. **How the files arrive.** A new route `POST /api/import` sits behind the same host fences as every `/api` route. Its body is JSON: the target folder, `freeze`, the drop handle, and for each taken file its index in the handle's set and its final relative name.
   - The server opens each source file from the handle's set and streams it into `OpenLibrary::add_file`. Generalize the receive path in `routes/upload/receive.rs` over an async reader, so room checks, name and fence refusals, the per-file write and `keep(mtime)` stay one implementation.
   - It reserves for a book freeze before the first file exactly as the upload route does (PK-17). It answers in the upload route's answer shape, so the explorer's notices and follow-up are shared.
   - A handle is used once. An unknown, used or expired handle is refused. An index outside the set is refused. No absolute path is ever accepted over HTTP.
   - mtime comes from the source file's metadata, and so does birth time where the platform gives one. Decide whether to keep `btime` in the Entry, as the CLI's local scan does, and say which in the PR.
4. **Folders the CLI and the drop share.** If the CLI's local scan already has the "which names a Library refuses" rule, use the same function. Do not duplicate it.
5. **Spec.**
   - Add the import rules to `docs/spec/loopback-access/README.md` as new IDs after the door's: the drop handle, one use, expiry, index-only addressing, no absolute paths over HTTP, and symbolic links not followed.
   - Rescope LA-9–LA-11 to say they bound the multipart upload route. State what bounds an import: the room check per file, the count per request, and the handle's set.
   - Update EP-11's sub-bullets that describe "the upload route the browser drops a file into" so they also cover a file imported by path (mtime from the source, birth time if kept). Update FM-9's note about a dropped file having no birth time if that changes.
6. **Logs.** Events never carry an absolute path, a dropped file's name, or the handle (EL-1). Counts and refusal kinds only.

### Out of scope

- Tray and residency.
- Packaging.
- Removing Tauri.
- Removing the multipart upload route, which the browser development flow still uses.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The desktop preload uses `webUtils.getPathForFile` (gated), and every window keeps `contextIsolation`, `sandbox` and no `nodeIntegration` (gated).
- [x] Unit tests cover the IPC handler: it accepts only from the explorer window's webContents, and refuses an empty or non-string path list.
- [x] Rust walk tests cover:
  - a nested folder, in depth-first order;
  - an empty folder, which contributes nothing;
  - a symbolic link to a file, and one to a folder, both neither followed nor taken and reported;
  - a scratch-prefixed name and a management-area name, refused;
  - a folder with more than 100 entries, all taken;
  - a dropped single file;
  - an unreadable path, reported.
- [x] Drop handle tests cover:
  - one use;
  - expiry;
  - the cap on held handles;
  - the answer carries no absolute path.
- [x] `POST /api/import` route tests (`routes/import` or alongside upload), ported in spirit from the upload route's:
  - written and refused files;
  - a book freeze reserved before the first file and released on a stop (PK-17);
  - the source mtime kept;
  - no partial file left on a mid-file failure;
  - unknown, used and expired handles refused;
  - an out-of-set index refused;
  - a body naming an absolute path refused;
  - the route behind the host's fences.
- [x] Web vitest covers the bridge path: a drop with `window.coffretDesktop` present builds `Added` records from the bridge's answer, goes through the same Pack choice, and sends to `/api/import`. Without the bridge, the existing multipart path is unchanged.
- [x] Events carry no absolute path, no file name and no handle (an EL-1 test).
- [x] The new and rescoped rules pass `make spec-rule-ids` and `make spec-citations`.

### Before merge (verified outside the check command)

- [x] On macOS, the Electron app (`make desktop-electron-dev` over the agent Library), driven with Playwright's `_electron`, takes a drop dispatched as a real-path drag through CDP's `Input.dispatchDragEvent`:
  - a drop of two JPEGs lands both in the mapped folder, with the source mtime;
  - a drop of a folder holding a 20-page book shows the confirmation with the Pack choice, and adding it as a book freezes it into one Pack;
  - no request body carries the files' bytes; this is visible in the request log.
  The merging session records this in the PR.
