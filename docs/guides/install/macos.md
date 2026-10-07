# Install on macOS

The macOS bundle runs on Apple silicon (M1 and later). What the app does and
where it keeps its state are in the [install guide](README.md).

## Download and install

1. Open the [latest Release](https://github.com/x7c1/coffret/releases/latest)
   and download `Coffret_<version>_aarch64.dmg`.
2. Open the `.dmg` and drag `Coffret.app` into `Applications`.

## First launch: getting past Gatekeeper

Open `Coffret.app`. The first time, macOS refuses and reports that the app "is
damaged and can't be opened". Nothing is wrong with the download: the app is
neither signed by an identified developer nor notarized by Apple, and
Gatekeeper, the macOS check for downloaded apps, describes that as damage.

Clear the quarantine attribute macOS put on the download; that is what
triggers the check. You only need this once per downloaded copy:

```bash
xattr -d com.apple.quarantine /Applications/Coffret.app
```

If you put the app somewhere other than `Applications`, pass that path
instead (for example `~/Applications/Coffret.app`). Then open `Coffret.app`
normally.

If macOS instead says that Apple "could not verify" the app, it offers a way
through System Settings: dismiss the dialog (**Done**), go to **System
Settings → Privacy & Security**, click **Open Anyway** next to the message
that Coffret was blocked, then open the app again and confirm with your
password. On macOS 14 and earlier, right-click `Coffret.app` in `Applications`
and choose **Open** instead. The `xattr` command above works in this case
too.

## Updating

Quit Coffret from its tray icon, drag the new `Coffret.app` from the new
`.dmg` over the old one, then clear the quarantine attribute again. The
[install guide](README.md#updating) says what happens to your state.
