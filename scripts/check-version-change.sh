#!/usr/bin/env bash
set -euo pipefail

# Detect whether the workspace version in backend/Cargo.toml went up between
# HEAD~1 and HEAD, and report the result via $GITHUB_OUTPUT so a workflow
# step can branch on it.
#
# Only a raise counts. A version that went down is never a release: the
# workspace was lowered to 0.0.0 once, before the first release, so that the
# first release PR could raise it to 0.1.0, and that commit must not cut a
# v0.0.0.
#
# Outputs (to $GITHUB_OUTPUT, when set):
#   changed=true|false
#   version=<semver>      — only when changed=true; the new version.
#
# Implementation notes:
#   - The version is read by scripts/workspace-version.sh, through
#     `cargo metadata`, from a crate that takes it from the workspace.
#   - The previous version is obtained by temporarily checking out
#     backend/Cargo.toml from HEAD~1 and reading the version against that
#     snapshot, then restoring the working-tree copy. cargo metadata only
#     reads Cargo.toml + Cargo.lock; no network access is required.
#   - It runs against the repository it is run in (`git rev-parse
#     --show-toplevel`), which is what lets scripts/check-version-change-test.sh
#     run it against throwaway repositories.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(git rev-parse --show-toplevel)"
CARGO_TOML="${REPO_ROOT}/backend/Cargo.toml"

read_workspace_version() {
  "${SCRIPT_DIR}/workspace-version.sh" "${REPO_ROOT}/backend"
}

current_version=$(read_workspace_version)

# Stash the working-tree Cargo.toml so we can restore it after reading
# the HEAD~1 snapshot.
backup=$(mktemp)
cp "$CARGO_TOML" "$backup"
trap 'cp "$backup" "$CARGO_TOML"; rm -f "$backup"' EXIT

git show HEAD~1:backend/Cargo.toml >"$CARGO_TOML"
previous_version=$(read_workspace_version)

echo "Current version: ${current_version}"
echo "Previous version: ${previous_version}"

if [ -z "${GITHUB_OUTPUT:-}" ]; then
  GITHUB_OUTPUT=/dev/null
fi

# The higher of two versions, as `sort -V` orders X.Y.Z.
higher_version() {
  printf '%s\n%s\n' "$1" "$2" | sort -V | tail -n 1
}

if [ "$current_version" != "$previous_version" ] &&
  [ "$(higher_version "$current_version" "$previous_version")" = "$current_version" ]; then
  echo "Version raised from ${previous_version} to ${current_version}"
  {
    echo "changed=true"
    echo "version=${current_version}"
  } >>"$GITHUB_OUTPUT"
else
  echo "Version not raised; skipping release"
  echo "changed=false" >>"$GITHUB_OUTPUT"
fi
