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

The scripts that keep state of their own — `e2e-it`, `e2e-electron`,
`drive-round-trip-it`, `drive-index-layout-it` and the `drive-it-*` targets —
set their own directories under `.tmp/` and ignore these; `drive-authorize`
logs under `COFFRET_LOG_DIR` like the rest.

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
`make cli ARGS="--help"` lists the commands. Every command after those names a
Library by `--library <name>` — the name `init` gave it, which a device that
joins reads from the Library — or by `--library-id <id>`, which tells apart two
Libraries on one device that carry the same name, and names a Library that has
no name yet (one joined before its catalog caught up). A device holds a Library
once: where two directories under `libraries/` carry one Library ID, a copy of
one beside it, say, either flag is refused with both directories until one of
them is moved out.

A Library just made can then be started from the explorer, with no `map` or
`sync` at the command line: the empty root offers to map a folder of this
device to it, "New folder…" on a folder's "⋮" (the tree's "Library" row among
them) prepares the path a drop is to land under — held by the page alone until
something is dropped there, and forgotten on a reload — and a drop starts the
sync that carries the files in. A top-level folder mapped with `map --prefix`
and nothing in it yet is listed at the root, marked as not in the Library yet,
to drop into the same way.

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
| `make desktop-build` | Builds the explorer, the Node-API addon and the Electron app, then bundles it for this host under `frontend/packages/apps/desktop/release/` (`Coffret_<version>_aarch64.dmg` on macOS, `coffret-desktop_<version>_amd64.deb` on Linux) and checks the bundle (see [building it](#building-it)) |
| `make desktop` | `make desktop-build`, then installs the bundle: `sudo apt install --reinstall` of the newest `.deb`, or `/Applications/Coffret.app` replaced |
| `make desktop-dev-build` | Builds the explorer, the Node-API addon and the Electron app, without bundling or starting it |
| `make desktop-dev` | `make desktop-dev-build`, then runs the app from the checkout over the Libraries under the development state directory, as the development app; on Ubuntu 24.04 and later it first needs an AppArmor profile (see [its sandbox](#the-development-apps-sandbox-on-linux)) |
| `make napi-addon` | Builds the explorer, then the Node-API addon the desktop app runs the server in, as `backend/target/release/coffret.node` |
| `make icons` | Derives the desktop app's icons, the tray's and the explorer's favicon from the SVG sources under `assets/icon/` (see [the desktop app](#the-desktop-app)) |
| `make drive-store-it` | Runs the `ObjectStore` and commit conformance suites, and the cases that pin what Drive answers, against a real Google Drive folder: each case in a subfolder of its own under `COFFRET_DRIVE_FOLDER_ID`, trashed when the case ends. It needs an account and a consent given at a browser (`make drive-authorize`), so CI never runs it; without the variables its cases report themselves skipped. The variables are described at `drive-authorize` and `drive-store-it` in the `Makefile` |

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

The desktop app is an Electron app, `frontend/packages/apps/desktop`. It runs
the server in its own process through the Node-API addon (`make napi-addon`),
and shows the explorer in a window of its own rather than in the system
browser. The Passphrase is typed into a second, small window,
served by the app's Passphrase door: the page there submits to the door
directly, and no page of the app reaches the app's own process. Once the
Library is open that window goes and the explorer's comes. When the explorer
asks for the Library to be unlocked — a press of *unlock*, or the idle lock
seen — the Passphrase window comes back to take the Passphrase, and goes again
once the Library is unlocked; the explorer comes back in place. A page that
has lost its session gets a fresh one from *Reload the explorer* in the app's
menu (Cmd/Ctrl+R). Addresses the explorer opens a window for, such as Google's
consent page, open in the system browser.

Closing the Passphrase window before a Library is open quits the app, since
nothing is served yet. Once one is open the app stays resident: closing the
explorer's window only hides it, and the server keeps serving the Library. The
explorer comes back from a second launch, a click on the Dock icon (macOS),
the tray's *Open the explorer*, or a click on the tray icon where the platform
delivers one (not on macOS, where a click opens the menu); a window that was
destroyed is opened anew from a fresh launch ticket, and where the Library has
locked, the Passphrase window comes in front of it in unlock mode.

The tray icon's menu has *Open the explorer*; *Unlock…*, which brings the
Passphrase window in unlock mode and is offered only while the Library is
locked; and *Quit Coffret*. The app ends by *Quit Coffret*, by the app menu's
*Quit* (Cmd+Q on macOS, Ctrl+Q in the *File* menu elsewhere), or by the
session ending. Quitting ends the server, which locks the Library, and every
browser session with it.

On Linux a tray icon is shown only where something on the session bus hosts
one — owns `org.kde.StatusNotifierWatcher`, which GNOME does only with an
extension such as AppIndicator and KStatusNotifierItem Support. The app asks
the bus as it starts (`gdbus`, given about a second), and logs which way it
went. Where nothing hosts one, or `gdbus` is missing or does not answer, there
is no tray, and closing the explorer's window quits the app. macOS and
Windows always have a tray.

A Passphrase window that says it can no longer take the Passphrase, after five
refused tries, is spent: the app is told so, and loads it afresh the next time
it is brought forward, so pressing *unlock* again, or the tray's *Unlock…*,
gives a window that takes the Passphrase. Before a Library is open, a second
launch, the Dock or the tray's *Open the explorer* brings it back fresh in the
same way, as does closing it, which quits the app, and starting the app again.

The Library also locks itself after the idle interval
(`COFFRET_IDLE_MINUTES`, 30 minutes unless set), as the server's does, and is
unlocked in place as above. The Passphrase is typed only in the Passphrase
window, never in the explorer's page. The server `make dev` starts has no such
window, so its *unlock* says to start it again.

There are two of it, told apart by their identifier, which the app takes from
how it is run:

- **The installed app** is `io.github.x7c1.coffret`: the app from a bundle,
  installed by `make desktop` or from a GitHub Release. It is started from the
  desktop, so nothing sets `COFFRET_STATE_DIR` for it, and it serves the
  Libraries under the default state directory: the production Library,
  whichever checkout it was built in.
- **The development app** (`make desktop-dev`) is the same app run from the
  checkout, which makes it `io.github.x7c1.coffret.dev`. It inherits the
  `Makefile`'s `COFFRET_STATE_DIR`, so it serves the Libraries under the
  development state directory, and refuses to start where that is not set.
  Its own identifier gives it its own single-instance scope and its own
  browser data, so it runs beside the installed app.

The development app is told apart from the installed one: its windows are
titled "Coffret (dev)", its tray icon carries the "D" badge and its tooltip
reads "Coffret (dev)", and its window icon (Linux) is the badged
`assets/icon-dev.png`, and its Dock icon (macOS) the same picture inset as the
installed app's `.icns` is, `assets/dock-dev.png`. On Linux each app's windows carry its own
identifier as their app ID (`WM_CLASS` on X11), so GNOME groups and names the
two apart, and matches the installed app's windows with the desktop entry its
`.deb` installs, whose `StartupWMClass` is `io.github.x7c1.coffret`.

Every icon is derived from two SVG sources under `assets/icon/`:
`coffret.svg`, the app icon, and `coffret-tray.svg`, the same chest as a
one-colour silhouette. `make icons` (`scripts/gen-icons.sh`) writes the
`.icns` and the PNG the bundles carry into
`frontend/packages/apps/desktop/packaging/`, the tray icons and the
development app's badged icon into `frontend/packages/apps/desktop/assets/`,
and the explorer's favicon under
`frontend/packages/apps/web/public/`; the outputs are committed, and running it
again rewrites the same bytes. The development app's icons carry a "D" badge in
the bottom-right corner, and its tray icon is badged too, so the two apps are
told apart in the tray without opening a menu. The tray icon is a
one-colour picture rather than the app icon, because a tray draws it small and
in the menu bar's own colours: on macOS it is black and set as a template
image, which the menu bar draws black or white for its appearance, and on
Linux it is white, since GNOME's top bar is dark in both its light and its dark
style. On macOS 26 the installed app's tile is drawn from `icon.icon`, the
source's background colour as its fill and the chest as one flat layer, in the
shape macOS gives every tile; macOS draws an `.icns`-only app's tile with a
glass finish of its own, which blurs its edges in the Dock. The `.icns`, the
icon before macOS 26, and the development app's `dock-dev.png` are the app icon
inset to 80.5 % on a transparent canvas, the share macOS gives its own icons,
because the Dock draws icons with margins and a full-bleed one would look
larger than its neighbours. Their background square has continuous corners at
the radius macOS 26 draws its tiles with, where the source's circular arcs
would look pinched beside them. The favicon is the chest alone, without the app
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

The script rasterises the SVGs with the `resvg` CLI, which writes a PNG of any
size without a graphics library beside it; it is a tool of the script alone,
not of the app. Install it once with
`cargo install resvg --version 0.47.0 --locked`; the script also needs
python3, which assembles the `.icns` from those PNGs, so no macOS tool is
needed and the script runs on Linux as well.

Both apps log where the binaries log, under `COFFRET_LOG_DIR` when that is set.

### Building it

`make desktop-build` (`scripts/desktop.sh build`) builds the explorer, the
Node-API addon with the explorer compiled in (`make napi-addon`), the app's
code, and then the bundle for the host, with electron-builder and the
package's `electron-builder.config.js`, into
`frontend/packages/apps/desktop/release/`: `Coffret_<version>_aarch64.dmg`
(and `mac-arm64/Coffret.app`) on macOS, `coffret-desktop_<version>_amd64.deb`
on Linux. The version is the workspace version of `backend/Cargo.toml`. The
build then checks the bundle — its fuses, its signature on macOS, its package
fields on Linux — as the Bundle workflow does ([the release
guide](release.md#desktop-bundles) lists what is checked).

It needs the frontend's dependencies (`pnpm install` in `frontend/`) and the
Rust toolchain. The Electron package fetches its binary the first time the
app is run, and electron-builder fetches the Electron release it packages
(and, on Linux, the tool that writes the `.deb`) the first time it bundles,
so the first `make desktop-dev` and the first `make desktop-build` need the
network.

On macOS the bundle needs Xcode 26 or later, not the Command Line Tools alone:
electron-builder compiles the app's Icon Composer icon (`packaging/icon.icon`)
with Xcode's `actool`. Install Xcode, then point the developer directory at it
with `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`;
`xcrun actool --version` reports 26 or later once it is in place.

On Linux the addon is built with the system's C compiler, `/usr/bin/cc`
(`build-essential`), whatever `PATH` puts first: Electron is loaded by the
distribution's dynamic loader, and an addon linked by a compiler from a Nix
profile, say, would ask for libraries that loader cannot find. Each build then
refuses an addon whose libraries do not resolve from an empty environment
through the program interpreter Electron's binary asks for.

### The development app's sandbox on Linux

The development app runs with Chromium's sandbox, as the installed app does,
and the sandbox needs to create user namespaces. Ubuntu 24.04 and later set
`kernel.apparmor_restrict_unprivileged_userns` to `1`, which lets an
unprivileged program create them only where an AppArmor profile allows it. The
installed app's profile names `/opt/Coffret/coffret-desktop` alone ([the
install guide](install/ubuntu.md#the-sandbox)); the development app runs the
checkout's Electron,
`frontend/node_modules/.pnpm/electron@<version>/node_modules/electron/dist/electron`,
which it does not name. So `make desktop-dev` refuses to start the app where
all of these hold:

- `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` reads `1`;
- the checkout's `chrome-sandbox` is not setuid root;
- no profile under `/etc/apparmor.d/` attaches to the checkout's Electron.

Where that file is missing or reads `0` (Ubuntu 22.04, other distributions,
macOS), the check passes and nothing is needed. When it refuses, it prints the
profile to install, with this checkout's absolute path in it, and the commands
that install and load it. The path has its symlinks resolved, as AppArmor
matches the path the binary really runs from, so copy it from the message
rather than from `$PWD`:

```bash
sudo tee /etc/apparmor.d/coffret-desktop-dev >/dev/null <<'PROFILE'
abi <abi/4.0>,
include <tunables/global>

profile coffret-desktop-dev <checkout>/frontend/node_modules/.pnpm/electron@*/node_modules/electron/dist/electron flags=(unconfined) {
  userns,
}
PROFILE
sudo apparmor_parser -r /etc/apparmor.d/coffret-desktop-dev
```

This is needed once per checkout, and `make` never runs it: it needs `sudo`,
so the developer runs it. Like the installed app's, the profile confines
nothing and allows the binary to create user namespaces alone. It names any
version of Electron (`electron@*`), so it outlives an Electron upgrade. A
second checkout or worktree has a path of its own, so it needs a profile of
its own: another `profile` block naming its path in full (the check does not
expand AppArmor variables such as `@{HOME}`, so it would not count a path
written with one), or a file of its own, which `make desktop-dev` names after
the checkout when `coffret-desktop-dev` is already taken.

Chromium's own message, were the app started without the check, asks for
`chrome-sandbox` to be made setuid root; `pnpm install` undoes that, and the
profile is the way this guide takes. The check reads the profiles' files, not
what the kernel has loaded, so a profile written but not loaded (its
`apparmor_parser -r` line not run, or failed) lets the check pass and Chromium
then aborts with that message; run the `apparmor_parser -r` line, and see
what it says. `make desktop-dev-sandbox-test` runs the check over throwaway
directories (`scripts/desktop-dev-sandbox-test.sh`).

### Trying a bundle without installing it

A bundle is the installed app, so started as it is it serves the production
Library, and hands its launch to an installed copy that is already running. To
try one beside the installed app, run its binary from the mounted `.dmg` (or
the extracted `.deb`) with a state directory and a browser-data directory of
its own:

```bash
COFFRET_STATE_DIR="$PWD/.tmp/try" COFFRET_LOG_DIR="$PWD/.tmp/try/logs" \
  /Volumes/Coffret*/Coffret.app/Contents/MacOS/Coffret --user-data-dir="$(mktemp -d)"
```

The app takes `--user-data-dir` in place of its own browser-data directory, so
its single-instance lock is apart from the installed app's too.

On Linux the binary of an extracted `.deb` is
`<dir>/opt/Coffret/coffret-desktop`. On Ubuntu 24.04 and later it cannot
start Chromium's sandbox from there, for the reason the development app
cannot ([its sandbox](#the-development-apps-sandbox-on-linux)): the AppArmor
profile that lets the app create user namespaces names
`/opt/Coffret/coffret-desktop` alone, and the extracted `chrome-sandbox` is
not setuid root ([the install guide](install/ubuntu.md#the-sandbox)). A
profile of the same kind naming `<dir>/opt/Coffret/coffret-desktop`, in a
file and under a name of its own (not `coffret-desktop`, the installed app's),
lets it; otherwise try it on Ubuntu 22.04, or install it.

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
