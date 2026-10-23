#!/usr/bin/env bash
#
# Show that `scripts/desktop.sh place-addon`, which `make napi-addon` calls,
# places the addon as a new file each time rather than rewriting the old one.
#
# A copy onto coffret.node would keep its inode and change the file under a
# development app that has it loaded; on macOS the kernel then kills the next
# app started from it. A rename of a fresh copy over it gives the path a new
# inode and leaves the old file to whatever has it mapped. So each case places
# a stand-in library, then another with different content, and checks that
# coffret.node is a different inode holding the second one.
#
# The cases run over a throwaway directory of their own, handed to the
# subcommand as COFFRET_ADDON_DIR (documented in scripts/desktop.sh for this
# test), so they need no built addon and never touch backend/target.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly DESKTOP="$ROOT/scripts/desktop.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

# The inode of $1. `ls -i` rather than `stat`, whose flags differ between GNU
# and BSD.
inode_of() {
  local listing
  listing="$(ls -di "$1")"
  echo "${listing%% *}"
}

# Places the library in case $1's directory, failing the case if the
# subcommand fails.
place() {
  local name="$1"
  if ! COFFRET_ADDON_DIR="$work/$name" "$DESKTOP" place-addon >"$work/$name.log" 2>&1; then
    echo "FAIL $name: place-addon failed"
    cat "$work/$name.log"
    return 1
  fi
}

# Case $1: the library Cargo builds on a host, named $2, placed twice.
expect_new_inode() {
  local name="$1" library="$2" dir="$work/$1" first second leftovers
  mkdir -p "$dir"
  echo "first build" >"$dir/$library"
  place "$name" || {
    failures=$((failures + 1))
    return
  }
  first="$(inode_of "$dir/coffret.node")"
  # A new file for the second build, as the linker writes one.
  rm "$dir/$library"
  echo "second build" >"$dir/$library"
  place "$name" || {
    failures=$((failures + 1))
    return
  }
  second="$(inode_of "$dir/coffret.node")"
  if [ "$first" = "$second" ]; then
    echo "FAIL $name: coffret.node kept inode $first across two placements"
    failures=$((failures + 1))
    return
  fi
  if [ "$(cat "$dir/coffret.node")" != "second build" ]; then
    echo "FAIL $name: coffret.node does not hold the second library: $(cat "$dir/coffret.node")"
    failures=$((failures + 1))
    return
  fi
  leftovers="$(find "$dir" -name '.coffret.node.*')"
  if [ -n "$leftovers" ]; then
    echo "FAIL $name: the temporary copy was left behind: $leftovers"
    failures=$((failures + 1))
    return
  fi
  echo "ok   $name (inode $first, then $second)"
}

# Case $1: no library built, which the subcommand must refuse.
expect_refusal_without_library() {
  local name="$1"
  mkdir -p "$work/$name"
  if COFFRET_ADDON_DIR="$work/$name" "$DESKTOP" place-addon >"$work/$name.log" 2>&1; then
    echo "FAIL $name: place-addon succeeded with no library"
    failures=$((failures + 1))
    return
  fi
  if [ -e "$work/$name/coffret.node" ]; then
    echo "FAIL $name: place-addon made coffret.node with no library"
    failures=$((failures + 1))
    return
  fi
  echo "ok   $name"
}

expect_new_inode macos libcoffret_napi.dylib
expect_new_inode linux libcoffret_napi.so
expect_refusal_without_library no-library

if [ "$failures" -ne 0 ]; then
  echo "desktop-addon-test: $failures case(s) failed" >&2
  exit 1
fi
echo "desktop-addon-test: every case answered as expected"
