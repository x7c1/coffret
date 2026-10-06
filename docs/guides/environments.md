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

## The rule for agents

The default state directory holds the production Library. An agent:

- never reads the default state directory, and never points a binary at it
- runs coffret through `make`, which sets `COFFRET_STATE_DIR` to the
  development state directory — `make dev`, `make cli ARGS="…"`, `make down`
- sets `COFFRET_STATE_DIR` explicitly, to a directory of its own, whenever it
  runs a binary by hand (`coffret`, `coffret-server`, or anything started with
  `cargo run`), and `COFFRET_LOG_DIR` beside it
- does not run `make prod` in the production checkout, and does not change
  that checkout's `local.mk`, unless asked to
