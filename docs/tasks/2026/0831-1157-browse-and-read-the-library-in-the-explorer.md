---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && ! git grep -nE \"api/(entries|image|thumb)\" -- frontend/packages"
assignee: null
branch: task/0831-1157-browse-and-read-the-library-in-the-explorer
created_at: 2026-08-31T11:57:00Z
updated_at: 2026-08-31T13:35:00Z
---

# feat(frontend): browse and read the Library in the explorer

## Overview

`coffret-server` now serves a real Library — the folder set, a folder's
children with per-file device state, and plaintext bytes fetched on demand
— but the web app is still the performance spike: it calls the removed
`/api/entries` / `/api/image/{id}` / `/api/thumb/{id}` routes, renders a
thumbnail grid the first release deliberately does not have, and hangs on
"loading library…" forever. This change replaces that spike UI with the
explorer's real first screen: a filer — folder tree on the left, file list
on the right, a status bar below — and reconnects the spike's one keeper,
the keyboard-driven reader with its forward-biased prefetch, to the real
byte route.

The shape of the screen is already decided (a filer, names only — no
thumbnails; state per file; opening a supported format shows it large and
arrows move through the folder skipping unsupported ones), so this change
is about building exactly that against the four routes the server now has.

### A gateway package for the wire contract

The web app talks to the server through hand-inlined `fetch` calls and a
hand-mirrored `Entry` type today; every DTO the server gained (listing
rows with `state` / `openable` / `container`, the JSON error shape with
its `error` kinds and `reason` values) would otherwise be mirrored the
same way, one drift per field. Add a workspace package under
`frontend/packages/gateway/` (the layer CLAUDE.md reserves for external
I/O) holding the typed client: the response and error types of
`GET /api/library`, `GET /api/folders`, `GET /api/list?path=`, and the URL
builder for `GET /api/file?path=`, plus one `fetch` wrapper that turns a
non-2xx JSON body into a typed refusal the UI can branch on (`error`
kind, `reason`, `message`). The types are written by hand to mirror the
server's serialization — there is no codegen here — so keep them in one
file per response, named after the route, where a server change has one
obvious place to land. The app package consumes only this client.

### The filer

One screen, three regions, plain React as before (no router library; the
current folder and open file live in the URL hash so reload and the back
button return to the same place — the spike lost its place on every
reload):

- **Left: the folder tree**, built from `GET /api/folders` (one flat
  sorted list; nest it client-side by `/`). Folders only, always visible,
  the current folder highlighted. The Library has folders only where
  current Entries stand, so an empty Library shows an empty tree, not an
  error.
- **Right: the current folder's children** from `GET /api/list?path=` —
  sub-folders first, then files, in the server's order (it is EP-3 byte
  order; do not re-sort, do not case-fold). Columns stay minimal: an icon
  (folder / image / other file, by `openable` and `content_type`), the
  name, the size, the modification time, and a state chip — `present`
  (the file is on this device) or `remote` (not yet fetched). Every
  stored file appears by name whatever its format; a non-openable file
  opens nothing but shows a short "preview of this format is not
  supported yet" notice when activated.
- **Bottom: the status bar** — the Library's name and provider from
  `GET /api/library`, and a transient line while a fetch is in flight
  ("fetching <name>…"). Nothing else; there is no management screen in
  the first release.
- **Loading / empty / error are all states the screen can leave**: the
  initial loads show a loading state, a failed request shows the
  refusal's `message` with a retry button (the spike's silent forever-
  loading is exactly what this replaces), and an empty folder says it is
  empty.

### The reader

Activating an openable file opens the reader over the list: the image
large, `←` / `→` moving through the **openable** files of the current
folder (skipping non-openable ones — the decided behavior), `Escape` (or
a click) closing back to the list with the selection on the file that was
open. The mechanics to keep from the spike are `layout.ts`'s pure
`prefetchTargets` policy (radius 3, forward-biased, bounds-clamped) and
the decoded-image cache keyed by index — re-point them at
`/api/file?path=` URLs over the openable subsequence. The bytes come with
`Cache-Control: private, no-store`, so the in-memory `Image` map is the
only cache; that is intentional (the browser must not spill plaintext to
disk cache) — say so in a comment where the map lives.

