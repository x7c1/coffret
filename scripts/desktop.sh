#!/usr/bin/env bash
#
# The desktop shell: building it, installing it, and running the development
# build of it. The Makefile's `desktop*` targets call this; each builds the
# explorer first (`make web-dist`), since the shell carries the built page in
# its binary.
#
#   scripts/desktop.sh build       bundle the installed app under
#                                  backend/target/release/bundle/
#   scripts/desktop.sh install     install what `build` bundled: on Linux the
#                                  newest .deb, through apt; on macOS
#                                  /Applications/Coffret.app, replaced
#   scripts/desktop.sh dev-build   the debug binary, built as the development app
#   scripts/desktop.sh dev         run the binary `dev-build` built
#
# Two apps, told apart by their identifier. The installed one carries
# tauri.conf.json's, `io.github.x7c1.coffret`, and serves the Libraries under
# the binaries' default state directory, since nothing sets another. The
# development one is built with the identifier `io.github.x7c1.coffret.dev`,
# through the TAURI_CONFIG variable, which tauri-build and `generate_context!`
# merge over the configuration files at compile time; a plain `cargo build`
# honours it, so the development build needs neither the Tauri CLI nor a second
# configuration file. Its own identifier gives it its own single-instance scope
# and its own app ID, so it runs beside the installed app rather than handing
# its launch over to it. It serves the Libraries under COFFRET_STATE_DIR, which
# the Makefile exports as the development state directory
# (docs/guides/environments.md) and which `dev` refuses to run without.
#
# tauri-build declares `rerun-if-env-changed=TAURI_CONFIG`, so switching between
# the development build and a plain one (`make check`) rebuilds the shell rather
# than reusing a binary with the other identifier.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
cd "$ROOT"

readonly CRATE_DIR="$ROOT/backend/crates/apps/coffret-desktop"
readonly TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/backend/target}"
readonly BUNDLE_DIR="$TARGET_DIR/release/bundle"
readonly DEV_IDENTIFIER="io.github.x7c1.coffret.dev"
readonly DEV_TAURI_CONFIG="{\"identifier\":\"$DEV_IDENTIFIER\"}"
readonly DEV_BIN="$TARGET_DIR/debug/coffret-desktop"

usage() {
  echo "usage: scripts/desktop.sh build|install|dev-build|dev" >&2
  exit 2
}

fail() {
  echo "error: $*" >&2
  exit 1
}

# The installed app, bundled. TAURI_CONFIG is unset so that an exported value —
# the development identifier, say — cannot leak into the bundle: the installed
# app carries tauri.conf.json's identifier.
build_app() {
  cargo tauri --version >/dev/null 2>&1 ||
    fail "the Tauri CLI is not installed; install it once with: cargo install tauri-cli --version '^2' --locked"
  (cd "$CRATE_DIR" && env -u TAURI_CONFIG cargo tauri build --features embed-web)
}

# --reinstall, because a local build keeps the release's version number, which
# apt would otherwise treat as already installed. The newest .deb is the one
# just built; older ones may still be in the bundle directory.
install_app() {
  case "$(uname -s)" in
    Linux)
      local deb
      deb="$(find "$BUNDLE_DIR/deb" -maxdepth 1 -name 'coffret-desktop_*.deb' -printf '%T@ %p\n' 2>/dev/null |
        sort -nr | head -n 1 | cut -d' ' -f2-)"
      [ -n "$deb" ] || fail "no coffret-desktop .deb under $BUNDLE_DIR/deb; run make desktop-build"
      echo "Installing $deb"
      sudo apt install --reinstall "$deb"
      ;;
    Darwin)
      local app="$BUNDLE_DIR/macos/Coffret.app"
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
  echo "Building coffret-desktop (debug) as $DEV_IDENTIFIER"
  (cd "$ROOT/backend" && TAURI_CONFIG="$DEV_TAURI_CONFIG" cargo build -p coffret-desktop --features embed-web)
}

# In the foreground, until the app is quit from its tray or its window is
# closed before a Library is open.
dev() {
  [ -n "${COFFRET_STATE_DIR:-}" ] ||
    fail "COFFRET_STATE_DIR is not set; run this through make desktop-dev, which sets it to the development state directory"
  [ -x "$DEV_BIN" ] || fail "$DEV_BIN is not built; run make desktop-dev-build"
  echo "Starting $DEV_IDENTIFIER over the Libraries under $COFFRET_STATE_DIR"
  exec "$DEV_BIN"
}

[ $# -eq 1 ] || usage
case "$1" in
  build) build_app ;;
  install) install_app ;;
  dev-build) dev_build ;;
  dev) dev ;;
  *) usage ;;
esac
