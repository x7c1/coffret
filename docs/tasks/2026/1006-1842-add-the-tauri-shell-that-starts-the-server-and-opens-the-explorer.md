---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && test -f backend/crates/apps/coffret-desktop/tauri.conf.json && grep -q "\"identifier\": \"io.github.x7c1.coffret\"" backend/crates/apps/coffret-desktop/tauri.conf.json && grep -qE "^desktop:" Makefile && grep -qE "^desktop-dev:" Makefile && grep -qE "^desktop-build:" Makefile && grep -qE "^desktop-dev-build:" Makefile && grep -q "libwebkit2gtk-4.1-dev" .github/workflows/ci.yml && grep -qE "^APPS := .*coffret-desktop" Makefile && test -d backend/crates/apps/coffret-server/src/launch -o -f backend/crates/apps/coffret-server/src/launch.rs && test -f scripts/desktop.sh'
assignee: null
branch: task/1006-1842-add-the-tauri-shell-that-starts-the-server-and-opens-the-explorer
created_at: 2026-10-06T18:42:11Z
updated_at: 2026-10-06T19:16:25Z
---

# feat(desktop): add the Tauri shell that starts the server and opens the explorer in the system browser

## Overview

Running coffret for everyday use still means a checkout: `make prod` builds the
server and the explorer, asks for the Passphrase on the terminal, and serves the
page through `vite preview`. The desktop shell replaces that with one installed
app. It is a **launcher, not a window onto the explorer**: it starts the server
in its own process, serves the built explorer through `coffret-explorer-host`
(the Rust twin of the vite proxy, merged already), and opens that URL in the
system's default browser. The explorer's reader and its drag-and-drop need a
Chromium-class engine, which the Linux webview (WebKitGTK) is not — so the shell
has no webview window for the explorer at all. What it does have is one small
window of its own, for the Passphrase, so that no secret ever passes through the
explorer's page (spec: DK-10 says how a device is given a secret; LA-3 and LA-6
say the server key never reaches a page either).

**1. `coffret-server` exposes what its `main.rs` does, so the shell can do it in-process.**
`backend/crates/apps/coffret-server/src/main.rs` opens the Library behind the
Passphrase, builds `ServerState`, catches the catalog up, publishes the server
key, binds `127.0.0.1:<port>` and serves the router with the idle lock armed.
Move that sequence into the library (a `launch` module: a `Launch` describing the
Library name, port — `0` for any free port — and idle interval, taking the
Passphrase through the same `FnOnce() -> Result<Passphrase>` shape
`open_library` already takes; and a `Serving` handing back the bound address and
the `Arc<ServerState>`, with the serve future the caller drives). `main.rs`
becomes a thin CLI over it with exactly today's behaviour and messages — the
terminal prompt stays `coffret_shell::passphrase::entering`, the printed lines
stay the same, `--passphrase-stdin` keeps working. Routes tests must not change.

**2. A new binary crate `coffret-desktop` under `backend/crates/apps/`, on Tauri v2.**
Lay it out the way `delta-desktop` is laid out in the delta repository
(`Cargo.toml` with `tauri-build`, `build.rs`, `tauri.conf.json`,
`tauri.linux.conf.json`, `icons/`, `linux/<name>.desktop`), but it opens no
webview for the explorer:
- `tauri.conf.json`: `productName` `Coffret`, `identifier`
  `io.github.x7c1.coffret`, `app.windows: []`, `enableGTKAppId: true`, bundle
  targets `dmg` and `deb`, the Tauri default icons for now.
  `tauri.linux.conf.json` names the package `coffret-desktop` (a bare `coffret`
  would collide with other packages) and a `linux/coffret-desktop.desktop`
  template whose `StartupWMClass` is the identifier, as delta does so GNOME shows
  one app per identifier. On Linux set GLib's program name to the identifier
  before the event loop (`glib::set_prgname`).
- Plugins: `tauri-plugin-single-instance` (a second launch opens the explorer URL
  in the browser again and exits), `tauri-plugin-dialog` (a message dialog for a
  startup failure a person has to act on), and `tauri-plugin-opener` for opening
  the URL in the default browser.
- **The Passphrase window.** The shell's own assets under the crate (`ui/`,
  `frontendDist: "ui"`, `withGlobalTauri: true`): one page with a `<select>` of
  the Libraries on this device and a password `<input>`, a button, and a line
  for a refusal. It calls `invoke("open_library", { name, passphrase })` and
  nothing else. Keep it to plain HTML and a few lines of script — no framework,
  no build step. The window has label `unlock`, is small, is not resizable, and
  closing it while nothing is served quits the app. The list of Libraries comes
  from `coffret-device`: add a function on `LibraryDir` that lists the Libraries
  under the state root (directories under `libraries/` that are whole, i.e. not
  `.partial`), with a unit test; the shell preselects the only one when there is
  one.
