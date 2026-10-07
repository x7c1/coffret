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

## Production CLI operations

The desktop app opens a Library and serves it; it does not offer the
commands that create or change one: `init`, `join`, `authorize`, and `map`
with a prefix. A person runs those on the production Library through
`make cli`, naming the default state directory on the command line:

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
| `make dev` | The server for `LIBRARY` under the development state directory, and the explorer's dev server at `http://localhost:5173/`, which reads the checkout's source as it changes |
| `make cli ARGS="…"` | The command line, built in release, against the development state directory unless `COFFRET_STATE_DIR` is given |
| `make down` | Stops the pair `make dev` started for `LIBRARY`; `make down dev` is the restart that a Library which has locked itself after idling needs |
| `make desktop-build` | Builds the explorer, then bundles the desktop app under `backend/target/release/bundle/` (a `.deb` on Linux, `Coffret.app` and a `.dmg` on macOS) |
| `make desktop` | `make desktop-build`, then installs the bundle: `sudo apt install --reinstall` of the newest `.deb`, or `/Applications/Coffret.app` replaced |
| `make desktop-dev-build` | Builds the explorer, then the debug desktop app as the development app |
| `make desktop-dev` | `make desktop-dev-build`, then runs it over the Libraries under the development state directory |

`make server` and `make web` are the two halves of `make dev` in the
foreground, and take the same variables.

## The desktop app

The desktop app asks for a Library and its Passphrase in a small window of its
own, starts the server in its own process, and opens the explorer in the default
browser. Once a Library is open the window goes away and a tray icon is left,
which unlocks the Library, opens the explorer again, or quits; quitting ends the
server, which locks the Library. A second launch opens the explorer again rather
than starting a second copy, with the app's window in front of it where the
Library has locked. The Library also locks itself after the idle interval
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
