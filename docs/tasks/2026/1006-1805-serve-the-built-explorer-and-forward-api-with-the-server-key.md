---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && test -f backend/crates/apps/coffret-explorer-host/Cargo.toml && grep -q "embed-web" backend/crates/apps/coffret-explorer-host/Cargo.toml && grep -rq "SERVER_KEY_HEADER" backend/crates/apps/coffret-explorer-host/src && grep -qE "^APPS := .*coffret-explorer-host" Makefile && grep -qE "^web-dist:" Makefile && grep -rq "spec: LA-3" backend/crates/apps/coffret-explorer-host/src && (cd backend && cargo build --release -p coffret-explorer-host --features embed-web --examples)'
assignee: null
branch: task/1006-1805-serve-the-built-explorer-and-forward-api-with-the-server-key
created_at: 2026-10-06T18:05:51Z
updated_at: 2026-10-06T18:31:31Z
---

# feat(desktop): serve the built explorer and forward /api with the server key from a Rust host

## Overview

The explorer reaches the server through a proxy on the device, and that proxy
is the point of the server key: the server admits a caller by a key it wrote
owner-only into the Library's directory (`backend/crates/apps/coffret-device/src/server_key.rs`,
spec: LA-3), a page in a browser cannot read that file, and the key is never
on a URL or in a cookie (spec: LA-6). Today the proxy is vite:
`frontend/packages/apps/web/vite.config.ts` serves the explorer and forwards
`/api` to the server with `x-coffret-key` read off the file per request, the
`Host` changed to the server's, and the `Origin` rewritten only when it is the
proxy's own. That ties running the explorer to node and a checkout. A desktop
shell is coming that will start the server in its own process and open the
explorer in the system browser; it needs the same proxy in Rust.

**1. A library crate `coffret-explorer-host` under `backend/crates/apps/`.** A
lib beside `coffret-shell`, which is the precedent for a non-binary crate under
`apps/`: it composes the explorer's serving side over `coffret-device` and
`coffret-server`. Its public surface is a `Config` (the server's loopback
`SocketAddr`, and the `LibraryDir` or key-file path to read the key from) and
a function returning an `axum::Router` the caller binds wherever it likes.
Add it to the Makefile's `APPS` list so `make deps` holds it to the same
boundary as the other shells (no direct dependency on a use case or a gateway;
the domain is reached through `coffret-device`).

**2. `/api/*` is forwarded to the server, streaming, with the key.** Exactly
what the vite proxy does, read it before writing this:
- method, path and query are forwarded unchanged to `http://<server>`; the
  request body is streamed, not buffered (a book dropped on a folder is tens of
  megabytes in one request), and so is the response body;
- any `x-coffret-key` the caller sent is removed, and the key currently in the
  file is set — read per request, because a server that was started again drew
  a new key and this host outlives it; a file that cannot be read forwards no
  key and lets the server refuse (do not invent a refusal here); use the
  `SERVER_KEY_HEADER` constant `coffret-server` exports rather than spelling the
  header again;
- `Host` is set to the server's address (vite's `changeOrigin`), because the
  server refuses a `Host` naming anywhere but where it is (spec: LA-5);
- `Origin` is rewritten to the server's origin only when it equals this host's
  own origin (`http://<Host of the incoming request>`); any other `Origin` is
  forwarded exactly as it arrived, so the server can refuse a page on another
  site; the `Sec-Fetch-*` headers are forwarded as they are;
- response status and headers come back as the server sent them, except
  hop-by-hop headers; redirects are not followed;
- a server that is not answering (connection refused) is a `502` with a plain
  sentence naming the address, so a person sees that the half that is wrong is
  the server, not the page. The HTTP client is yours to choose: `reqwest` is in
  the workspace already (`google-drive-store` uses it) and streams with its
  `stream` feature; a `hyper` client is also fine. Add nothing to the workspace
  that a crate in it does not already pin without a reason stated in the
  Cargo.toml comment.

**3. The built explorer is served from the binary, behind a feature.** With
feature `embed-web`, `include_dir!` embeds `frontend/packages/apps/web/dist`
(path relative to the crate's `CARGO_MANIFEST_DIR`) and the router serves it:
a file at its path with a content type by extension, and `index.html` for a
path with no file (the explorer is a single page). Without the feature — the
default, so that `cargo build` and `make check` never need the frontend built
first — `/` answers a short plain page saying the explorer was not built into
this binary. delta does the same in `delta-server` (`embed-web`); the shape
is worth a look but the code is not shared.

**4. An example binary to try it in a browser.** `examples/serve.rs` in the
crate: `--server 127.0.0.1:8787 --library books [--port 0]`, resolves the
Library's directory the way the binaries do (`COFFRET_STATE_DIR`,
`LibraryDir::resolve`), binds `127.0.0.1:<port>` and prints the URL. It exists
so the host can be tried against `make dev`'s server today and is what the
desktop shell will do in-process; keep it to argument parsing and a bind.

**5. `make web-dist` builds the explorer for embedding.** `cd frontend && pnpm
--filter @coffret/web build`, with a comment saying `embed-web` reads its
output. Document the crate in `make help` only through `web-dist`; there is no
target to run the host yet.

**6. Tests, without the feature.** Stand up a stub axum server on
`127.0.0.1:0` that answers with the headers it received, and drive the host's
router with `tower::ServiceExt::oneshot` or over a real listener:
- the key from a temporary key file is set and a key the caller sent is not
  forwarded;
- a missing key file forwards no key;
- `Host` is the server's; an `Origin` equal to the host's own is rewritten,
  one from elsewhere is forwarded unchanged;
- a multi-megabyte request body and response body arrive intact (streaming
  round trip);
- a refused connection is a `502` naming the address.
Behind `#[cfg(feature = "embed-web")]`, one test that `/` serves the embedded
`index.html` and an unknown path falls back to it, which runs when the feature
is on (the check command builds with it after `make check` built the frontend).

Write the crate-level doc comment in the style of the other crates: what it is
for, why the key is attached here and not in the page (cite `spec: LA-3` and
`spec: LA-6`), and that it is the vite proxy's twin. Spec citations use the
`spec:` prefix the `spec-citations` check expects.

Out of scope: the desktop shell itself, any change to `vite.config.ts` or
`scripts/dev.sh`, and serving the explorer from `coffret-server`'s own port
(rejected: the server would then admit its own page without a key, which LA-2
and LA-3 forbid).

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `backend/crates/apps/coffret-explorer-host` exists, builds without features under `make check`, and builds in release with `--features embed-web` and its example after the frontend is built
- [x] Tests cover: key injected from the file and a caller's key stripped, no key when the file is missing, `Host` rewritten, `Origin` rewritten only when it is the host's own, multi-megabyte bodies round-tripped, `502` on a refused connection
- [x] The crate uses `coffret-server`'s `SERVER_KEY_HEADER` and cites `spec: LA-3` in its documentation
- [x] `APPS` in the Makefile lists `coffret-explorer-host` and `make deps` passes with it; `make web-dist` exists
- [x] `make check` passes (shell lint, spec citations, backend, frontend)

### Before merge (verified outside the check command)
- [x] With `make dev LIBRARY=books` running for the development Library (a person starts it, since it asks for the Passphrase), `cargo run -p coffret-explorer-host --features embed-web --example serve -- --server 127.0.0.1:8787 --library books` prints a URL at which a Chromium browser shows the explorer's listing for that Library and can open a page — an agent runs this when the development pair is up, and reports the exact lines otherwise
