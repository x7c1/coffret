---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "/api/browse" backend/crates/apps/coffret-server/src/router.rs && grep -q "/api/map" backend/crates/apps/coffret-server/src/router.rs && grep -rq "/api/browse\|browse" frontend/packages/gateway/api/src && test -f frontend/packages/apps/web/src/mapping.ts && ! grep -q "coffret map" frontend/packages/apps/web/src/FileList.tsx'
assignee: null
branch: task/1006-2028-map-a-folder-on-this-device-from-the-explorer
created_at: 2026-10-06T20:28:40Z
updated_at: 2026-10-07T03:51:41Z
---

# feat: map a folder on this device from the explorer

## Overview

A Library's folders live on this device only where a mapping says so (a
mapped root per Library root or per top-level folder, spec: EP-9, CK-7). Today
the explorer tells a person a folder "is not on this device — map `<top>` with
`coffret map`", and the mapping itself is a terminal command, after which the
person comes back to the page. The device already has everything the page
needs — `coffret-device::set_mapping` records a mapping, writes the root's
marker (`.coffret/root`, the identity a later placement is checked against)
and the server reads mappings per request, so a mapping recorded while it runs
is seen on the next listing — so this moves the gesture into the explorer. A
browser cannot hand a page a real path on the device, so the folder is chosen
through the server: it lists the device's folders, the page browses them or
takes a typed path, and the server records the mapping.

**1. `GET /api/browse?path=<absolute>` lists folders on this device.** A keyed
route (spec: LA-2) that answers the folders directly under `path` — names and
absolute paths, sorted by name, dot-folders left out — plus the path itself and
its parent (`null` at the filesystem root). With no `path`, the person's home
directory. A path that does not exist or is not a folder is a `400` refusal
naming it; one the account cannot read is `403`. Only folders are listed, never
files, and nothing is followed outside what the path names (no symlink
walking beyond the one listing). Tests over a temporary tree: the listing's
shape, the dot-folder rule, the parent at the root, both refusals.

**2. `POST /api/map` records a mapping.** Body `{ "local_root": "<absolute>",
"prefix": "<top-level folder>" | null }` — `null` maps the Library root, as
`coffret map` without `--prefix` does. The route calls
`coffret_device::set_mapping` with `MarkerRequest::AdoptWhatIsThere` (what the
CLI does without `--reset-marker`), and answers what `coffret map` prints: the
mapping now recorded and, if one was replaced, where it was before. Refusals
follow the device crate's: a path that is not an existing directory, a prefix
that is not a valid top-level Entry Path. `--reset-marker` has no counterpart
here; a root whose marker does not match stays a CLI matter and the route
answers the device crate's refusal for it. The route does not need the Master
Key (a mapping is the device's record, not the Library's data) — decide whether
it is admitted while the Library is locked and document why either way. A
routes test records a root mapping over a temporary folder and sees the next
`/api/list` answer `mapped: true`; another maps a prefix; another sees the
replaced mapping reported.

**3. The explorer offers the mapping where the banner is.** In
`FileList.tsx`'s banner for a folder that is not on this device, and the one
for an unmapped root, replace "map `<top>` with `coffret map`" with one button:
*map this folder…* (or *map the Library root…*). It opens a small picker: the
current path, its folders as a list to descend into, a parent link, and a text
field holding the current path that the person can also type into; one button,
*map here*, calls `POST /api/map` with the path and the banner's prefix (or
`null` for the root), then reloads the listing so the banner goes and the
folder's files show their state. A refusal is shown in the picker and it stays
open. Keep the picker's state and the browse/map calls in a module free of DOM
(`mapping.ts`, in the style of `reconnect.ts`) with unit tests: descending,
going to the parent, a typed path replacing the browsed one, the mapped prefix
carried from the banner, the refusal kept. The api package gains the two calls
and their answer shapes in the style of `reconnect.ts`. `make deps` must find
both routes served.

**4. Words.** The banners stop naming `coffret map`. The CLI's `map` and
`mappings` are unchanged. `docs/guides/environments.md`'s line "The Libraries
either one lists are made with `make cli ARGS="init …"`…; the shell does not
create or join one" stays; add nothing there about mapping unless it already
says mapping is a CLI matter (then correct it).

Spec: no rule changes. EP-9 / CK-7 (mappings are the device's), the marker
rules the device crate already enforces, LA-2 for the routes.

Out of scope: in-place unlock, the reset of a mismatched marker, mapping a
folder below the top level (the mapping model has none), a native folder
dialog in the desktop shell (the server-driven picker works in the browser and
the shell alike).

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `GET /api/browse` is a keyed route listing folders under a path (home by default) with the parent, dot-folders hidden, and `400`/`403` refusals, with routes tests over a temporary tree
- [x] `POST /api/map` is a keyed route that records a root or prefix mapping through `set_mapping`, reports a replaced mapping, refuses a non-directory, and is followed by `/api/list` answering `mapped: true`, with routes tests
- [x] The explorer's unmapped banners offer the picker instead of naming `coffret map`; the picker's state module has unit tests for descending, parent, typed path, prefix and refusal
- [x] Both routes are in the api package and pass `make deps`
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Under `make dev` for the development Library (a person starts it), in a fresh top-level folder made with "new folder", the banner's button opens the picker, browsing to a folder on this device and pressing *map here* makes the banner go and a drop into the folder is accepted — an agent runs this with the development pair up, or reports the exact lines otherwise
