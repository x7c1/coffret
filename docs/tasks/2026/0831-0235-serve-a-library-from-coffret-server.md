---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, user-experience, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0831-0235-serve-a-library-from-coffret-server
created_at: 2026-08-31T02:35:00Z
updated_at: 2026-08-30T20:40:00Z
---

# feat(backend): serve a Library's folders and files from coffret-server

## Overview

`coffret-server` is still the viewer spike: it `walkdir`s a folder of plain
images, numbers them by position in a sorted `Vec`, and serves those numbers
as ids. It has never seen an Index, a Container, or a key. Meanwhile
`coffret-device` exists precisely so that the explorer and the CLI run the
same flows over the same device state — its own crate doc says the explorer
is "a shell over this" too — and the CLI now runs a full round trip on real
Storage. This change makes the server the second shell.

After it, `coffret-server --library <name>` opens the Library the CLI set up
(same `settings.json`, same `master-key.cfmk`, same `index.sqlite`, same
`COFFRET_STATE_DIR`), asks for the Passphrase once at startup, and serves
three things to the browser, all keyed by Entry Path: the folder tree, a
folder's immediate children, and an Entry's plaintext bytes — read from the
mapped folder when this device already has the file, fetched into it first
when it does not. Nothing else: the browser never sees a key, a ciphertext,
or a token, and Storage is never contacted for a listing.

The web app is not touched here (it still calls the spike's routes and shows
its loading state until the next change reconnects it). Background fill of
an opened Pack and the drag-and-drop upload receiver are also later changes;
this one is the read path and the device-layer API both of those build on.

### The device layer grows a read side

`coffret-device` today exposes one-shot flows (`run_sync`, `run_fetch`,
`run_fetch_entry`) that each open the Library — settings, Argon2 unlock,
store build, SQLite open — and drop it. A server opens once and answers
thousands of requests, so the crate gets an API over an already-open
Library, and the one-shot flows become thin wrappers over it (the CLI keeps
working unchanged):

- `open_library` stays the entry point; the flows sync / freeze / fetch /
  fetch-entry each gain a form that takes `&OpenLibrary` (name it as the
  crate's naming has it — the point is one body per flow, called by both the
  wrapper and the server).
- **Browsing** — a listing of one folder's immediate children, and the set
  of all folders, both derived from the Index alone. The Index has no folder
  concept and no "children of" query: `entries_under(prefix)` is a range
  scan of the whole subtree (`path_prefix::subtree_range`). Derive the
  children in the device layer from that scan — a child folder is the next
  `/`-separated component after the prefix, a child file is an Entry whose
  path has no `/` past the prefix. Order is EP-3 order (byte order of the
  canonical path, no case folding, no locale); the scan already returns it.
  Do not add an `Index` method for this yet: the doc's own measurement
  (3,300 rows in 24 ms) says a scan is fine until tens of thousands of
  Entries, and a new port method would need its SQLite implementation and a
  conformance case for a speedup nobody has measured. Say so in the doc
  comment, so the next person knows where the seam is.

  A file row carries what the design calls "display-format-independent
  metadata": path, name (last component), size, mtime, and the device's
  state for it — `present` when `local_entry_at` records a present
  materialization, `remote` otherwise (EP-10: a mapping asserts nothing; a
  present record is the only claim). It also carries the Container kind
  (single-file or Pack) since the upload receiver will refuse same-name
  drops onto Pack-resident Entries by exactly that bit, and putting it in
  the row now keeps that change from touching the listing.
- **Where an Entry lives on this device** — the mapping translation (EP-9)
  that `fetch::translate` performs is `pub(super)` inside the usecase's
  `fetch` module. The server needs the local path of an Entry to read it
  after a fetch and to read it when it is already present. Expose that
  translation as public usecase API rather than re-deriving EP-9 in the
  device layer; whether that is a `fetch::local_path_of(index, &path)` or
  `EntryFetch` carrying the placed path is the implementer's call, but the
  rule lives in one place either way (`coffret-device`'s crate doc makes
  this promise — keep it).
