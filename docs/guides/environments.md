# Environments

Which checkout of this repository serves which Library, and what the `make`
targets do in each. For a person or an agent running coffret from a checkout.

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

with `?=`, so a value from the environment, or from a `local.mk` the
`Makefile` includes (`~/.config/coffret/local.mk` for the machine,
`local.mk` at the root of the checkout for that checkout), wins over it.
The scripts that keep state of their own — `e2e-it`, `drive-round-trip-it`,
`drive-index-layout-it` and the `drive-it-*` targets — set their own
directories under `.tmp/` and ignore these; `drive-authorize` logs under
`COFFRET_LOG_DIR` like the rest.

## Two kinds of checkout

**A development checkout** is any clone in which code is changed, and every
worktree of it. It has no `local.mk` naming a state directory, so everything it
runs works on Libraries under the development state directory. It may be on
any branch, including one whose Index schema or on-disk layout has moved, and
that is why it never reaches the default state directory unless told to.

**The production checkout** is one separate clone on the machine, kept on
`main`, whose `local.mk` names the default state directory. That is the
`local.mk` at the root of the checkout, never `~/.config/coffret/local.mk`:
every checkout on the machine includes the latter, so a state directory named
there would point every development checkout at the production Library too.
Its `local.mk` sets three things:

```make
COFFRET_STATE_DIR := <the default state directory, written out>
COFFRET_LOG_DIR := <a logs directory beside it>
PORT := <a server port no development checkout uses>
```

The state directory is written out in full — `$(HOME)/.local/state/coffret`,
or the `$XDG_STATE_HOME` equivalent — since `make` does not expand a shell
default. `PORT` is per checkout: the server listens on the loopback port the
checkout's `PORT` names (8787 unless set), and two servers on one machine need
two ports.

## What the targets do

| Target | Development checkout | Production checkout |
|---|---|---|
| `make dev` | The server for `LIBRARY` under the development state directory, and the explorer's dev server at `http://localhost:5173/`, which reads the checkout's source as it changes | Not used: the production checkout serves through `make prod` |
| `make prod` | Refused unless the checkout is on `main` with a clean tree; otherwise as in the production checkout, over the development state directory | Refuses on another branch, with uncommitted changes, or with commits `origin/main` does not hold; otherwise fast-forwards `main` to `origin/main`, installs the frontend dependencies from the lockfile, builds the explorer, and starts the server for `LIBRARY` and `vite preview` at `http://localhost:4173/` |
| `make cli ARGS="…"` | The command line, built in release, against the development state directory | The command line against the default state directory |
| `make down` | Stops the pair `make dev` or `make prod` started for `LIBRARY` | The same |
| `make desktop-build` | Builds the explorer, then bundles the desktop app under `backend/target/release/bundle/` (a `.deb` on Linux, `Coffret.app` and a `.dmg` on macOS) | The same |
| `make desktop` | `make desktop-build`, then installs the bundle: `sudo apt install --reinstall` of the newest `.deb`, or `/Applications/Coffret.app` replaced | The same |
| `make desktop-dev-build` | Builds the explorer, then the debug desktop shell as the development app | Not used |
| `make desktop-dev` | `make desktop-dev-build`, then runs it over the Libraries under the development state directory | Not used |

`make server` and `make web` are the two halves of `make dev` in the
foreground, and take the same variables.

One server serves a Library at a time, so `make dev` and `make prod` share the
files that record what is running for a Library: either one finds a pair of
the other kind already up, says so, and starts nothing. `make down` stops a
pair of either kind, and `make down prod` is the restart that a Library which
has locked itself after idling needs.

A Library under the development state directory is made the way any Library
is, through `make cli ARGS="init …"` or `make cli ARGS="join …"` in the
development checkout; `make cli ARGS="--help"` lists the commands.

## The desktop shell

The desktop shell is one installed app in place of a checkout: it asks for a
Library and its Passphrase in a small window of its own, starts the server in
its own process, and opens the explorer in the default browser. Once a
Library is open the window goes away and a tray icon is left, which unlocks
the Library, opens the explorer again, or quits; quitting ends the server,
which locks the Library. A second launch opens the explorer again rather than
starting a second copy, with the app's window in front of it where the Library
has locked. The Library also locks itself after the idle interval
(`COFFRET_IDLE_MINUTES`, 30 minutes unless set), as the server's does. It is
unlocked in place: press *unlock* in the explorer's status bar, or choose
*Unlock…* from the tray, and the app's own window comes forward to take the
Passphrase; the explorer's listing and reader come back once it is entered. The
Passphrase is typed only in that window, never in the explorer's page. The
server `make dev` and `make prod` start has no such window, so its *unlock*
says to start it again.

There are two of it, told apart by their identifier:

- **The installed app** (`make desktop`) is `io.github.x7c1.coffret`. It is
  started from the desktop, so nothing sets `COFFRET_STATE_DIR` for it, and it
  serves the Libraries under the default state directory: the production
  Library, whichever checkout it was built in.
- **The development app** (`make desktop-dev`) is built with the identifier
  `io.github.x7c1.coffret.dev`. It inherits the `Makefile`'s
  `COFFRET_STATE_DIR`, so it serves the Libraries under the development state
  directory, and its own identifier gives it its own single-instance scope, so
  it runs beside the installed app. It is one server like any other: a Library
  that `make dev` is already serving is refused in its window, and the other
  server has to be stopped (`make down`) first.

Both log where the binaries log, under `COFFRET_LOG_DIR` when that is set. The
Libraries either one lists are made with `make cli ARGS="init …"` or
`make cli ARGS="join …"` as above; the shell does not create or join one.

Building the shell needs the Tauri CLI once for `make desktop-build`
(`cargo install tauri-cli --version '^2' --locked`), and on Linux the
development files of WebKitGTK 4.1, GTK 3, ayatana-appindicator and librsvg.
On Linux both `make desktop-build` and `make desktop-dev-build` link the shell
with the system's C compiler, `/usr/bin/cc` (`build-essential`), whatever
`PATH` puts first: a compiler from a Nix profile, say, would make the binary
ask for a dynamic loader that cannot find the distribution's GTK. Both then
refuse a binary whose program interpreter is not the system's or whose
libraries do not resolve from an empty environment. The shell is a member of
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
- does not run `make prod` in the production checkout, and does not change
  that checkout's `local.mk`, unless asked to
