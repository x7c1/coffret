# Install the desktop app

How to download, open and run the Coffret desktop app from a GitHub Release,
without building anything. Bundles are published for macOS (Apple silicon
only; Intel Macs are not supported) and for Ubuntu and other Debian-based
distributions (x86_64, as a `.deb`). The macOS app is signed only ad hoc —
with no Developer ID, and not notarized by Apple — so its first launch needs a
one-time workaround; the `.deb` is not signed.

- **[macOS](macos.md)** — the `.dmg`, and getting past Gatekeeper
- **[Ubuntu](ubuntu.md)** — the `.deb`

Coffret is in pre-alpha development: data formats may change without backward
compatibility between releases (see the README's status).

## What the app does

The app asks for a Library and its Passphrase in a small window of its own,
starts the server in its own process, and shows the explorer in a window of
its own. Once a Library is open the Passphrase window goes away; closing the
explorer's window hides it, and the app stays running with a tray icon, which
opens the explorer again, unlocks the Library once it has locked itself, or
quits. Only one copy runs at a time: launching it again brings the explorer
back. Quitting ends the server, which locks the Library. Links the explorer
opens in a new window, such as Google's consent page, open in your default
browser. [Environments](../environments.md#the-desktop-app) describes it in
full.

On Linux the tray icon needs something on the desktop that hosts one; GNOME
does only with an extension such as *AppIndicator and KStatusNotifierItem
Support*, which Ubuntu's desktop ships enabled as *Ubuntu AppIndicators*.
Without one there is no tray icon, and closing the explorer's window quits the
app.

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

It holds the Libraries on this device (`libraries/<library id>/`, each with
its settings, its Master Key under the Passphrase, and its catalog) and the
logs (`logs/`). A Library a build before Library names put there is in a
directory named after what this device called it. The first time this build
opens it — from the app or with a `coffret` command — it is given that name
and its directory is moved to its ID. An app started from the desktop or
the application menu is not given `COFFRET_STATE_DIR`, so it always uses this
directory; the
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

The app also keeps a directory of its own for the window's browser data (its
cache and the explorer's storage, nothing of a Library's): on macOS
`~/Library/Application Support/io.github.x7c1.coffret/`, and on Linux
`io.github.x7c1.coffret/` under `$XDG_CONFIG_HOME`, or under `~/.config`
where that is unset. Removing the app leaves it too; delete it along with the
app if you like.