- **`fetch_entry` on an open Library** — the existing range-read fetch of
  one Entry (`FetchEntryRequest`), placed under EP-11 into the mapped
  folder. Two requests for the same remote Entry arriving together — the
  reader's prefetch does exactly this — must not both place it: the scratch
  file is named by Container id (`scratch::name`), so two concurrent
  placements of one Container collide on disk. Single-flight per Entry Path
  in the device layer (the second caller waits for the first's verdict).

### The Index survives a second process

`SqliteIndex::open` sets no `journal_mode` and no `busy_timeout`, and its
doc comment assumes one process. A server holding the Index while the user
runs `coffret sync` in a terminal is now the normal case, not a mistake:
a `sync` write transaction would make every server read fail with
`SQLITE_BUSY` at once, and vice versa. Set `journal_mode = WAL` and a
`busy_timeout` (seconds, not milliseconds — a fetch's `mark_present`
waiting behind a sync commit is fine; failing is not) in `open`, and
rewrite that doc comment to say what the arrangement now is: one connection
per process, WAL so readers and one writer coexist across processes, the
busy timeout as the wait. The `index_conformance` suite runs over the
SQLite implementation as before; add one case in the gateway's own tests
that opens the same file twice and reads on one while a write transaction
is open on the other.

### The server

Replace the spike wholesale. Nothing of `walkdir`, the thumbnail cache, the
`image` dependency, `--thumbs`, or the `/api/entries` / `/api/image` /
`/api/thumb` routes survives; the thumbnail supply comes back with derived
Entries in a later release and is designed elsewhere. `git` keeps the spike.

- **Startup**: `coffret-server --library <name> [--port 8787]`. Logging
  starts first, the same way the CLI's `logging.rs` does it — do not copy
  that file; move it (and the Passphrase prompt) somewhere both binaries
  reach, since two copies of "how a coffret process starts" will drift.
  Then `open_library` with a terminal Passphrase prompt (`--passphrase-stdin`
  for scripts, as the CLI has it), then bind `127.0.0.1:<port>` only. One
  process is one unlock (DK-9); the derived keys live for the process. A
  Library that is not on this device, a wrong Passphrase, an expired grant:
  the same refusals the CLI prints, exit 1, before anything is bound.
- **Routes**, all under `/api`, all taking the Entry Path as a query
  parameter (`?path=…`) so `/` never needs escaping in a URL segment. Every
  incoming path is outside text: `EntryPath::nfc`, then EP-2 shape rules;
  a path that fails them is `400` with a JSON body saying which rule. An
  empty or absent `path` means the Library root where a folder is expected.
  - `GET /api/library` — `{ name, library_id, provider }`, for the status
    bar. No epoch, no folder id, no bucket name: nothing the browser has a
    use for.
  - `GET /api/folders` — every folder path in the Library, sorted, flat.
    The tree pane nests them.
  - `GET /api/list?path=<folder>` — `{ path, folders: [{ name, path }],
    files: [{ name, path, size, mtime, state, container, openable, content_type }] }`.
    `mtime` is the Entry's own modification time as ISO 8601 UTC. `state`
    is `present` or `remote` (the transient `fetching` / `failed` states of
    the design's model are the browser's, not the Index's). `openable` and
    `content_type` come from **one classifier module in the server**, keyed
    by the path's extension: the browser image types (`jpg`/`jpeg`, `png`,
    `webp`, `gif`, `avif`) are openable with their media type; everything
    else is not openable and is served as `application/octet-stream`. The
    Index's `mime` column is `None` for every row today — nothing fills it
    at scan — so the classifier does not consult it; say so in its doc
    comment, since filling it is the obvious later improvement. Adding a
    format must be a one-line change to that table.
  - `GET /api/file?path=<entry>` — the plaintext bytes with the classifier's
    content type, `Cache-Control: private, no-store` (the bytes are the
    user's plaintext; the spike's `public, max-age=86400` is wrong for
    them). Present on this device → read the file at its translated local
    path. Not present → `fetch_entry` first, then read what it placed.
    Refusals are JSON, with status codes the browser can branch on:
    `404` when no current Entry has that path; `409` with `reason` when the
    fetch was declined — `unmapped` (EP-9: no mapping covers the path,
    which the design says the explorer surfaces as "this folder is not on
    this device"), `surfaced` with the `Surfaced` variant name, `locked`
    (KL-7: the Container's key is lost), `unmaterializable` (EP-2/EP-4:
    no file can stand at that path on this device); `502` when Storage
    failed. A
    Container that fails authentication is `502` too — the bytes never
    reached disk (EP-11), and there is nothing the browser can do about it.
    A file served is served whole; Range requests are not honoured (a
    browser `<img>` does not send them).
- **Errors on the wire** are one JSON shape — `{ error: <kind>, message,
  ...detail }` — from one place, mapped from the device layer's `Error`.
  No Rust `Debug` output in a body. The `500` fallback logs the chain and
  says only that the server failed.
- **Logging** follows the crate's rule: events carry no Entry Path, no
  local path, no key material. Log the operation, the outcome kind, sizes
  and durations.
- `make server` runs the new binary: `LIBRARY ?= main`, passing
  `COFFRET_STATE_DIR` through untouched, so pointing it at the round-trip
  target's state (`.tmp/drive-round-trip/state`) serves what that target
  built. `make web`'s help text stops calling it the spike.

### Tests

The router is testable without a socket (`tower::ServiceExt::oneshot`) and
without Storage: build an `OpenLibrary` from the usecase's in-memory store
and index (its fields are public for this reason), commit a few Entries
through the real sync flow into a temporary mapped folder, and drive the
routes. Cases the change needs at minimum: a folder listing with both a
child folder and child files in byte order; `state` flipping from `remote`
to `present` after `/api/file` fetched it; the same file requested twice
concurrently placed once; every refusal above with its status and `error`
kind; a non-NFC spelling resolving to the same Entry as its composed
form (EP-1: outside text is normalized in, never refused for
composition) and a `..`-bearing path refused as `400`; a non-openable
extension listed with `openable: false` and served as octet-stream. The
device-layer browsing and single-flight get their own unit tests beside the
code, and the SQLite two-connection case lives in the gateway crate.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes, including the new `coffret-server` router tests,
      the `coffret-device` browsing / single-flight tests, and the
      `coffret-sqlite-index` two-connection case.
- [x] `coffret-server` depends on `coffret-device` and not on
      `coffret-usecase`, `coffret-sqlite-index`, `s3-store`, or
      `google-drive-store` (the `deps` target's gateway rule stays green;
      add the app-side assertion to it: an app binary reaches the domain
      only through `coffret-device`).
- [x] The CLI's behaviour is unchanged: the `s3-store-it` device-layer cases
      run in CI as before, and `cargo run -p coffret-cli -- --help` still
      lists every subcommand.

### Manual / on-hardware (verified by a human before merge)

- [ ] `COFFRET_STATE_DIR=.tmp/drive-round-trip/state make server LIBRARY=second`
      asks for the Passphrase once, then `curl 'localhost:8787/api/list?path=runs'`
      lists that Library's run folders and `curl -o /dev/null -w '%{content_type}'
      'localhost:8787/api/file?path=<one image path>'` returns an image type,
      with no request to Google Drive in the log.
- [ ] While that server is running, `coffret sync --library second` in
      another terminal completes (exit 0 or 2, not an Index error), and the
      server keeps answering listings during and after it.

## Out of scope

- The web app: the filer-type tree / list / status chips and the
  `ReaderView` reconnection are the next change.
- Background fill of an opened Pack (the design's "prefetch mechanism fills
  the rest") and the transient `fetching` / `failed` states it drives.
- The drag-and-drop upload receiver (`POST` multipart into a mapped folder,
  then sync).
- Serving the built frontend bundle from the server (dev uses Vite's proxy
  as before).
- Filling `EntryMetadata.mime` at scan time; thumbnails and derived Entries.
- Idle auto-lock of the process's keys (DK-4, deferred as before).
