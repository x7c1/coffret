#!/usr/bin/env bash
#
# Check a desktop bundle that scripts/desktop.sh built, against what
# frontend/packages/apps/desktop/electron-builder.config.js says it is, and
# fail on the first thing that does not hold. `scripts/desktop.sh build` runs
# it on the bundle it made, so a local build and the Bundle workflow check the
# same things.
#
#   scripts/check-desktop-bundle.sh <bundle> <version>
#
# For either bundle, the fuses of the Electron binary in it read as set:
# RunAsNode, EnableNodeOptionsEnvironmentVariable and
# EnableNodeCliInspectArguments off, OnlyLoadAppFromAsar on. The addon and the
# icons are in it, outside the asar archive.
#
# A .dmg (macOS) is attached read-only, and the Coffret.app in it is checked:
# its name carries <version>, its signature is ad hoc with the hardened runtime
# (`codesign -dv` says `adhoc,runtime`), and its entitlements include allow-jit.
#
# A .deb (Linux) is read with dpkg-deb: its package name is coffret-desktop and
# its version <version>, and it holds the desktop entry, with the installed
# identifier as its StartupWMClass, chrome-sandbox, and the AppArmor profile its
# install script loads where AppArmor takes it.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly FUSES="$ROOT/frontend/packages/apps/desktop/node_modules/.bin/electron-fuses"
readonly IDENTIFIER="io.github.x7c1.coffret"

fail() {
  echo "check-desktop-bundle: $*" >&2
  exit 1
}

ok() {
  echo "ok   $*"
}

[ $# -eq 2 ] || {
  echo "usage: scripts/check-desktop-bundle.sh <bundle> <version>" >&2
  exit 2
}
readonly BUNDLE="$1" VERSION="$2"
[ -f "$BUNDLE" ] || fail "$BUNDLE is not a file"
[ -x "$FUSES" ] || fail "$FUSES is missing; run \`pnpm install\` in frontend/ first"

# The fuses of the Electron binary at $1 (the .app on macOS, the executable on
# Linux), as `electron-fuses read` prints them: "  RunAsNode is Disabled".
check_fuses() {
  local app="$1" read fuse
  read="$("$FUSES" read --app "$app")" || fail "the fuses of $app cannot be read"
  echo "$read"
  for fuse in RunAsNode EnableNodeOptionsEnvironmentVariable EnableNodeCliInspectArguments; do
    grep -qx "  *$fuse is Disabled" <<<"$read" || fail "the fuse $fuse of $app is not disabled"
    ok "fuse $fuse is disabled"
  done
  grep -qx "  *OnlyLoadAppFromAsar is Enabled" <<<"$read" || fail "the fuse OnlyLoadAppFromAsar of $app is not enabled"
  ok "fuse OnlyLoadAppFromAsar is enabled"
}

# The files the app loads from its resources directory $1.
check_resources() {
  local resources="$1" file
  for file in app.asar coffret.node assets/tray.png; do
    [ -f "$resources/$file" ] || fail "$resources/$file is missing"
  done
  ok "the resources hold app.asar, coffret.node and assets/"
}

MOUNT=""
detach() {
  [ -z "$MOUNT" ] || hdiutil detach "$MOUNT" -quiet || hdiutil detach "$MOUNT" -force -quiet || true
  [ -z "$MOUNT" ] || rmdir "$MOUNT" 2>/dev/null || true
}

