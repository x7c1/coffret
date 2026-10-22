#!/usr/bin/env bash
#
# Show that the check `scripts/desktop.sh dev` makes on Linux before it starts
# the development app lets it start where Chromium's sandbox can, and refuses,
# saying what to run, where it cannot.
#
# Each case runs the check alone (`scripts/desktop.sh dev-sandbox-check`), never
# Electron, over a throwaway directory of its own: a restriction file standing
# in for /proc/sys/kernel/apparmor_restrict_unprivileged_userns, a profile
# directory standing in for /etc/apparmor.d, and a checkout holding the path of
# an Electron binary and its chrome-sandbox, handed to the check through the
# variables scripts/desktop.sh documents for its test. The host's own state is
# never read, so the cases answer the same on any host.
#
# A throwaway chrome-sandbox cannot be made root's without root, so the
# setuid case makes it setuid and the user's own, and sets
# COFFRET_SANDBOX_HELPER_OWNER to the user's ID, the owner the check then wants
# in place of root. The case after it keeps the default owner, root, and
# expects the same file to be refused, so it is the owner, not the bit alone,
# that lets the first one through.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly DESKTOP="$ROOT/scripts/desktop.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

readonly ELECTRON_SUFFIX="frontend/node_modules/.pnpm/electron@1.2.3/node_modules/electron/dist/electron"
readonly ATTACHMENT_SUFFIX="frontend/node_modules/.pnpm/electron@*/node_modules/electron/dist/electron"

# Sets up case $1 with the restriction $2 ("missing" for no file): an empty
# profile directory and a checkout whose chrome-sandbox is an ordinary file.
setup() {
  local dir="$work/$1" restriction="$2"
  mkdir -p "$dir/apparmor.d" "$(dirname "$dir/checkout/$ELECTRON_SUFFIX")"
  : >"$(dirname "$dir/checkout/$ELECTRON_SUFFIX")/chrome-sandbox"
  chmod 0755 "$(dirname "$dir/checkout/$ELECTRON_SUFFIX")/chrome-sandbox"
  [ "$restriction" = missing ] || echo "$restriction" >"$dir/restrict"
}

# Writes a profile named $2 attaching to $3 into case $1's profile directory,
# in the form the installed app's and the development app's profiles take.
profile() {
  local dir="$work/$1" name="$2" attachment="$3"
  printf 'abi <abi/4.0>,\ninclude <tunables/global>\n\nprofile %s %s flags=(unconfined) {\n  userns,\n}\n' \
    "$name" "$attachment" >"$dir/apparmor.d/$name"
}

# Runs the check over case $1, with the variables after it (NAME=value) set
# too.
run_check() {
  local name="$1" dir="$work/$1"
  shift
  env COFFRET_USERNS_RESTRICT_FILE="$dir/restrict" \
    COFFRET_APPARMOR_DIR="$dir/apparmor.d" \
    COFFRET_ELECTRON_BINARY="$dir/checkout/$ELECTRON_SUFFIX" \
    "$@" "$DESKTOP" dev-sandbox-check >"$work/$name.log" 2>&1
}

# Expects the check over case $1, with the variables after it set too, to let
# the app start.
expect_start() {
  local name="$1"
  shift
  if ! run_check "$name" "$@"; then
    echo "FAIL $name: the check refused"
    cat "$work/$name.log"
    failures=$((failures + 1))
    return
  fi
  echo "ok   $name"
}

# Expects the check over case $1 to refuse, saying each of the lines after it.
expect_refusal() {
  local name="$1" said
  shift
  if run_check "$name"; then
    echo "FAIL $name: the check let the app start"
    cat "$work/$name.log"
    failures=$((failures + 1))
    return
  fi
  for said in "$@"; do
    if ! grep -qF -- "$said" "$work/$name.log"; then
      echo "FAIL $name: the check refused without saying: $said"
      cat "$work/$name.log"
      failures=$((failures + 1))
      return
    fi
  done
  echo "ok   $name"
}

# Ubuntu 22.04, other distributions, macOS.
setup restriction-missing missing
expect_start restriction-missing

setup restriction-off 0
expect_start restriction-off

setup no-profile 1
expect_refusal no-profile \
  "$work/no-profile/checkout/$ELECTRON_SUFFIX" \
  "profile coffret-desktop-dev $work/no-profile/checkout/$ATTACHMENT_SUFFIX flags=(unconfined) {" \
  "sudo tee $work/no-profile/apparmor.d/coffret-desktop-dev" \
  "sudo apparmor_parser -r $work/no-profile/apparmor.d/coffret-desktop-dev"

setup matching-profile 1
profile matching-profile coffret-desktop-dev "$work/matching-profile/checkout/$ATTACHMENT_SUFFIX"
expect_start matching-profile

# The installed app's profile, as its .deb installs it, names its binary alone.
setup installed-app-profile-only 1
printf 'abi <abi/4.0>,\ninclude <tunables/global>\n\nprofile "coffret-desktop" "/opt/Coffret/coffret-desktop" flags=(unconfined) {\n  userns,\n}\n' \
  >"$work/installed-app-profile-only/apparmor.d/coffret-desktop"
expect_refusal installed-app-profile-only \
  "sudo tee $work/installed-app-profile-only/apparmor.d/coffret-desktop-dev"

# Another checkout's profile already takes the name, so this checkout's is
# named after it.
setup other-checkout-profile 1
profile other-checkout-profile coffret-desktop-dev "/elsewhere/coffret/$ATTACHMENT_SUFFIX"
expect_refusal other-checkout-profile \
  "profile coffret-desktop-dev-checkout $work/other-checkout-profile/checkout/$ATTACHMENT_SUFFIX flags=(unconfined) {" \
  "sudo tee $work/other-checkout-profile/apparmor.d/coffret-desktop-dev-checkout"

setup setuid-sandbox 1
chmod 4755 "$(dirname "$work/setuid-sandbox/checkout/$ELECTRON_SUFFIX")/chrome-sandbox"
expect_start setuid-sandbox COFFRET_SANDBOX_HELPER_OWNER="$(id -u)"

setup setuid-sandbox-not-root 1
chmod 4755 "$(dirname "$work/setuid-sandbox-not-root/checkout/$ELECTRON_SUFFIX")/chrome-sandbox"
expect_refusal setuid-sandbox-not-root \
  "sudo apparmor_parser -r $work/setuid-sandbox-not-root/apparmor.d/coffret-desktop-dev"

if [ "$failures" -ne 0 ]; then
  echo "desktop-dev-sandbox-test: $failures case(s) failed" >&2
  exit 1
fi
echo "desktop-dev-sandbox-test: every case answered as expected"
