---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && test -f backend/crates/apps/coffret-napi/Cargo.toml && grep -rq "catch_unwind" backend/crates/apps/coffret-napi/src/ && ! grep -rqE "#\[napi\]$" backend/crates/apps/coffret-napi/src/ && grep -q "coffret-napi" Makefile'
assignee: null
branch: task/1021-0326-host-the-server-in-a-napi-addon-with-a-passphrase-door
created_at: 2026-10-20T18:26:00Z
updated_at: 2026-10-20T19:43:20Z
---

# feat(desktop): host the server in a Node-API addon, with a Passphrase door of its own

## Overview

The desktop app is moving from Tauri to Electron. The explorer will run inside the app's own window, and the server will run in-process in Electron's main process, through a Node-API addon written with napi-rs. This is the first of a sequence of PRs. It adds only the Rust side: a library crate that holds what an app shell needs to open a Library and serve the explorer, and a thin napi-rs crate over it. No Electron code lands here, and the Tauri app (`backend/crates/apps/coffret-desktop`) keeps working unchanged in behavior. It may depend on the new library crate for the modules this PR moves out of it.

A throwaway prototype, kept off main, proved the shape. Do not copy its shortcuts (listed under "Known prototype shortcuts").

### Decisions (already made; implement them)

1. **Two new crates under `backend/crates/apps/`.**
   - `coffret-desktop-core` (rlib) holds the logic and its tests.
   - `coffret-napi` (cdylib) is a thin binding over it: `[lib] crate-type = ["cdylib"]`, `test = false`, `doctest = false`, because test executables that reference `napi_*` symbols do not link.
   - Add both to the workspace, and add both to the `APPS` list that `make deps` checks.
   - Pin napi-rs deliberately: the current stable major, `napi`, `napi-derive`, `napi-build`. Record the version and why in the PR body.
