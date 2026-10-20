# Environments

What serves which Library on a device, and what the `make` targets do. For a
person or an agent running coffret from a checkout.

## Two state directories

A device keeps its Libraries, the grants it holds, and its logs under a state
directory. The binaries' own default is `coffret` under `$XDG_STATE_HOME`, or
under `$HOME/.local/state` where that is unset: the **default state
directory**. That is where the **production Library** lives: the one a person
uses every day. The binaries keep defaulting to it.

Every `make` target in a checkout points somewhere else unless told otherwise.
The `Makefile` exports

- `COFFRET_STATE_DIR` — `coffret-dev` under `$XDG_STATE_HOME`, or under
  `$HOME/.local/state`: the **development state directory**
- `COFFRET_LOG_DIR` — `logs` under `COFFRET_STATE_DIR`

with `?=`, so a value from the environment or the command line wins over it.
A checkout may be on any branch, including one whose Index schema or on-disk
layout has moved, and that is why it never reaches the default state
directory unless told to.

The `Makefile` also includes a `local.mk` if there is one:
`~/.config/coffret/local.mk` for the machine, then `local.mk` at the root of
the checkout for that checkout. Either is for toolchain pins and parameter
defaults (`LIBRARY := books`, say); neither names a state directory, since
every checkout on the machine includes the former, and a state directory
named there would point every one of them at the production Library. A
checkout that once served the production Library itself may still have a
`local.mk` setting `COFFRET_STATE_DIR` and `COFFRET_LOG_DIR`; remove those
lines, or every target there, `make dev` and `make cli` included, works on the
production Library.

The scripts that keep state of their own — `e2e-it`, `drive-round-trip-it`,
`drive-index-layout-it` and the `drive-it-*` targets — set their own
directories under `.tmp/` and ignore these; `drive-authorize` logs under
`COFFRET_LOG_DIR` like the rest.

## The production Library