check_dmg() {
  local app signature flags flag entitlements
  [ "$(basename "$BUNDLE")" = "Coffret_${VERSION}_aarch64.dmg" ] ||
    fail "$BUNDLE is not named Coffret_${VERSION}_aarch64.dmg"
  ok "the bundle is named Coffret_${VERSION}_aarch64.dmg"
  MOUNT="$(mktemp -d "${TMPDIR:-/tmp}/coffret-dmg.XXXXXX")"
  trap detach EXIT
  hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$MOUNT" "$BUNDLE" >/dev/null ||
    fail "$BUNDLE cannot be attached"
  app="$MOUNT/Coffret.app"
  [ -d "$app" ] || fail "$BUNDLE holds no Coffret.app"

  signature="$(codesign -dv --verbose=2 "$app" 2>&1)" || fail "codesign cannot read the signature of Coffret.app: $signature"
  echo "$signature"
  grep -q '^Signature=adhoc$' <<<"$signature" || fail "Coffret.app is not signed ad hoc"
  # "CodeDirectory v=… flags=0x10002(adhoc,runtime) …"
  flags="$(sed -n 's/^CodeDirectory .* flags=0x[0-9a-f]*(\([^)]*\)).*/\1/p' <<<"$signature")"
  for flag in adhoc runtime; do
    case ",$flags," in
      *",$flag,"*) ;;
      *) fail "Coffret.app's signature flags ($flags) do not include $flag" ;;
    esac
  done
  grep -q "^Identifier=$IDENTIFIER$" <<<"$signature" || fail "Coffret.app is not signed as $IDENTIFIER"
  ok "Coffret.app is signed ad hoc, as $IDENTIFIER, with the hardened runtime"
  codesign --verify --deep --strict "$app" || fail "Coffret.app's signature does not verify"
  ok "Coffret.app's signature verifies"

  entitlements="$(codesign -d --entitlements - --xml "$app" 2>/dev/null)" ||
    fail "codesign cannot read the entitlements of Coffret.app"
  grep -q '<key>com.apple.security.cs.allow-jit</key><true/>' <<<"$entitlements" ||
    fail "Coffret.app's entitlements do not allow JIT"
  ok "Coffret.app's entitlements allow JIT"

  check_fuses "$app"
  check_resources "$app/Contents/Resources"
}

check_deb() {
  local fields contents root entry path
  [ "$(basename "$BUNDLE")" = "coffret-desktop_${VERSION}_amd64.deb" ] ||
    fail "$BUNDLE is not named coffret-desktop_${VERSION}_amd64.deb"
  ok "the bundle is named coffret-desktop_${VERSION}_amd64.deb"
  fields="$(dpkg-deb -I "$BUNDLE")" || fail "dpkg-deb cannot read $BUNDLE"
  echo "$fields"
  [ "$(dpkg-deb -f "$BUNDLE" Package)" = coffret-desktop ] || fail "$BUNDLE is not the package coffret-desktop"
  [ "$(dpkg-deb -f "$BUNDLE" Version)" = "$VERSION" ] || fail "$BUNDLE is not version $VERSION"
  [ "$(dpkg-deb -f "$BUNDLE" Architecture)" = amd64 ] || fail "$BUNDLE is not for amd64"
  ok "the package is coffret-desktop $VERSION for amd64"

  contents="$(dpkg-deb -c "$BUNDLE")" || fail "dpkg-deb cannot list $BUNDLE"
  for path in ./usr/share/applications/coffret-desktop.desktop ./opt/Coffret/coffret-desktop \
    ./opt/Coffret/chrome-sandbox ./opt/Coffret/resources/apparmor-profile; do
    grep -q " $path\$" <<<"$contents" || fail "$BUNDLE does not hold $path"
    ok "the package holds $path"
  done

  root="$(mktemp -d "${TMPDIR:-/tmp}/coffret-deb.XXXXXX")"
  # shellcheck disable=SC2064 # the directory is fixed here, once
  trap "rm -rf '$root'" EXIT
  dpkg-deb -x "$BUNDLE" "$root"
  entry="$root/usr/share/applications/coffret-desktop.desktop"
  cat "$entry"
  grep -qx "StartupWMClass=$IDENTIFIER" "$entry" || fail "the desktop entry's StartupWMClass is not $IDENTIFIER"
  ok "the desktop entry's StartupWMClass is $IDENTIFIER"
  if grep -q -- '--no-sandbox' "$entry"; then
    fail "the desktop entry starts the app with --no-sandbox"
  fi
  ok "the desktop entry does not turn the sandbox off"

  check_fuses "$root/opt/Coffret/coffret-desktop"
  check_resources "$root/opt/Coffret/resources"
}

case "$BUNDLE" in
  *.dmg) check_dmg ;;
  *.deb) check_deb ;;
  *) fail "$BUNDLE is neither a .dmg nor a .deb" ;;
esac
echo "check-desktop-bundle: $(basename "$BUNDLE") is as configured"
