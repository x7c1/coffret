#!/usr/bin/env bash
#
# The desktop app (frontend/packages/apps/desktop, on Electron): bundling it,
# installing it, and running the development app from the checkout. The
# Makefile's `desktop*` targets call this.
#
#   scripts/desktop.sh build       bundle the installed app for this host
#                                  under frontend/packages/apps/desktop/release/:
#                                  on macOS Coffret_<version>_aarch64.dmg (and
#                                  mac-arm64/Coffret.app), on Linux
#                                  coffret-desktop_<version>_amd64.deb
#   scripts/desktop.sh install     install what `build` bundled: on Linux the
#                                  newest .deb, through apt, from a copy in a
#                                  temporary directory apt can read; on macOS
#                                  /Applications/Coffret.app, replaced
#   scripts/desktop.sh dev-build   build what `dev` runs: the explorer, the
#                                  Node-API addon and the app's code
#   scripts/desktop.sh dev         run the app from the checkout, as the
#                                  development app
#
# The app runs the server in its own process through the Node-API addon
# (coffret-napi, built by `make napi-addon` as backend/target/release/coffret.node
# with the explorer compiled in). `build` makes the explorer and the addon, the
# app's code, and then the bundle, with electron-builder and the configuration
# in the package (electron-builder.config.js), which says what the bundle
# carries and how it is signed and locked down. It then checks the bundle
# against that (scripts/check-desktop-bundle.sh).
#
# The bundle's version is the workspace version of backend/Cargo.toml, read
# through `cargo metadata` (scripts/workspace-version.sh) and handed to
# electron-builder as COFFRET_BUNDLE_VERSION; the package's own version is a
# placeholder. A release raises the workspace version, so the bundles of a
# release carry its number.
#
# Two apps, told apart by their identifier, which the app takes from how it is
# run (src/settings.ts). The installed one, from a bundle, is
# `io.github.x7c1.coffret`, and serves the Libraries under the binaries'
# default state directory, since nothing sets another. The development one,
# run from the checkout by `dev`, is `io.github.x7c1.coffret.dev`: it carries
# the badged icons, its own single-instance scope and its own app ID, so it
# runs beside the installed app rather than handing its launch over to it, and
# it serves the Libraries under COFFRET_STATE_DIR, which the Makefile exports
# as the development state directory (docs/guides/environments.md) and without
# which both `dev` and the app refuse to run.
#
# On Linux the addon is built with the system's C compiler, /usr/bin/cc,
# whatever PATH puts first, and refused where the system cannot load it
# (check_loadable): Electron is loaded by the distribution's dynamic loader, so
# the addon must be too. A compiler from a Nix profile, say, would make it ask
# for libraries the host's loader does not find.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
cd "$ROOT"

readonly APP_DIR="$ROOT/frontend/packages/apps/desktop"
readonly RELEASE_DIR="$APP_DIR/release"
readonly NAPI_ADDON="$ROOT/backend/target/release/coffret.node"
readonly DEV_IDENTIFIER="io.github.x7c1.coffret.dev"
readonly SYSTEM_CC="/usr/bin/cc"

usage() {
  echo "usage: scripts/desktop.sh build|install|dev-build|dev" >&2
  exit 2
}

fail() {
  echo "error: $*" >&2
  exit 1
}

# On Linux, makes the builds that follow compile C and link with the system's
# compiler: CC for the build scripts that compile C, the linker for the addon.
use_system_toolchain() {
  [ "$(uname -s)" = Linux ] || return 0
  [ -x "$SYSTEM_CC" ] ||
    fail "$SYSTEM_CC is missing; the desktop app's addon links with the system's C compiler on Linux, so install it once with: sudo apt install build-essential"
  export CC="$SYSTEM_CC"
  export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$SYSTEM_CC"
}