The installed desktop app serves the production Library (see [the desktop
app](#the-desktop-app) below).

Install it, and update it, in either of two ways:

- `make desktop` from a checkout on `main`, which builds the bundle and
  installs it over the one before. Quit the running app from its tray icon
  first.
- A bundle from a GitHub Release, as the [install
  guide](install/README.md) describes.

## The development Library

A Library under the development state directory is served by either of

- `make dev`, which `make down` stops
- the development app from `make desktop-dev`, which runs beside the
  installed app

never both at the same time for one Library: one server serves a Library at
a time, and the second is refused.

A Library under the development state directory is made the way any Library
is, through `make cli ARGS="init …"` or `make cli ARGS="join …"`;
`make cli ARGS="--help"` lists the commands.

`make rename-smoke LIBRARY=<name>` checks that the build in the checkout
renames correctly against such a Library on its real Storage, a development
Library on Drive, say, which CI cannot reach. It needs the Library's
Passphrase, typed once and never echoed, and `jq` on the `PATH`; no client
secret, since the Library keeps its client. Under the Library-root mapping, or
the first mapping where the root is not mapped, it creates a
`rename-smoke-<UTC timestamp>/` folder holding two books of three small SVG
pages each, and so uploads a few small files. It syncs them, renames one book's
folder with `coffret rename`, renames two pages of the other on disk and syncs
again, and checks that both renames uploaded no Container. Each sync covers the
whole Library, so whatever else is waiting in its mapped
folders is carried in too. It writes nothing outside the smoke folder, and
leaves that folder in the Library: delete it from the explorer. Like every
target, it works under the development state directory unless
`COFFRET_STATE_DIR` names another; against the production Library it is

```bash
make rename-smoke COFFRET_STATE_DIR="$HOME/.local/state/coffret" LIBRARY=<name>
```

after the two things [Production CLI operations](#production-cli-operations)
asks first, and the smoke folder is then left in the production Library.

## Production CLI operations

The desktop app opens a Library and serves it; it does not offer the
commands that create or change one: `init`, `join`, `authorize`, `map`
with a prefix, and `change-passphrase`. A person runs those on the
production Library through `make cli`, naming the default state directory
on the command line:

```bash
make cli COFFRET_STATE_DIR="$HOME/.local/state/coffret" ARGS="…"
```

(or `$XDG_STATE_HOME/coffret` where that is set). A variable given on the
`make` command line overrides the `Makefile`'s `?=`, and `COFFRET_LOG_DIR`
follows it to `logs` beside the Libraries. Two things first:

- **Quit the desktop app** from its tray icon, so that nothing is serving the
  Library while it is changed.
- **Use a clean checkout of `main`.** Another branch may have moved the Index
  schema or the on-disk layout, and a binary built from it would write a
  production Library that the installed app, built from `main`, cannot read.

## What the targets do

| Target | What it does |
|---|---|
| `make dev` | The server for `LIBRARY` under the development state directory, a build of the explorer in watch mode, and the development host serving that build at `http://127.0.0.1:5173/` in front of the server; it prints the launch file to open the explorer by |
| `make cli ARGS="…"` | The command line, built in release, against the development state directory unless `COFFRET_STATE_DIR` is given |
| `make rename-smoke LIBRARY=<name>` | Builds the command line, asks for the Passphrase once, and checks a `coffret rename` and two renames on disk against that Library, under a fresh smoke folder it leaves behind |
| `make down` | Stops the three `make dev` started for `LIBRARY`; `make down dev` is the restart that a Library which has locked itself after idling needs |
| `make desktop-build` | Builds the explorer, then bundles the desktop app under `backend/target/release/bundle/` (a `.deb` on Linux, `Coffret.app` and a `.dmg` on macOS) |
| `make desktop` | `make desktop-build`, then installs the bundle: `sudo apt install --reinstall` of the newest `.deb`, or `/Applications/Coffret.app` replaced |
| `make desktop-dev-build` | Builds the explorer, then the debug desktop app as the development app |
| `make desktop-dev` | `make desktop-dev-build`, then runs it over the Libraries under the development state directory |
| `make napi-addon` | Builds the explorer, then the Node-API addon the Electron app runs the server in, as `backend/target/release/coffret.node` |
| `make desktop-electron-dev` | `make napi-addon`, then builds the Electron app (`frontend/packages/apps/desktop`) and runs it over the Libraries under the development state directory, as the development app (see [the Electron app](#the-electron-app)) |
| `make icons` | Derives the desktop app's icons, the tray's and the explorer's favicon from the SVG sources under `assets/icon/` (see [the desktop app](#the-desktop-app)) |

`make server` and `make web` are the two halves of `make dev` in the
foreground, and take the same variables; `WEB_PORT` moves the explorer's port.

## The explorer in development

The explorer reaches a server through the explorer host in every mode, and
development is no exception: there is no dev server of the frontend's own, no
proxy in its build configuration, and no hot reload. `make web` (and `make
dev`) run `vite build --watch`, which rewrites
`frontend/packages/apps/web/dist` as the source changes, and
`coffret-explorer-dev`, which serves that directory from disk through the same
host the desktop app runs — the same fences, and the same browser tokens. A
change is picked up by reloading the tab once the build has written it.

The host admits a browser only by a browser token, which the page gets by
exchanging the one-time ticket in a **launch file**: an owner-only HTML page
under the Library's directory (`explorer-launch/explorer.html`) that refreshes
to the explorer with the ticket in the address's fragment. The host prints its
path once it has seen the server. Open that file in the browser — `make dev`
prints the path once all three are up, and `.tmp/dev/<LIBRARY>/host.log` holds it
too. The address typed by hand shows a page saying the tab holds no session.

The host keeps the file ready: it writes it again once its ticket has been used
or is about to expire, and writes a fresh one — ending every tab's session —
when the server is started again, since a server of another run is not the one
those tabs' sessions were issued under. While the server is stopped there is no
file at all.

What the development host cannot see is whether the server's Library is locked,
since the server is another process: it forwards as though it were unlocked,
and the server's own locked refusal is what the explorer shows.

## The desktop app

The desktop app asks for a Library and its Passphrase in a small window of its
own, starts the server in its own process, and opens the explorer in the default
browser. Once a Library is open the window goes away and a tray icon is left,
which unlocks the Library, opens the explorer again, disconnects every browser
tab, or quits; quitting ends the server, which locks the Library. A second
launch opens the explorer again rather than starting a second copy, with the
app's window in front of it where the Library has locked.

The app opens the explorer the way the development host's launch file does: it
issues a one-time ticket, writes it into an owner-only **launch file** — a page
that refreshes to `http://127.0.0.1:<port>/#launch=<ticket>` — and asks the
system to open that file, so the browser's command line carries a path and
never the ticket. Every opening (after the Passphrase, from the tray's *Open
the explorer*, from a second launch) is a fresh ticket in a fresh file, which
the app removes once the page has exchanged it or after the minute it is good
for. The files live in a `0700` directory named for the app's identifier:
`$XDG_RUNTIME_DIR/<identifier>` on Linux, and `explorer-launch/<identifier>`
under the state directory where `$XDG_RUNTIME_DIR` is unset and on macOS. The
app refuses to start where that directory is another account's or open to
anyone else, and removes what its last run left there as it starts.

The address typed by hand, a bookmark, and a tab whose session ended — after
12 hours, or because the app ended it — show a page saying the tab holds no
session, and to reload the explorer from the app; with this app, that is
opening it again from the tray. *Disconnect all browser tabs*
ends every tab's session at once; quitting does too, and so does the server
stopping. At most eight tabs hold a session at once, and closing a tab does not
end its session; opening a ninth ends the session of the tab used least
recently, which then shows the page saying it holds no session. Locking does
not end a session: a tab of a locked Library keeps its session, shows the
locked state, and comes back once the Library is unlocked. The browser is
expected to be a Chromium-based one, installed from a `.deb` on Linux or as the
macOS app; a browser installed as a Snap may not read the launch file, and is
not supported.

The Library also locks itself after the idle interval
(`COFFRET_IDLE_MINUTES`, 30 minutes unless set), as the server's does. It is
unlocked in place: press *unlock* in the explorer's status bar, or choose
*Unlock…* from the tray, and the app's own window comes forward to take the
Passphrase; the explorer's listing and reader come back once it is entered. The
Passphrase is typed only in that window, never in the explorer's page. The
server `make dev` starts has no such window, so its *unlock* says to start it
again.

There are two of it, told apart by their identifier:

- **The installed app** (`make desktop`) is `io.github.x7c1.coffret`. It is
  started from the desktop, so nothing sets `COFFRET_STATE_DIR` for it, and it
  serves the Libraries under the default state directory: the production
  Library, whichever checkout it was built in.
- **The development app** (`make desktop-dev`) is built with the identifier
  `io.github.x7c1.coffret.dev`. It inherits the `Makefile`'s
  `COFFRET_STATE_DIR`, so it serves the Libraries under the development state
  directory, and its own identifier gives it its own single-instance scope, so
  it runs beside the installed app.

Every icon is derived from two SVG sources under `assets/icon/`:
`coffret.svg`, the app icon, and `coffret-tray.svg`, the same chest as a
one-colour silhouette. `make icons` (`scripts/gen-icons.sh`, which needs the
Tauri CLI and python3) writes the installed app's `icons/`, the development
app's `icons-dev/`, the tray's `icons-tray/` and the explorer's favicon under
`frontend/packages/apps/web/public/`; the outputs are committed, and running it
again rewrites the same bytes. The development app's icons carry a "D" badge in
the bottom-right corner, and its tray icon is badged too, so the two apps are
told apart in the tray without opening a menu; on macOS its tooltip also reads
"Coffret (dev)" (Tauri shows no tray tooltip on Linux). The app reads which it
is from its identifier. The tray icon is a
one-colour picture rather than the app icon, because a tray draws it small and
in the menu bar's own colours: on macOS it is black and set as a template
image, which the menu bar draws black or white for its appearance, and on
Linux it is white, since GNOME's top bar is dark in both its light and its dark
style. The `.icns` is the app icon inset to 80 % on a transparent canvas,
because the Dock draws icons with margins and a full-bleed one would look
larger than its neighbours. The favicon is the chest alone, without the app
icon's background square, so that it reads at 16 px in a browser tab; it takes
the tray source's shape in the app icon's two colours, and lighter ones on a
dark tab (the SVG switches by itself, and `index.html` picks the light or the
dark PNG fallback by the colour scheme). `make icons` also writes a badged
favicon beside it (`favicon-dev.svg`, `favicon-32-dev.png`,
`favicon-32-dark-dev.png`), with the same "D" badge separated from the chest
by a transparent ring. A development host — `make dev`'s, or the development
app's — answers the favicon's paths with the badged files, so its tabs are
told apart from the installed app's; `index.html` links only the plain ones,
and the tab's title stays "coffret" whichever host served it.

Both log where the binaries log, under `COFFRET_LOG_DIR` when that is set.

Building the app needs the Tauri CLI once for `make desktop-build`
(`cargo install tauri-cli --version '^2' --locked`), and on Linux the
development files of WebKitGTK 4.1, GTK 3, ayatana-appindicator and librsvg.
On Linux both `make desktop-build` and `make desktop-dev-build` link the app
with the system's C compiler, `/usr/bin/cc` (`build-essential`), whatever
`PATH` puts first: a compiler from a Nix profile, say, would make the binary
ask for a dynamic loader that cannot find the distribution's GTK. Both then
refuse a binary whose program interpreter is not the system's or whose
libraries do not resolve from an empty environment. The app is a member of
the backend workspace, so `make check` builds it and needs those files too;
the backend job in `.github/workflows/ci.yml` lists the Debian and Ubuntu
package names.

### The Electron app

The desktop app is moving to Electron (`frontend/packages/apps/desktop`). For
now it runs from a checkout alone, through `make desktop-electron-dev`, as the
development app: it carries the identifier `io.github.x7c1.coffret.dev`,
serves the Libraries under the development state directory, and refuses to
start where `COFFRET_STATE_DIR` is not set. It shares that identifier and
those Libraries with `make desktop-dev`, so run one or the other.

It runs the server in its own process through the Node-API addon
(`make napi-addon`), and shows the explorer in a window of its own rather than
in the system browser. The Passphrase is typed into a second, small window,
served by the app's Passphrase door: the page there submits to the door
directly, and no page of the app reaches the app's own process. Once the
Library is open that window goes and the explorer's comes. When the explorer
asks for the Library to be unlocked — a press of *unlock*, or the idle lock
seen — the Passphrase window comes back to take the Passphrase, and goes again
once the Library is unlocked; the explorer comes back in place. A page that
has lost its session gets a fresh one from *Reload the explorer* in the app's
menu (Cmd/Ctrl+R). Addresses the explorer opens a window for, such as Google's
consent page, open in the system browser.

Closing the Passphrase window before a Library is open quits the app, and so
does closing the explorer's window; quitting ends the server, which locks the
Library. A second launch brings the open window forward. There is no tray yet.
A Passphrase window that says it can no longer take the Passphrase, after too
many refused tries, is not reloaded while it shows, so pressing *unlock* only
brings it forward: close it and press *unlock* again for a fresh one, or,
before a Library is open, start the app again.

The Electron package fetches its binary the first time it is run, so the first
`make desktop-electron-dev` needs the network.

## The rule for agents

The default state directory holds the production Library. An agent:

- never reads the default state directory, and never points a binary at it.
  The logs under it are the one exception: no event in them retains a
  credential, an Entry Path, or a path of the person's, so an agent may read
  them to tell a failure apart
- runs coffret through `make`, which sets `COFFRET_STATE_DIR` to the
  development state directory — `make dev`, `make cli ARGS="…"`, `make down`
- sets `COFFRET_STATE_DIR` explicitly, to a directory of its own, whenever it
  runs a binary by hand (`coffret`, `coffret-server`, or anything started with
  `cargo run`), and `COFFRET_LOG_DIR` beside it
- never runs `make cli` with `COFFRET_STATE_DIR` pointing at the default state
  directory