2. **Panics never cross the FFI boundary.** Every exported function carries `#[napi(catch_unwind)]`, so a panic becomes a JS exception rather than an abort. Keep `panic = "unwind"` (the workspace default; do not set `panic = "abort"` in any profile).
3. **One owned tokio runtime per process.** It lives in the core crate (as `Desktop` owns one in the Tauri app, `coffret-desktop/src/main.rs:51-57`). The server's tasks and the hosts run on it. No synchronous export calls `tokio::spawn` off that runtime. `coffret_shell::logging::start()` is called once per process.
4. **What the addon exposes.** Keep it small and typed:
   - start, with the app's identifier and the idle minutes (validated as `settings.rs` does today);
   - the Library names on this device;
   - open the door for a mode (open or unlock), returning its URL;
   - whether a Library is open, and which;
   - issue a one-time launch ticket, and the explorer host's address;
   - register a callback for unlock requests (the explorer's `POST /api/unlock`, via `UnlockPrompt`), through a threadsafe function;
   - retire everything at quit (every session and ticket discarded, as `Launcher::retire` does today).
   - The addon never receives a Passphrase as an argument. No export takes one.
5. **The Passphrase door.** The Passphrase never passes through Electron's main process. The door is a small HTTP listener the core opens on `127.0.0.1:0`. It serves the Passphrase page itself, so the page and its `POST` are same-origin and no CORS is involved.
   - Main loads the door's URL into a separate window, with the window's token in the URL fragment. The page reads the token and drops it from the address with `history.replaceState`.
   - The page is a port of `coffret-desktop/ui/index.html` + `ui/unlock.js`, with Tauri's `invoke` replaced by `fetch` to the door. It is embedded in the crate.
   - Admission follows the explorer host's fences (LA-13–LA-16, LA-19):
     - an exact `Host` of the door's own authority;
     - `Origin` equal to it, and `Sec-Fetch-Site: same-origin` on the `POST`;
     - a bounded body;
     - no CORS headers.
   - **Token.** One token per opening of the window: 32 bytes from the OS CSPRNG, the same secret type and comparison the explorer host uses for its tickets (`coffret-explorer-host` credentials, LA-20). It is held as a digest and compared in constant time. It ends on success or after 5 refused attempts; the page then says the window must be opened again.
   - **Reading the Passphrase.** The body is read straight into the DK-7 inventory type, through the visitor in `coffret-desktop/src/unlock/entered_passphrase.rs`. Move that into core, so there is no intermediate `String`.
   - **Opening and unlocking.** These do what `open_library.rs` / `unlock_library.rs` do today, serialized by the same kind of `opening` mutex, minus the launch file:
     - `Launch::open` with an `UnlockPrompt` channel;
     - bind the explorer host on `127.0.0.1:0`, with `HostKind` from the identifier (`app_build.rs`; move it to core);
     - `ServerState::unlock` for the unlock mode.
   - **Refusals.** A refusal answers with `refusal_line` and logs with `refusal_evidence` (`coffret-desktop/src/unlock/refusal_line.rs`; move it to core). Never send `format!("{error:#}")`, which carries paths.
   - **Events.** Events never carry the door token, a launch ticket, or the URL (EL-1).
   - **On server stop.** When the server stops, the core retires everything and reports the stop to the shell through the addon, as an event the shell can act on. It does not call `process::exit`.
6. **The page source.** The explorer host serves the embedded page (`PageSource::Embedded`, feature `embed-web`, as the Tauri app does). A development override to `PageSource::Directory` is allowed only behind an explicit environment variable named in the PR.
7. **Spec.** Add the door's rules to `docs/spec/loopback-access/README.md` as new IDs after LA-21: what the door admits, the token, attempts, and what its events carry. Extend DK-7's sub-bullet in `docs/spec/device-key-custody/README.md`: for the desktop app, the claim starts where the door's handler reads the body into the inventory type; copies in the Passphrase window's renderer and its network stack are outside the claim. These follow the decisions above. LA-17 and LA-21 stay as they are in this PR, since the Tauri app still writes launch files; a later PR rewrites them.

### Known prototype shortcuts (do not carry over)

- No `catch_unwind` on exports.
- `format!("{error:#}")` in door answers.
- The token from `/dev/urandom` as a plain `String`, in a `HashSet`.
- `Access-Control-Allow-Origin: null` with no Origin check.
- The Passphrase through a serde `String`.
- Unlimited retries.
- No body limit.
- `HostKind::Development` hardcoded.
- `process::exit` on server stop.
- A separate Cargo workspace.

### Out of scope

- The Electron app itself, windows, tray, drop-as-paths, packaging, and removing Tauri. Later PRs do these.
- Changing the Tauri app's behavior.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `backend/crates/apps/coffret-napi` exists as a cdylib with `test = false`. Every `#[napi]` export carries `catch_unwind`: no bare `#[napi]` line in its `src/` (gated in `check_command`).
- [x] Both new crates are in the `APPS` list `make deps` checks (gated), and `make deps` passes: no app reaches usecase or local-fs directly.
- [x] Door admission tests in `coffret-desktop-core` refuse each of these:
  - a missing, wrong or duplicated `Host`;
  - a cross-origin or missing `Origin` on the `POST`;
  - a `Sec-Fetch-Site` other than `same-origin`;
  - a missing, unknown or used token;
  - an over-limit body.
- [x] Door token tests:
  - a token ends on success;
  - a token ends after 5 refused attempts and is then refused;
  - two windows' tokens are independent.
- [x] A test shows the Passphrase is read straight into the inventory type. The existing `EnteredPassphrase` ZeroizeOnDrop / not-Clone test moves with it and still passes.
- [x] Door refusals:
  - a wrong Passphrase answers with `refusal_line`'s sentence, and the answer contains no filesystem path;
  - an events test (in the style of `coffret-desktop/src/launch/tests.rs:475`) shows no door token, ticket or URL in any recorded event.
- [x] Concurrent open/unlock submissions are serialized: two at once give one Library opened and one coherent answer.
- [x] Unlock flow test: an unlock request from the server's `UnlockPrompt` reaches the registered callback; unlocking with the Library's Passphrase leaves the server unlocked.
- [x] Launch ticket test: a ticket issued through the core exchanges for a browser token at the explorer host, and is refused the second time.
- [x] The Tauri app still builds and its tests pass (`make check`).
- [x] The new spec rules have IDs and pass `make spec-rule-ids` and `make spec-citations`.

### Before merge (verified outside the check command)

- [ ] On macOS, the built `.node` is loaded from plain Node on the agent Library:
  - `start` succeeds;
  - opening the door returns a URL whose page renders in Playwright's Chromium;
  - submitting the agent Passphrase opens the Library;
  - a ticket from the addon admits Chromium to the explorer.
  Record the commands and output in the PR.
