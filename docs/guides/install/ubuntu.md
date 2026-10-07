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

`apt` pulls in the WebKitGTK, GTK and app-indicator libraries the app needs.
Coffret then appears in the application menu, and the `coffret-desktop`
command starts it from a terminal. The package is not signed; `apt` installs
it because you named the file.

To uninstall it, run `sudo apt remove coffret-desktop`. That leaves the state
directory as it is (see [Removing the app](README.md#removing-the-app)).

## Updating

Quit Coffret from its tray icon, then install the new `.deb` with the same
`apt install` command. The [install guide](README.md#updating) says what
happens to your state.
