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
#                                  newest .deb, through apt, from a copy in a
#                                  temporary directory apt can read; on macOS
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
#
# On Linux both builds link with the system's C compiler, /usr/bin/cc, whatever
# PATH puts first. The shell loads the distribution's GTK and WebKitGTK, so it
# must be loaded by the distribution's dynamic loader; a compiler from a Nix
# profile, say, would make the binary ask for Nix's loader, which does not read
# the host's library cache and so never finds the libraries GTK itself needs.
# Each build then refuses a binary the system cannot load (check_loadable).

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
readonly RELEASE_BIN="$TARGET_DIR/release/coffret-desktop"
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
# compiler: CC for the build scripts that compile C, the linker for the binary.
use_system_toolchain() {
  [ "$(uname -s)" = Linux ] || return 0
  [ -x "$SYSTEM_CC" ] ||
    fail "$SYSTEM_CC is missing; the desktop app links with the system's C compiler on Linux, so install it once with: sudo apt install build-essential"
  export CC="$SYSTEM_CC"
  export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$SYSTEM_CC"
}

# Refuses a binary the system cannot load: on Linux its program interpreter
# must be the system's (under /lib64 or /lib, not /nix/store), and every library
# it needs must resolve through that interpreter from an empty environment, as
# when the app is started from the desktop. The loader's own exit status is
# what counts; its message for a missing library varies.
check_loadable() {
  local bin="$1" headers interp
  [ "$(uname -s)" = Linux ] || return 0
  command -v readelf >/dev/null 2>&1 ||
    fail "readelf is missing; install binutils once with: sudo apt install build-essential"
  [ -f "$bin" ] || fail "$bin is not built"
  # LC_ALL=C: a translated readelf words the interpreter line differently.
  headers="$(LC_ALL=C readelf -l "$bin")" || fail "readelf cannot read $bin as an ELF binary"
  interp="$(sed -n 's/.*Requesting program interpreter: \(.*\)]$/\1/p' <<<"$headers")"
  [ -n "$interp" ] || fail "$bin names no program interpreter"
  case "$interp" in
    /lib64/* | /lib/*) ;;
    *) fail "$bin asks for the program interpreter $interp, not the system's under /lib64 or /lib; it was linked with a compiler other than $SYSTEM_CC" ;;
  esac
  env -i "$interp" --list "$bin" >/dev/null ||
    fail "$bin does not load with the program interpreter $interp from an empty environment; a library it needs is missing"
  echo "$bin loads with $interp"
}

# The installed app, bundled. TAURI_CONFIG is unset so that an exported value —
# the development identifier, say — cannot leak into the bundle: the installed
# app carries tauri.conf.json's identifier.
build_app() {
  cargo tauri --version >/dev/null 2>&1 ||
    fail "the Tauri CLI is not installed; install it once with: cargo install tauri-cli --version '^2' --locked"
  use_system_toolchain
  (cd "$CRATE_DIR" && env -u TAURI_CONFIG cargo tauri build --features embed-web)
  check_loadable "$RELEASE_BIN"
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
# just built; older ones may still be in the bundle directory.
install_app() {
  case "$(uname -s)" in
    Linux)
      local deb
      deb="$(find "$BUNDLE_DIR/deb" -maxdepth 1 -name 'coffret-desktop_*.deb' -printf '%T@ %p\n' 2>/dev/null |
        sort -nr | head -n 1 | cut -d' ' -f2-)"
      [ -n "$deb" ] || fail "no coffret-desktop .deb under $BUNDLE_DIR/deb; run make desktop-build"
      # Under /tmp rather than TMPDIR, which may itself be private.
      STAGE_DIR="$(mktemp -d /tmp/coffret-desktop-install.XXXXXX)"
      trap remove_stage_dir EXIT
      # The binary the .deb carries, not the one in the target directory, which
      # a later build may have replaced.
      dpkg-deb -x "$deb" "$STAGE_DIR/root"
      check_loadable "$STAGE_DIR/root/usr/bin/coffret-desktop"
      local staged
      staged="$(stage_deb "$deb" "$STAGE_DIR")"
      echo "Installing $deb"
      sudo apt install --reinstall "$staged"
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
  use_system_toolchain
  (cd "$ROOT/backend" && TAURI_CONFIG="$DEV_TAURI_CONFIG" cargo build -p coffret-desktop --features embed-web)
  check_loadable "$DEV_BIN"
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
