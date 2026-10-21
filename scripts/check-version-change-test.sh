#!/usr/bin/env bash
#
# Show that check-version-change.sh reports the workspace version, and decides
# whether it went up, from what it reads through cargo metadata.
#
# Each case is a throwaway git repository under a temporary directory, holding
# a backend/ workspace of its own with two commits, so the repository itself is
# never touched. The workspace's members are coffret-desktop-core, whose
# version scripts/workspace-version.sh reads, and an unrelated crate. One case
# leaves coffret-desktop-core out, and expects the script to fail rather than
# report an empty version. It needs git, cargo and jq.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly CHECK="$ROOT/scripts/check-version-change.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

# Writes backend/ of $1 as a workspace at version $2 whose members are the
# crates named after it.
workspace() {
  local dir="$1" version="$2" crate members=""
  shift 2
  mkdir -p "$dir/backend"
  for crate in "$@"; do
    mkdir -p "$dir/backend/crates/$crate/src"
    : >"$dir/backend/crates/$crate/src/lib.rs"
    printf '[package]\nname = "%s"\nversion.workspace = true\nedition = "2021"\n' "$crate" \
      >"$dir/backend/crates/$crate/Cargo.toml"
    members="$members\"crates/$crate\", "
  done
  printf '[workspace]\nresolver = "2"\nmembers = [%s]\n\n[workspace.package]\nversion = "%s"\n' \
    "${members%, }" "$version" >"$dir/backend/Cargo.toml"
}

# A repository named $1 whose first commit is at version $2 and whose second
# is at $3, with the members after them.
repository() {
  local dir="$work/$1" before="$2" after="$3"
  shift 3
  mkdir -p "$dir"
  git -C "$dir" init -q
  workspace "$dir" "$before" "$@"
  git -C "$dir" add -A
  git -C "$dir" -c user.name=test -c user.email=test@example.invalid commit -q -m before
  workspace "$dir" "$after" "$@"
  git -C "$dir" add -A
  git -C "$dir" -c user.name=test -c user.email=test@example.invalid commit -q --allow-empty -m after
}

# Runs the check in repository $1, expecting it to succeed and write exactly
# the lines after it to GITHUB_OUTPUT.
expect_output() {
  local name="$1" dir="$work/$1" output="$work/$1.output" expected
  shift
  expected="$(printf '%s\n' "$@")"
  if ! (cd "$dir" && GITHUB_OUTPUT="$output" "$CHECK") >"$work/$name.log" 2>&1; then
    echo "FAIL $name: the check failed"
    cat "$work/$name.log"
    failures=$((failures + 1))
    return
  fi
  if [ "$(cat "$output")" != "$expected" ]; then
    echo "FAIL $name: expected the output"
    echo "$expected"
    echo "but got"
    cat "$output"
    failures=$((failures + 1))
    return
  fi
  if ! git -C "$dir" diff --quiet -- backend/Cargo.toml; then
    echo "FAIL $name: backend/Cargo.toml was left changed"
    failures=$((failures + 1))
    return
  fi
  echo "ok   $name"
}

# Runs the check in repository $1, expecting it to fail and to say $2.
expect_failure() {
  local name="$1" said="$2" dir="$work/$1"
  if (cd "$dir" && GITHUB_OUTPUT="$work/$name.output" "$CHECK") >"$work/$name.log" 2>&1; then
    echo "FAIL $name: the check succeeded"
    cat "$work/$name.log"
    failures=$((failures + 1))
    return
  fi
  if ! grep -qF "$said" "$work/$name.log"; then
    echo "FAIL $name: the check failed without saying: $said"
    cat "$work/$name.log"
    failures=$((failures + 1))
    return
  fi
  echo "ok   $name"
}

repository raised 0.1.0 0.2.0 coffret-desktop-core coffret-model
expect_output raised changed=true version=0.2.0

repository unchanged 0.1.0 0.1.0 coffret-desktop-core coffret-model
expect_output unchanged changed=false

# The workspace was once lowered to 0.0.0 ahead of the first release; that is
# never a release.
repository lowered 0.1.0 0.0.0 coffret-desktop-core coffret-model
expect_output lowered changed=false

repository no-version-crate 0.1.0 0.2.0 coffret-model
expect_failure no-version-crate 'has no coffret-desktop-core'

if [ "$failures" -ne 0 ]; then
  echo "check-version-change-test: $failures case(s) failed" >&2
  exit 1
fi
echo "check-version-change-test: every case answered as expected"
