#!/usr/bin/env bash
#
# Show that scripts/e2e-electron.sh picks where the desktop app is shown as its
# header says: the current display where there is one and the screen is not
# blanked, a headless weston of its own where there is none or it is blanked,
# and a refusal that says why where weston is not there to fall back on.
#
# Each case runs the choice alone (`scripts/e2e-electron.sh choose-display`),
# never weston or Electron, with stand-ins handed over through the variables
# the script reads for this test: COFFRET_E2E_WESTON for the weston it would
# start ("" for none), and COFFRET_E2E_LOGINCTL for a loginctl that answers
# the session's IdleHint from STUB_IDLE ("fail" for a loginctl that cannot
# answer, as on a CI runner). The host's own display and session are never
# read, so the cases answer the same on any Linux host. Elsewhere the choice is
# always the current display, which is the one case run there.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly SCRIPT="$ROOT/scripts/e2e-electron.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

readonly LOGINCTL="$work/loginctl"
cat >"$LOGINCTL" <<'STUB'
#!/usr/bin/env bash
# show-user <user> -p Display --value | show-session <id> -p IdleHint --value
echo "$*" >>"$STUB_CALLS"
[ "$STUB_IDLE" != fail ] || exit 1
case "$1" in
  show-user) echo 7 ;;
  show-session) echo "$STUB_IDLE" ;;
  *) exit 1 ;;
esac
STUB
chmod +x "$LOGINCTL"

readonly WESTON="$work/weston"
printf '#!/bin/sh\nexit 1\n' >"$WESTON"
chmod +x "$WESTON"

mkdir -p "$work/runtime"

# Runs the choice for case $1, with the variables after it (NAME=value) set in
# an environment holding nothing of the host's display or session.
run_choice() {
  local name="$1"
  shift
  env -u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_ID -u COFFRET_E2E_ELECTRON_DISPLAY \
    XDG_RUNTIME_DIR="$work/runtime" \
    COFFRET_E2E_WESTON="$WESTON" \
    COFFRET_E2E_LOGINCTL="$LOGINCTL" \
    STUB_IDLE=no \
    STUB_CALLS="$work/$name.calls" \
    "$@" "$SCRIPT" choose-display >"$work/$name.out" 2>"$work/$name.err"
}

# Expects case $1, with the variables after it set, to choose $2.
expect_choice() {
  local name="$1" wanted="$2" chosen
  shift 2
  if ! run_choice "$name" "$@"; then
    echo "FAIL $name: the choice refused"
    cat "$work/$name.err"
    failures=$((failures + 1))
    return
  fi
  chosen="$(cat "$work/$name.out")"
  if [ "$chosen" != "$wanted" ]; then
    echo "FAIL $name: chose '$chosen', not '$wanted'"
    cat "$work/$name.err"
    failures=$((failures + 1))
    return
  fi
  echo "ok   $name"
}

# Expects case $1, with the variables before `--` set, to refuse, saying each
# of the lines after `--`.
expect_refusal() {
  local name="$1" said
  shift
  local settings=()
  while [ "$1" != -- ]; do
    settings+=("$1")
    shift
  done
  shift
  if run_choice "$name" "${settings[@]}"; then
    echo "FAIL $name: the choice did not refuse, and chose $(cat "$work/$name.out")"
    failures=$((failures + 1))
    return
  fi
  for said in "$@"; do
    if ! grep -qF -- "$said" "$work/$name.err"; then
      echo "FAIL $name: the choice refused without saying: $said"
      cat "$work/$name.err"
      failures=$((failures + 1))
      return
    fi
  done
  echo "ok   $name"
}

if [ "$(uname -s)" != Linux ]; then
  expect_choice not-linux current COFFRET_E2E_WESTON=
else
  # The display a person is at, awake, or the one xvfb-run gives CI.
  expect_choice display-awake current DISPLAY=:9
  # A socket left bound in the runtime directory stands for a compositor's;
  # python3 binds it, and where there is none the case is left out.
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import socket, sys; socket.socket(socket.AF_UNIX).bind(sys.argv[1])' \
      "$work/runtime/wayland-9"
    expect_choice wayland-awake current WAYLAND_DISPLAY=wayland-9 COFFRET_E2E_WESTON=
  else
    echo "skip wayland-awake: python3, which binds the stand-in socket, is not on this PATH"
  fi
  # A loginctl that cannot answer, as on a CI runner, is not a blanked screen.
  expect_choice session-unknown current DISPLAY=:9 STUB_IDLE=fail COFFRET_E2E_WESTON=

  # The screen blanked: the session named, or else the user's graphical one.
  expect_choice blanked-session-named weston DISPLAY=:9 STUB_IDLE=yes XDG_SESSION_ID=3
  if ! grep -qx 'show-session 3 -p IdleHint --value' "$work/blanked-session-named.calls"; then
    echo "FAIL blanked-session-named: the session named was not the one asked about"
    failures=$((failures + 1))
  fi
  expect_choice blanked-graphical-session weston DISPLAY=:9 STUB_IDLE=yes
  if ! grep -qx 'show-session 7 -p IdleHint --value' "$work/blanked-graphical-session.calls"; then
    echo "FAIL blanked-graphical-session: the user's graphical session was not the one asked about"
    failures=$((failures + 1))
  fi

  # No display at all, or a WAYLAND_DISPLAY whose socket is not there.
  expect_choice no-display weston
  expect_choice wayland-socket-missing weston WAYLAND_DISPLAY=wayland-gone

  # Nothing to fall back on.
  expect_refusal blanked-no-weston DISPLAY=:9 STUB_IDLE=yes COFFRET_E2E_WESTON= -- \
    "cannot run here" "blanked" "weston" "is not installed" "COFFRET_E2E_ELECTRON_DISPLAY=current"
  expect_refusal no-display-no-weston COFFRET_E2E_WESTON= -- \
    "cannot run here" "there is no display" "is not installed"

  # The choice taken by hand.
  expect_choice taken-current current DISPLAY=:9 STUB_IDLE=yes COFFRET_E2E_WESTON= \
    COFFRET_E2E_ELECTRON_DISPLAY=current
  expect_choice taken-weston weston DISPLAY=:9 COFFRET_E2E_ELECTRON_DISPLAY=weston
  expect_refusal taken-unknown COFFRET_E2E_ELECTRON_DISPLAY=x11 -- \
    "takes current or weston"
fi

if [ "$failures" -ne 0 ]; then
  echo "e2e-electron-display-test: $failures case(s) failed" >&2
  exit 1
fi
echo "e2e-electron-display-test: every case answered as expected"
