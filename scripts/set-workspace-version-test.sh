#!/usr/bin/env bash
#
# Show that set-workspace-version.sh sets the workspace version in Cargo.toml,
# that every member crate and Cargo.lock follow it, and that it refuses what it
# cannot set.
#
# Each case is a throwaway workspace under a temporary directory, so the
# repository itself is never touched. Its members are coffret-desktop-core,
# whose version scripts/workspace-version.sh reads, and a crate that depends on
# it by path, both taking their version from the workspace, with a Cargo.lock
# generated for them. The workspace has no registry dependencies, so every
# cargo command here runs offline and needs no fetched crates: what is under
# test is the version and the lock entries of the workspace's own crates,
# which are the only entries the script changes. It lies outside this
# repository, so cargo uses the default toolchain rather than the one
# rust-toolchain.toml pins. It needs cargo and jq.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly SET="$ROOT/scripts/set-workspace-version.sh"
readonly MEMBERS=(coffret-desktop-core coffret-model)

# Nothing here needs the network; say so, so that cargo never reaches for it.
export CARGO_NET_OFFLINE=true

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

fail() {
  echo "FAIL $1: $2"
  [ -z "${3:-}" ] || cat "$3"
  failures=$((failures + 1))
}

# Writes a workspace into directory $1 at version $2, with a Cargo.lock. The
# table after [workspace.package] holds a `version` key of its own, which the
# script must leave alone. With $3 set to `no-version`, [workspace.package]
# holds no version line, and the members name their versions themselves.
workspace() {
  local dir="$1" version="$2" variant="${3:-}" crate member_version
  member_version='version.workspace = true'
  [ "$variant" != no-version ] || member_version="version = \"$version\""
  mkdir -p "$dir"
  for crate in "${MEMBERS[@]}"; do
    mkdir -p "$dir/crates/$crate/src"
    : >"$dir/crates/$crate/src/lib.rs"
    printf '[package]\nname = "%s"\n%s\nedition.workspace = true\n' "$crate" "$member_version" \
      >"$dir/crates/$crate/Cargo.toml"
  done
  printf '\n[dependencies]\ncoffret-desktop-core = { path = "../coffret-desktop-core" }\n' \
    >>"$dir/crates/coffret-model/Cargo.toml"
  {
    printf '[workspace]\nresolver = "2"\nmembers = ["crates/*"]\n\n'
    printf '[workspace.package]\n'
    [ "$variant" = no-version ] || printf 'version = "%s"  # the release version\n' "$version"
    printf 'edition = "2021"\n\n'
    printf '[workspace.metadata.unrelated]\nversion = "untouched"\n'
  } >"$dir/Cargo.toml"
  (cd "$dir" && cargo generate-lockfile --quiet)
}

# The version Cargo.lock records for crate $2 of the workspace in $1.
locked_version() {
  awk -v crate="$2" '
    $0 == "name = \"" crate "\"" { found = 1; next }
    found && /^version = / { gsub(/^version = "|"$/, ""); print; exit }
  ' "$1/Cargo.lock"
}

# Sets the workspace of case $1 to version $2 and checks everything that
# should then read $2.
expect_set() {
  local name="$1" version="$2" dir="$work/$1" log="$work/$1.log" crate actual
  workspace "$dir" 0.1.0
  if ! "$SET" "$version" "$dir" >"$log" 2>&1; then
    fail "$name" "the script failed" "$log"
    return
  fi
  if ! grep -qx "version = \"$version\"  # the release version" "$dir/Cargo.toml"; then
    fail "$name" "Cargo.toml does not hold version = \"$version\"" "$dir/Cargo.toml"
    return
  fi
  if ! grep -qx 'version = "untouched"' "$dir/Cargo.toml"; then
    fail "$name" "the version of another table was changed" "$dir/Cargo.toml"
    return
  fi
  actual="$("$ROOT/scripts/workspace-version.sh" "$dir")"
  if [ "$actual" != "$version" ]; then
    fail "$name" "scripts/workspace-version.sh reads $actual"
    return
  fi
  for crate in "${MEMBERS[@]}"; do
    actual="$(cd "$dir" && cargo metadata --no-deps --format-version 1 |
      jq -r --arg crate "$crate" '.packages[] | select(.name == $crate) | .version')"
    if [ "$actual" != "$version" ]; then
      fail "$name" "cargo metadata reads $crate at $actual"
      return
    fi
    actual="$(locked_version "$dir" "$crate")"
    if [ "$actual" != "$version" ]; then
      fail "$name" "Cargo.lock holds $crate at $actual" "$dir/Cargo.lock"
      return
    fi
  done
  if ! (cd "$dir" && cargo metadata --locked --offline --format-version 1 >/dev/null 2>"$log"); then
    fail "$name" "cargo metadata --locked --offline refused the result" "$log"
    return
  fi
  echo "ok   $name"
}

# Runs the script with the arguments after $3 against case $1, prepared as
# workspace variant $2, expecting it to fail, to say $3, and to leave
# Cargo.toml and Cargo.lock as they were.
expect_refusal() {
  local name="$1" variant="$2" said="$3" dir="$work/$1" log="$work/$1.log"
  shift 3
  workspace "$dir" 0.1.0 "$variant"
  cp "$dir/Cargo.toml" "$work/$name.toml"
  cp "$dir/Cargo.lock" "$work/$name.lock"
  if "$SET" "$@" "$dir" >"$log" 2>&1; then
    fail "$name" "the script succeeded" "$log"
    return
  fi
  if ! grep -qF "$said" "$log"; then
    fail "$name" "the script failed without saying: $said" "$log"
    return
  fi
  if ! cmp -s "$dir/Cargo.toml" "$work/$name.toml" || ! cmp -s "$dir/Cargo.lock" "$work/$name.lock"; then
    fail "$name" "Cargo.toml or Cargo.lock was changed"
    return
  fi
  echo "ok   $name"
}

expect_set patch 0.1.1
expect_set minor 0.2.0
expect_set major 1.0.0

expect_refusal tag-not-version '' 'is not a version of the form X.Y.Z' v0.2.0
expect_refusal pre-release '' 'is not a version of the form X.Y.Z' 0.2.0-rc.1
expect_refusal no-version-line no-version 'does not have exactly one version line' 0.2.0

if [ "$failures" -ne 0 ]; then
  echo "set-workspace-version-test: $failures case(s) failed" >&2
  exit 1
fi
echo "set-workspace-version-test: every case answered as expected"
