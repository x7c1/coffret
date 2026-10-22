# Install on Ubuntu

The Linux bundle is a `.deb` for Ubuntu and other Debian-based distributions
(x86_64). What the app does and where it keeps its state are in the
[install guide](README.md). For other distributions, build it from a checkout
with `make desktop-build` (see [Environments](../environments.md)).

## Download and install

1. Open the [latest Release](https://github.com/x7c1/coffret/releases/latest)
   and download `coffret-desktop_<version>_amd64.deb`.
2. Install it from the directory you downloaded it to:

   ```bash
   sudo apt install ./coffret-desktop_<version>_amd64.deb
   ```

   The `./` matters: without it, `apt` looks for a package of that name in
   its repositories instead of installing the file.

`apt` pulls in the libraries Electron, which the app is built on, needs:
GTK 3, NSS, libnotify, libsecret, the accessibility bridge and a few X11
libraries, and `xdg-utils`, most of which a desktop install of Ubuntu already
has. The app itself goes under `/opt/Coffret/`; Coffret then appears in the
application menu, and the `coffret-desktop` command starts it from a terminal.
The package is not signed; `apt` installs it because you named the file.

An older Coffret installed from a `.deb` is the same package,
`coffret-desktop`, so installing this one upgrades it in place.

## The sandbox

The app runs with Chromium's sandbox, as a browser does; nothing turns it off,
and nothing needs to be turned off for it. The sandbox needs to create user
namespaces, and the package's install script sees to that on each release of
Ubuntu:

- **Ubuntu 24.04** lets an unprivileged program create user namespaces only
  where an AppArmor profile allows it. The package carries one,
  `/opt/Coffret/resources/apparmor-profile`, which allows that and nothing
  else (it confines nothing: `flags=(unconfined)`), for
  `/opt/Coffret/coffret-desktop` alone. The install script checks that this
  release of AppArmor takes the profile, copies it to
  `/etc/apparmor.d/coffret-desktop` and loads it.
- **Ubuntu 22.04** lets any program create user namespaces, and its AppArmor
  does not read the profile's format; the install script says it skips the
  profile, and the sandbox works without it.
- Where user namespaces cannot be created at all, the install script makes
  `/opt/Coffret/chrome-sandbox` setuid root instead, which is Chromium's
  other way into its sandbox. Elsewhere it is left an ordinary file.

The profile names the installed binary alone. The app run from a checkout
(`make desktop-dev`), or from an extracted `.deb`, is another binary, and on
Ubuntu 24.04 and later needs a profile of its own; [the environments
guide](../environments.md#the-development-apps-sandbox-on-linux) gives it and
the commands that install it.

## Removing

Run `sudo apt remove coffret-desktop`. That removes `/opt/Coffret/`, the
`coffret-desktop` command, the menu entry and the AppArmor profile, and leaves
the state directory and the app's browser data as they are (see [Removing the
app](README.md#removing-the-app)).

## Updating

Quit Coffret from its tray icon, then install the new `.deb` with the same
`apt install` command. The [install guide](README.md#updating) says what
happens to your state.
