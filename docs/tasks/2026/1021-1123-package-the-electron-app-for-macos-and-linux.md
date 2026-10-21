---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "electron-builder" frontend/packages/apps/desktop/package.json && grep -rqE "runAsNode|RunAsNode" frontend/packages/apps/desktop/ --include=*.json --include=*.ts --include=*.cjs --include=*.mjs --include=*.js --exclude-dir=node_modules --exclude-dir=dist && grep -q "frontend/packages/apps/desktop" .github/workflows/bundle.yml && ! grep -q "cargo tauri build" scripts/desktop.sh'
assignee: null
branch: task/1021-1123-package-the-electron-app-for-macos-and-linux
created_at: 2026-10-21T02:23:33Z
updated_at: 2026-10-21T03:06:01Z
---

# feat(desktop): package the Electron app for macOS and Linux, with its fuses off and a hardened runtime

## Overview

The Electron app (`frontend/packages/apps/desktop`) runs only from a checkout today, through `make desktop-electron-dev`. The installed app and the release bundles are still the Tauri app's: `scripts/desktop.sh build|install|dev-build|dev`, `make desktop*`, and `.github/workflows/bundle.yml`, which `release.yml` calls. This change makes the Electron app the one that is built, installed and released. The Tauri crate stays in the workspace until a later change removes it, but nothing builds or ships it any more.

### Decisions (already made; implement them)

1. **Builder.** Use electron-builder, pinned exactly and kept within pnpm's minimum-release-age gate. Its config lives in the desktop package. It produces:
   - a **macOS** `.dmg` for arm64, named `Coffret_<version>_aarch64.dmg` as the Tauri bundle is today, so the release guide's names stay true;
   - a **Linux** `.deb` for amd64, named `coffret-desktop_<version>_amd64.deb`, with the package name `coffret-desktop` kept so `apt` upgrades the installed Tauri package in place. Its desktop entry carries `StartupWMClass=io.github.x7c1.coffret`.

   The addon (`coffret.node`, built with the explorer embedded) and the assets ship as extra resources. They are not inside the asar.
2. **Version.** The bundle's version is the workspace version from `backend/Cargo.toml`, read with `cargo metadata` at build time and passed to electron-builder. `package.json` keeps a placeholder. The release-PR bump stays `cargo set-version --workspace`.
   - `scripts/check-version-change.sh` reads the version through a crate that survives the Tauri crate's removal, such as `coffret-desktop-core`. Change it in this PR so the next change does not break it.
3. **Fuses.** The shipped app turns off `RunAsNode`, `EnableNodeOptionsEnvironmentVariable` and `EnableNodeCliInspectArguments` with `@electron/fuses`, through electron-builder's `electronFuses` or an `afterPack` hook. Turn `OnlyLoadAppFromAsar` on if the app layout allows it; say which in the PR.
4. **macOS signing.** The app is signed ad-hoc (`-`) with the hardened runtime and an entitlements file that allows JIT (`com.apple.security.cs.allow-jit`). If the unsigned addon cannot load under library validation, also add `com.apple.security.cs.disable-library-validation`, and say so in the PR with the reason. No Developer ID and no notarization.
5. **Linux sandbox.** Make the installed `.deb` start with Chromium's sandbox on Ubuntu 22.04 and 24.04. Do not ship `--no-sandbox`.
   - Use electron-builder's chrome-sandbox SUID handling or AppArmor profile, whichever its pinned version provides for 24.04's restricted user namespaces.
   - Document what it does in `docs/guides/install/ubuntu.md`.
6. **`scripts/desktop.sh` and the Makefile.**
   - `build` builds the Electron bundles for the host OS: `make web-dist`, `make napi-addon`, the desktop package, then electron-builder.
   - `install` installs them as today, with the same paths and the same apt staging: on Linux the newest `.deb` through apt; on macOS `/Applications/Coffret.app`, replaced.
   - `dev-build` / `dev` become what `desktop-electron-dev` does today. Keep `desktop-electron-dev` as an alias or fold it into `desktop-dev`, and say which.
   - Remove the Tauri-only toolchain steps (`cargo tauri`, `TAURI_CONFIG`, the GTK loader guard), keeping any check that still applies to the `.node`.
   - Update `make help` lines accordingly.
7. **CI.** `.github/workflows/bundle.yml` builds the Electron bundles on `macos-14` and `ubuntu-22.04`.
   - It installs Node and pnpm and builds the addon.
   - It drops the Tauri CLI and the WebKitGTK apt packages from this workflow only. `ci.yml` and `toolchain-drift.yml` still build the Tauri crate until it is removed.
   - Its `paths` filter includes the desktop package, the addon crates and the scripts that build them.
   - It gathers exactly one `.dmg` or `.deb` per OS and uploads it as before.
   - It asserts the packaged result:
     - the fuses read as off (`npx @electron/fuses read --app <path>`);
     - on macOS, `codesign -dv` shows the ad-hoc signature with `runtime`, and the entitlements include allow-jit;
     - on Linux, `dpkg-deb -I` shows the package name and the workspace version, and `dpkg-deb -c` contains the desktop entry.

   Remove `ELECTRON_SKIP_BINARY_DOWNLOAD` from this workflow, since it now runs electron-builder. `release.yml` keeps calling it unchanged.
8. **Docs.** Update the release guide and the install guides for the Electron bundle:
   - `docs/guides/release.md`: names, how they are made, what CI asserts;
   - `docs/guides/install/README.md`, `macos.md` and `ubuntu.md`: what the user does on first launch with an ad-hoc signed app, what apt pulls in, and how to remove it;
   - `docs/guides/environments.md`: the targets table and the Electron section.

### Out of scope

- Removing the Tauri crate, the launch file, and the Tauri-only workspace dependencies and CI packages; a later change does that.
- Developer ID signing and notarization.
- Windows.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The desktop package declares electron-builder (gated), and its packaging config turns off the RunAsNode fuse among others (gated).
- [x] `bundle.yml` builds from the desktop package (gated), and `scripts/desktop.sh` no longer runs `cargo tauri build` (gated).
- [x] `scripts/check-version-change.sh` still reports the workspace version without relying on the Tauri crate. Its existing test passes, or a test is added that runs it with the Tauri crate absent from `cargo metadata`.
- [x] `shellcheck` and `shfmt` pass on the changed scripts (`make shell-lint` in `make check`).

### Before merge (verified outside the check command)

- [ ] This PR's `bundle` workflow run is green on both OSes, and its assertions show:
  - on macOS, fuses off, an ad-hoc `runtime` signature and allow-jit;
  - on Linux, the `.deb` name, package name, version and desktop entry.
- [ ] On macOS, `make desktop-build` produces `Coffret_<version>_aarch64.dmg`. Run from its mounted image or extracted `.app` with `COFFRET_STATE_DIR` and `COFFRET_LOG_DIR` pointed at the agent's state directory and `--user-data-dir` at a temporary directory (never the production state directory), the app:
  - starts, loads the addon and opens its Passphrase window, with the door listening;
  - refuses `ELECTRON_RUN_AS_NODE=1` and `--inspect` (the fuses hold).

  The merging session records this in the PR, and does not replace `/Applications/Coffret.app`. Typing the Passphrase into the packaged app's window to open the Library is left to a person: this machine refuses an agent's keystrokes and screen capture. The same flow, the Passphrase window through to the explorer, is verified on the unpackaged app, which differs from the packaged one only in where it finds the addon and assets (`src/resources.ts`, unit-tested).