- **`open_library` command.** On the Tauri side: wrap the typed secret in the
  `Passphrase` type (zeroized like everywhere else), call the server's `launch`
  with a closure that hands it over, and on success bind `coffret-explorer-host`'s
  router on `127.0.0.1:0` (its `Config::for_library` with the same state root),
  open the host's URL in the default browser, hide the window, and show the tray.
  A wrong Passphrase or a Library that does not open is answered to the page as
  the sentence the server would have printed, and the window stays. The server
  and host run on one tokio runtime the shell owns for its whole life.
- **Tray.** An icon with a menu: *Open the explorer* (opens the host URL again)
  and *Quit* (ends the process; the server ending is the lock, spec: DK-1). No
  *Unlock* item yet — in-place unlock is the next task.
- **State directory.** The shell sets nothing: the binaries' default
  (`$XDG_STATE_HOME/coffret`) is where the installed app's Libraries are. Under
  `make desktop-dev` the Makefile's exported `COFFRET_STATE_DIR` (`coffret-dev`)
  reaches the process, so the development shell serves development Libraries
  without any code knowing which it is. Logs go where the binaries' logs go
  (`coffret_shell::logging::start`, `COFFRET_LOG_DIR`).
- **Development identifier.** `make desktop-dev` builds with
  `TAURI_CONFIG='{"identifier":"io.github.x7c1.coffret.dev"}'` in the
  environment, exactly as delta's `scripts/dev.sh` does, so the development
  shell is a different app to the desktop (its own single-instance scope and
  app id) and the installed one keeps `tauri.conf.json`'s identifier.

**3. Makefile targets, mirroring delta's.** `desktop-build` (`web-dist`, then
`cargo tauri build` from the crate directory with `TAURI_CONFIG` unset; the
desktop crate depends on `coffret-explorer-host` with `features = ["embed-web"]`
so the bundle carries the page), `desktop` (`desktop-build`, then install: Linux
`sudo apt install --reinstall` the newest `.deb` under
`backend/target/release/bundle/deb/`, macOS replace `/Applications/Coffret.app`),
`desktop-dev-build` (`web-dist`, then a debug `cargo build -p coffret-desktop`
with the development `TAURI_CONFIG`) and `desktop-dev` (`desktop-dev-build`, then
run the debug binary; it inherits the Makefile's `COFFRET_STATE_DIR`). Put the
shell-side logic in a `scripts/desktop.sh` with `shellcheck`/`shfmt` clean
output, and the `## target:` help lines the Makefile reads. Add `coffret-desktop`
to the Makefile's `APPS` list; it is a shell over `coffret-device`,
`coffret-server` and `coffret-explorer-host` and must not reach a gateway or use
case directly.

**4. CI builds the shell.** The backend job in `.github/workflows/ci.yml` runs
`cargo build --locked` over the workspace, which now includes a crate that links
WebKitGTK, GTK 3 and the app-indicator library. Install them the way delta's
`ci.yml` does — an `APT_PACKAGES` list (`libwebkit2gtk-4.1-dev libgtk-3-dev
libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev patchelf`) with
the `.deb` download cache keyed on the list, then `apt-get install` — before the
Rust steps. The `cargo-deny` job needs nothing new unless a licence it refuses
appears; check its output.

**5. The environments guide.** `docs/guides/environments.md` gets the four
targets in its table and a short section saying what the shell is (installed
app = default state directory, `make desktop-dev` = development state directory,
identifier `.dev`). Leave `make prod` in place; its removal is a later task.

Out of scope: in-place unlock after the idle lock (the next task adds
`ServerState::unlock`, a `POST /api/unlock` request and the tray's *Unlock*
item), reconnecting a Drive grant, the Release workflow and unsigned-app
documentation, signing, a real icon, `init` / `join` in the shell.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `coffret-server` has a `launch` module the CLI's `main.rs` calls, and the CLI's routes tests and `--passphrase-stdin` handling are unchanged
- [x] `backend/crates/apps/coffret-desktop` exists with `tauri.conf.json` (`io.github.x7c1.coffret`, `app.windows` empty), builds and passes clippy under `make check`, and is on the Makefile's `APPS` list with `make deps` passing
- [x] `LibraryDir` lists the whole Libraries under a state root, with a unit test that ignores a `.partial` directory
- [x] `make desktop-build`, `make desktop`, `make desktop-dev-build` and `make desktop-dev` exist and appear in `make help`; `scripts/desktop.sh` passes `make shell-lint`
- [x] `.github/workflows/ci.yml` installs the WebKitGTK, GTK and app-indicator development packages before building the backend
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] `make desktop-dev-build` produces `backend/target/debug/coffret-desktop`, and starting it with a scratch `COFFRET_STATE_DIR` holding no Library shows the Passphrase window with an empty Library list and no crash within ten seconds; an agent runs this and reports the lines printed
- [ ] Needs a person: `make desktop-dev` shows the window, choosing `books` and entering the Passphrase opens the explorer in the default browser, *Open the explorer* in the tray opens it again, and *Quit* ends the server (nothing answers at its port afterwards)
- [ ] Needs a person: `make desktop` installs the app, and launching it from the desktop serves the production Library in the browser while `make desktop-dev` serves the development one at the same time
