# Install the desktop app

How to download, open and run the Coffret desktop app from a GitHub Release,
without building anything. Bundles are published for macOS (Apple silicon
only; Intel Macs are not supported) and for Ubuntu and other Debian-based
distributions (x86_64, as a `.deb`). Neither is signed, so the first launch
on a Mac needs a one-time workaround.

- **[macOS](macos.md)** — the `.dmg`, and getting past Gatekeeper
- **[Ubuntu](ubuntu.md)** — the `.deb`

Coffret is in pre-alpha development: data formats may change without backward
compatibility between releases (see the README's status).

## What the app does

The app asks for a Library and its Passphrase in a small window of its own,
starts the server in its own process, and opens the explorer in your default
browser. Once a Library is open the window goes away and a tray icon is left,
which unlocks the Library, opens the explorer again, or quits. Only one copy
runs at a time: launching it again opens the explorer again.
[Environments](../environments.md#the-desktop-shell) describes it in full.

The app opens Libraries that already exist on the device; it does not create
or join one. That is done with the `coffret` command line, which the bundles
do not include: build it from a checkout of this repository (see
[Environments](../environments.md)), and point it at the same state directory
as the app (below).

## Where the app keeps its state

The app keeps everything under the binaries' default state directory, the same
on both platforms:

| Variable | Directory |
|---|---|
| `XDG_STATE_HOME` set | `$XDG_STATE_HOME/coffret/` |
| otherwise | `~/.local/state/coffret/` |

It holds the Libraries on this device (`libraries/<name>/`, each with its
settings, its Master Key under the Passphrase, and its catalog) and the logs
(`logs/`). An app started from the desktop or the application menu is not
given `COFFRET_STATE_DIR`, so it always uses this directory; the
[Environments](../environments.md#two-state-directories) guide explains how
this differs from a checkout's development state directory.

## Updating

Download the bundle from the new Release and install it over the old one the
same way as the first time. Quit the running app from its tray icon first, and
start it again afterwards. The state directory is left as it is.

## Removing the app

Quit the app from its tray icon, then delete `/Applications/Coffret.app` on
macOS, or run `sudo apt remove coffret-desktop` on Ubuntu. Neither touches the
state directory. Delete that only if you mean to remove the Libraries on this
device along with the app: a Library's Master Key file is in it.
