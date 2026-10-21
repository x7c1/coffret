---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1022-0301-start-a-new-library-from-the-explorer
created_at: 2026-10-21T18:01:45Z
updated_at: 2026-10-21T19:07:17Z
---

# feat(explorer): start a new Library from the explorer, and prepare a folder's path before dropping into it

## Overview

A Library that was just `init`ed cannot be started from the explorer. The first book needs the command line's `map` and `sync`. Three things are in the way:

- **No map banner at an empty root.** The root of an empty Library shows no map banner. `sayUnmapped` in `frontend/packages/apps/web/src/FileList.tsx` keeps the banner back at a root that holds no files, because a root with no mapping and only folders is the ordinary shape of a device that mapped one top-level folder. An empty Library has neither files nor folders, so nothing offers to map it.
- **Empty prefixes are invisible.** A top-level folder mapped on this device (`coffret map --prefix …`) that holds no Entry yet does not appear in the listing, since the listing shows Entry Paths. There is nowhere to drop.
- **No way to prepare a path.** A Library cannot hold an empty folder, so there is no way to make the path a drop should land under. A long series, hundreds of volumes, cannot go in one drop: the count of files one drop may carry is bounded (LA-9, LA-28). The person has to make the series' parent path first and drop a few volumes at a time. Today the only way is to build the parent folder in the file manager and drop that.

### Decisions (already made; implement them)

1. **Offer to map an empty root.** Where the Library root is not mapped on this device and its listing holds neither files nor folders, show the existing unmapped banner with its "map the root" button (the `Unmapped` component in `FileList.tsx`). Keep the banner back, as today, at a root that holds only folders.
2. **"New folder…" prepares a path; it makes nothing.**
   - Add a "New folder…" item to the "⋮" menu of every folder row, both in the file list and in the left tree (`RowMenu`). The tree's root row, "Library", gets the item too. In an empty Library that is the one row there is, and the way to prepare a path right under the root.
   - The item asks for one name in a small dialog. It refuses, before anything is shown, a name the Library would refuse as an Entry Path segment: empty, containing `/`, a reserved name, or a name the same folder already holds. Reuse the existing name rules (the frontend's format/domain package or the API's) rather than writing new ones.
   - The prepared path is held by the page only. It is not sent to the server, and no directory is made on disk. Until something is dropped into it, it appears in the tree and the list as a folder marked "not in the Library yet — drop files here to add them". A reload or restart forgets it, and preparing it again is the remedy.
   - Opening it shows an empty listing that a drop can target. The drop goes through the same confirmation as any drop (x7c1/coffret#306), which already marks the levels the Library does not have yet.
   - Paths can be prepared inside a prepared path (`comics/Series/`).
   - Once a drop lands, the path is a real Entry Path and the mark goes.
   - The item is offered only where a mapping on this device reaches the folder; elsewhere a drop would be refused. On the root without a mapping it is left out, and the banner from decision 1 says what to do.
3. **List an empty mapped prefix.** At the root, show every top-level folder this device maps that holds no Entry yet, with the same "not in the Library yet" mark. It can then be opened and dropped into. Mappings are device state the server already has (`mappings`), so the listing gets them from the server: extend the root listing's answer or the folders answer, whichever fits. Never show another device's mappings.
4. **No sync control.** The explorer gets no control for running a sync. A drop already starts one, and a file placed straight into a mapped folder through the file manager stays a command-line matter.
5. **Docs.**
   - Update the concept docs where they describe what the explorer lists or say the Library has no empty folders, so they also cover a prepared path and an empty mapped prefix: `docs/concepts/library/README.md` (Browsing surface, and the line on naming a folder rather than drawing it).
   - Update `docs/guides/environments.md` if it describes starting a Library.

### Out of scope

- An explorer control for sync.
- Mapping a top-level folder from the explorer anywhere other than through the existing banner.
- Keeping a prepared path across a reload.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Web vitest, map banner: an unmapped root with neither files nor folders shows the unmapped banner with "map the root". An unmapped root with only folders does not. An unmapped root with files does, as before.
- [x] Web vitest, offering "New folder…":
  - the item is in the "⋮" of a mapped folder row in the list and in the tree, and of the tree's root row when the root is mapped;
  - it is absent where no mapping reaches the folder.
- [x] Web vitest, preparing a path:
  - "New folder…" refuses an empty name, a name with `/`, a reserved name and a name the folder already holds, and prepares nothing;
  - a valid name adds a row marked "not in the Library yet" to the list and the tree;
  - a path can be prepared inside it;
  - opening it shows an empty listing.
- [x] Web vitest, dropping into a prepared path: the drop is confirmed with the prepared levels marked new, goes to the prepared path (`/api/upload` or `/api/import` with that path), and after the listing is read again the row is an ordinary folder.
- [x] A server test and a web vitest cover empty mapped prefixes: a top-level folder this device maps with no Entry under it appears in the root listing with the "not in the Library yet" mark, and a mapped prefix that does hold Entries appears as an ordinary folder.
- [x] `make spec-citations` passes after the concept-doc edits.

### Before merge (verified outside the check command)

- [ ] On macOS, with the Electron app (`make desktop-dev`) driven by Playwright's `_electron`, over a throwaway Library made on the agent's MinIO with nothing mapped:
  - the empty root shows the map banner, and mapping the root through it lets the explorer list the root;
  - "New folder…" from the tree's "Library" row prepares `comics`, and "New folder…" on it prepares `comics/Series`;
  - a real-path drop of a folder of 3 volumes onto `comics/Series` is confirmed and lands, and the explorer then lists `comics/Series/…` as ordinary folders;
  - a reload forgets a prepared path nothing was dropped into.

  The merging session records this in the PR.