# The program interpreter an ELF executable asks for, which must be the
# system's (under /lib64 or /lib, not /nix/store).
interpreter_of() {
  local bin="$1" headers interp
  # LC_ALL=C: a translated readelf words the interpreter line differently.
  headers="$(LC_ALL=C readelf -l "$bin")" || fail "readelf cannot read $bin as an ELF binary"
  interp="$(sed -n 's/.*Requesting program interpreter: \(.*\)]$/\1/p' <<<"$headers")"
  [ -n "$interp" ] || fail "$bin names no program interpreter"
  case "$interp" in
    /lib64/* | /lib/*) ;;
    *) fail "$bin asks for the program interpreter $interp, not the system's under /lib64 or /lib" ;;
  esac
  echo "$interp"
}

# Refuses, on Linux, an addon the app cannot load: every library it needs
# must resolve through the program interpreter of the executable that loads
# it, `electron`, from an empty environment, as when the app is started
# from the desktop. The loader's own exit status is what counts; its message
# for a missing library varies.
check_loadable() {
  local addon="$1" electron="$2" interp
  [ "$(uname -s)" = Linux ] || return 0
  command -v readelf >/dev/null 2>&1 ||
    fail "readelf is missing; install binutils once with: sudo apt install build-essential"
  [ -f "$addon" ] || fail "$addon is not built"
  [ -f "$electron" ] || fail "$electron is missing"
  interp="$(interpreter_of "$electron")"
  env -i "$interp" --list "$addon" >/dev/null ||
    fail "$addon does not load with the program interpreter $interp from an empty environment; a library it needs is missing, or it was linked with a compiler other than $SYSTEM_CC"
  echo "$addon loads with $interp"
}

# The frontend's dependencies, electron and electron-builder among them.
require_frontend_dependencies() {
  [ -x "$APP_DIR/node_modules/.bin/electron" ] ||
    fail "the frontend's dependencies are not installed; run \`pnpm install\` in frontend/ first"
}

# The explorer, the addon with the explorer compiled in, and the app's code.
build_parts() {
  require_frontend_dependencies
  use_system_toolchain
  make --no-print-directory -C "$ROOT" napi-addon
  (cd "$ROOT/frontend" && pnpm --filter @coffret/desktop build)
}

# The installed app, bundled for this host. The release directory is emptied
# first, so that what is left in it is this build's alone.
build_app() {
  local version targets
  case "$(uname -s)" in
    Linux) targets=(--linux deb --x64) ;;
    Darwin) targets=(--mac dmg --arm64) ;;
    *) fail "the desktop app is bundled on Linux and macOS only" ;;
  esac
  version="$("$ROOT/scripts/workspace-version.sh" "$ROOT/backend")"
  build_parts
  rm -rf "$RELEASE_DIR"
  echo "Bundling Coffret $version"
  (cd "$APP_DIR" &&
    COFFRET_BUNDLE_VERSION="$version" pnpm exec electron-builder \
      --config electron-builder.config.js --publish never "${targets[@]}")
  if [ "$(uname -s)" = Linux ]; then
    check_loadable "$RELEASE_DIR/linux-unpacked/resources/coffret.node" \
      "$RELEASE_DIR/linux-unpacked/coffret-desktop"
  fi
  "$ROOT/scripts/check-desktop-bundle.sh" "$(the_bundle)" "$version"
}

# The one bundle `build` made, a .dmg on macOS and a .deb on Linux, refusing
# none or more than one.
the_bundle() {
  local bundles
  shopt -s nullglob
  bundles=("$RELEASE_DIR"/*.dmg "$RELEASE_DIR"/*.deb)
  shopt -u nullglob
  [ "${#bundles[@]}" -eq 1 ] ||
    fail "expected one .dmg or .deb under $RELEASE_DIR, found ${#bundles[@]}: ${bundles[*]}"
  echo "${bundles[0]}"
}

# A fresh directory apt's unprivileged `_apt` user can read, holding a copy of
# the .deb, so that apt does not warn that the download is performed unsandboxed
# because the file under the home directory is out of `_apt`'s reach. Prints
# the copy's path; the caller removes the directory. The steps are chained
# because the caller reads the path through a command substitution, where
# `set -e` does not hold: a failed copy must fail the substitution rather than
# print a path.
stage_deb() {
  local deb="$1" dir="$2"
  chmod 0755 "$dir" &&
    install -m 0644 "$deb" "$dir/" &&
    echo "$dir/$(basename "$deb")"
}

STAGE_DIR=""
remove_stage_dir() {
  [ -z "$STAGE_DIR" ] || rm -rf "$STAGE_DIR"
}

# --reinstall, because a local build keeps the release's version number, which
# apt would otherwise treat as already installed. The newest .deb is the one
# just built.
install_app() {
  case "$(uname -s)" in
    Linux)
      local deb
      deb="$(find "$RELEASE_DIR" -maxdepth 1 -name 'coffret-desktop_*.deb' -printf '%T@ %p\n' 2>/dev/null |
        sort -nr | head -n 1 | cut -d' ' -f2-)"
      [ -n "$deb" ] || fail "no coffret-desktop .deb under $RELEASE_DIR; run make desktop-build"
      # Under /tmp rather than TMPDIR, which may itself be private.
      STAGE_DIR="$(mktemp -d /tmp/coffret-desktop-install.XXXXXX)"
      trap remove_stage_dir EXIT
      # The addon the .deb carries, not the one in the target directory, which
      # a later build may have replaced.
      dpkg-deb -x "$deb" "$STAGE_DIR/root"
      check_loadable "$STAGE_DIR/root/opt/Coffret/resources/coffret.node" \
        "$STAGE_DIR/root/opt/Coffret/coffret-desktop"
      local staged
      staged="$(stage_deb "$deb" "$STAGE_DIR")"
      echo "Installing $deb"
      sudo apt install --reinstall "$staged"
      ;;
    Darwin)
      local app="$RELEASE_DIR/mac-arm64/Coffret.app"
      [ -d "$app" ] || fail "no $app; run make desktop-build"
      echo "Installing $app to /Applications"
      rm -rf /Applications/Coffret.app
      ditto "$app" /Applications/Coffret.app
      ;;
    *)
      fail "the desktop app installs on Linux and macOS only; make desktop-build bundles it"
      ;;
  esac
  echo "Installed. If Coffret is running, quit it from its tray icon and start it again to use the new build."
}

dev_build() {
  build_parts
  if [ "$(uname -s)" = Linux ]; then
    # The Electron package fetches its binary only once it is first run, so
    # the system's own /bin/sh stands in for it here: Electron's release
    # binaries ask for the same program interpreter.
    check_loadable "$NAPI_ADDON" /bin/sh
  fi
}

# The app from the checkout, in the foreground until it quits. The Electron
# package fetches its binary the first time it is run, so the first run needs
# the network.
dev() {
  [ -n "${COFFRET_STATE_DIR:-}" ] ||
    fail "COFFRET_STATE_DIR is not set; run this through make desktop-dev, which sets it to the development state directory"
  require_frontend_dependencies
  [ -f "$NAPI_ADDON" ] || fail "$NAPI_ADDON is not built; run make desktop-dev-build"
  [ -f "$APP_DIR/dist/main.js" ] || fail "the app's code is not built; run make desktop-dev-build"
  echo "Starting $DEV_IDENTIFIER over the Libraries under $COFFRET_STATE_DIR"
  # Run from the repository root, so that a relative COFFRET_STATE_DIR or
  # COFFRET_LOG_DIR means a directory under it.
  exec "$APP_DIR/node_modules/.bin/electron" "$APP_DIR"
}

[ $# -eq 1 ] || usage
case "$1" in
  build) build_app ;;
  install) install_app ;;
  dev-build) dev_build ;;
  dev) dev ;;
  *) usage ;;
esac