A `remote` page is fetched by the same `GET /api/file` the reader already
issues; while it is in flight the reader shows a placeholder with the
file's name and a progress hint, and a failed fetch shows the refusal
message with a retry — both leaveable states, never a stuck screen. When
a fetch succeeds the file is now on this device: flip that row's chip to
`present` (refresh the folder listing rather than guessing client-side).

The spike's `GridView`, its `Hud`, and the grid-layout helpers in
`layout.ts` that only the grid used go away with the routes they called;
`columnCount` / `rowCount` / `rowItems` die with `GridView`, while
`prefetchTargets` stays. The page-turn timing measurement can go too —
the acceptance target lives in the roadmap's benchmark scenario, not in a
permanent HUD.

### The one server-side addition: `mapped`

The listing cannot currently say "this folder is not on this device" —
the walkthrough that found this: a user opens an unmapped folder, sees
ordinary `remote` rows, clicks one, waits out a Storage round trip, and
only then gets `409 unmapped`. The device layer knows the answer before
any click (`mappings()` + EP-9: a top-level mapping claims its subtree,
the root mapping the remainder). Add a `mapped: bool` to the listing —
on the listing itself for the requested folder, and on each child folder
row (a child can differ from its parent only at the top level) — wire it
through `FolderListing` / `ChildFolder` in `coffret-device`'s `browse`
and the server's `ListingDto`, and document the field beside the others.
The UI then renders an unmapped folder's contents with a banner ("this
folder is not on this device — map it with `coffret map` to fetch
files") and disables activation of its rows, instead of letting the user
walk into the 409. Device-layer and router tests cover a mapped and an
unmapped folder each.

### Tests

The new pure logic gets vitest cases beside the existing `layout.test.ts`
style (node environment, no DOM): nesting the flat folder list into a
tree; the openable-subsequence navigation (skip, clamp at both ends);
`prefetchTargets` staying as-is; the hash ↔ view-state round trip; the
client's refusal parsing (a 409 body with `reason` becomes the typed
refusal, a non-JSON 500 does not throw the parser). Component/DOM test
infrastructure is deliberately not introduced here — the pure seams are
where the logic lives, and the manual criteria walk the composed screen.
Backend: the `mapped` field's device and router cases as above.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes — including the new gateway package's tests, the
      reworked app package building with no unused spike code, and the
      backend `mapped` cases — and no file under `frontend/packages`
      mentions the removed spike routes (`git grep` for
      `api/(entries|image|thumb)` is empty; appended to `check_command`).
- [x] The web app's only workspace dependency for server I/O is the new
      gateway package (no raw `fetch` of `/api/` URLs left in
      `packages/apps/web` outside it).

### Manual / on-hardware (verified by a human before merge)

- [ ] Against the round-trip state (`COFFRET_STATE_DIR=.tmp/drive-round-trip/state
      make server LIBRARY=second` in one terminal, `make web` in another,
      browser at `localhost:5173`): the tree shows `runs`, opening a run
      folder lists its files with size / time / `present` chips, clicking
      an image opens the reader, `←` / `→` page through the images
      without visible lag, `Escape` returns to the list, and reloading
      the browser comes back to the same folder.
- [ ] The status bar names the Library, and stopping the server turns the
      screen into the error state with a working retry once the server is
      back (not a silent hang).

## Out of scope

- Background fill of an opened Pack and the `fetching` / `failed` chips it
  drives from the server side (the next change); in this one, transient
  fetch state is purely the browser's own request lifecycle.
- Drag-and-drop upload (the change after).
- Thumbnails, derived Entries, and any grid view.
- Serving the built bundle from `coffret-server` (dev stays on the Vite
  proxy).
- Component/DOM test infrastructure (jsdom); pure-function tests only.
- `Content-Disposition` for non-openable downloads (ledgered separately).
